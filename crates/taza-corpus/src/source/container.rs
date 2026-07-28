//! 압축 컨테이너 해제 — 원천이 tar.gz 한 덩이든 zip이든 평문 한 장이든, 파서가
//! "이름과 읽기 스트림"만 보게 한다. 파서마다 제 압축을 여는 코드를 두지 않기 위해서다.

use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use std::io::{BufRead, BufReader, Read, Seek};
use std::path::Path;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};

/// 앞질러 풀어 둘 덩이 수. 파서가 한 덩이를 훑는 동안 압축 해제가 다음 덩이를
/// 만들어 두면 되므로 깊이는 얕아도 된다.
const READ_AHEAD: usize = 4;
const CHUNK: usize = 1 << 18;

/// 압축 해제를 파싱과 겹쳐 돌린다.
///
/// 압축 해제와 파싱은 둘 다 한 실을 꽉 채우는 일이라, 한 실에서 번갈아 하면 시간이
/// 그대로 더해진다. 위키백과 덤프는 푸는 데만 1분 반이 들어 그 겹침이 곧 그만큼의
/// 단축이 된다.
pub fn piped(mut inner: impl Read + Send + 'static) -> PipedReader {
    let (sender, receiver): (SyncSender<std::io::Result<Vec<u8>>>, _) = sync_channel(READ_AHEAD);
    std::thread::spawn(move || {
        loop {
            let mut chunk = vec![0u8; CHUNK];
            match inner.read(&mut chunk) {
                Ok(0) => return,
                Ok(read) => {
                    chunk.truncate(read);
                    if sender.send(Ok(chunk)).is_err() {
                        return;
                    }
                }
                Err(error) => {
                    let _ = sender.send(Err(error));
                    return;
                }
            }
        }
    });
    PipedReader {
        receiver,
        chunk: Vec::new(),
        at: 0,
    }
}

/// 나뉘어 압축된 파일을 처음부터 끝까지 읽는 스트림.
///
/// bzip2도 zstd도 여러 덩이를 이어 붙인 파일이 하나의 파일로 읽힌다. 위키백과가 내는
/// multistream 덤프가 그렇게 만들어져 있고, 덩이마다 따로 풀 수 있으므로 실을 나눠
/// 맡긴다 — 압축 해제가 파이프라인에서 가장 무거운 한 덩이다. 나눌 데가 없는 파일이면
/// 해제와 파싱을 겹치는 것으로 그친다.
pub fn open(path: &Path) -> Result<PipedReader, String> {
    if is_zstd(path) {
        let file = std::fs::File::open(path)
            .map_err(|error| format!("{} 열기 실패: {error}", path.display()))?;
        let decoder = zstd::Decoder::new(file)
            .map_err(|error| format!("{} 여는 데 실패: {error}", path.display()))?;
        return Ok(piped(decoder));
    }
    let streams = compressed_chunks(path)?;
    let file = std::fs::File::open(path)
        .map_err(|error| format!("{} 열기 실패: {error}", path.display()))?;
    if streams.len() < 2 {
        return Ok(piped(BzDecoder::new(file)));
    }

    // 실 하나가 스트림을 건너뛰며 맡고(0번 실이 0, W, 2W…), 읽는 쪽이 실을 돌아가며
    // 거두면 순서가 저절로 맞는다 — 거둔 것을 다시 줄 세우는 자리가 필요 없다.
    let lanes = std::thread::available_parallelism().map_or(4, |count| count.get());
    let lanes = lanes.min(streams.len());
    let receivers: Vec<Receiver<std::io::Result<Vec<u8>>>> = (0..lanes)
        .map(|lane| {
            let (sender, receiver) = sync_channel(1);
            let path = path.to_path_buf();
            let ranges: Vec<(u64, u64)> =
                streams.iter().copied().skip(lane).step_by(lanes).collect();
            std::thread::spawn(move || {
                for (start, stop) in ranges {
                    let decoded = decode_chunk(&path, start, stop);
                    let failed = decoded.is_err();
                    if sender.send(decoded).is_err() || failed {
                        return;
                    }
                }
            });
            receiver
        })
        .collect();

    let (sender, receiver) = sync_channel(READ_AHEAD);
    std::thread::spawn(move || {
        for lane in (0..lanes).cycle() {
            // 한 실이 동나면 그 뒤의 실에도 남은 것이 없다 — 스트림을 실 수만큼
            // 건너뛰며 나눠 맡았기 때문이다.
            let Ok(chunk) = receivers[lane].recv() else {
                return;
            };
            if sender.send(chunk).is_err() {
                return;
            }
        }
    });
    Ok(PipedReader {
        receiver,
        chunk: Vec::new(),
        at: 0,
    })
}

/// 덩이 하나를 통째로 푼다.
pub fn decode_chunk(path: &Path, start: u64, stop: u64) -> std::io::Result<Vec<u8>> {
    let mut file = std::fs::File::open(path)?;
    file.seek(std::io::SeekFrom::Start(start))?;
    let taken = file.take(stop - start);
    let mut decoded = Vec::new();
    if is_zstd(path) {
        zstd::Decoder::new(taken)?.read_to_end(&mut decoded)?;
    } else {
        BzDecoder::new(taken).read_to_end(&mut decoded)?;
    }
    Ok(decoded)
}

/// 곁들여 두는 덩이 자리표의 확장자. zstd 덩이는 표지를 뒤져 찾을 수 없어(네 바이트라
/// 압축된 자료 안에서 우연히 나타난다) 옮겨 담을 때 자리를 적어 둔다.
const CHUNK_INDEX: &str = "chunks";

pub fn is_zstd(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension == "zst")
}

/// 덩이 자리표 파일의 경로.
pub fn chunk_index_path(path: &Path) -> std::path::PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".");
    name.push(CHUNK_INDEX);
    std::path::PathBuf::from(name)
}

/// 파일 안의 덩이 구간들 — (시작, 끝). zstd는 곁들인 자리표를 읽고, bzip2는 표지를 찾는다.
pub fn compressed_chunks(path: &Path) -> Result<Vec<(u64, u64)>, String> {
    if is_zstd(path) {
        let index = chunk_index_path(path);
        let bytes = std::fs::read(&index)
            .map_err(|error| format!("{} 읽기 실패: {error}", index.display()))?;
        let starts: Vec<u64> = bytes
            .chunks_exact(8)
            .map(|chunk| u64::from_le_bytes(chunk.try_into().unwrap_or_default()))
            .collect();
        return Ok(spans(&starts, file_length(path)?));
    }
    bzip2_streams(path)
}

/// 파일 안의 bzip2 스트림 구간들 — (시작, 끝). 스트림 머리(`BZh` + 블록 크기)와 첫
/// 블록의 표지가 잇달아 나오는 자리를 찾는다 — 열 바이트가 통째로 우연히 맞을 일은 없다.
///
/// 구간을 밖으로 내는 이유는 푸는 것만 나누는 것으로는 모자라기 때문이다. 스트림
/// 경계가 곧 내용의 경계인 원천은(위키백과 덤프는 스트림 하나가 문서 100편) 손질까지
/// 나눠 맡길 수 있다.
fn bzip2_streams(path: &Path) -> Result<Vec<(u64, u64)>, String> {
    const HEADER: [u8; 6] = [0x31, 0x41, 0x59, 0x26, 0x53, 0x59];
    const LENGTH: usize = 4 + HEADER.len();
    let mut file = BufReader::new(
        std::fs::File::open(path)
            .map_err(|error| format!("{} 열기 실패: {error}", path.display()))?,
    );
    let mut starts = Vec::new();
    let mut window = vec![0u8; 1 << 20];
    let mut filled = 0usize;
    let mut offset = 0u64;
    loop {
        let read = file
            .read(&mut window[filled..])
            .map_err(|error| format!("{} 읽기 실패: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        filled += read;
        if filled < LENGTH {
            continue;
        }
        for (at, candidate) in window[..filled].windows(LENGTH).enumerate() {
            if &candidate[..3] == b"BZh"
                && candidate[3].is_ascii_digit()
                && candidate[4..] == HEADER
            {
                starts.push(offset + at as u64);
            }
        }
        // 표지가 덩이 경계에 걸쳐 있을 수 있으므로 꼬리를 다음 덩이 앞에 남긴다
        let keep = LENGTH - 1;
        window.copy_within(filled - keep..filled, 0);
        offset += (filled - keep) as u64;
        filled = keep;
    }
    Ok(spans(&starts, file_length(path)?))
}

fn file_length(path: &Path) -> Result<u64, String> {
    std::fs::metadata(path)
        .map(|metadata| metadata.len())
        .map_err(|error| format!("{} 크기 읽기 실패: {error}", path.display()))
}

/// 시작 자리 목록을 (시작, 끝) 구간으로 잇는다.
fn spans(starts: &[u64], end: u64) -> Vec<(u64, u64)> {
    starts
        .iter()
        .enumerate()
        .map(|(index, &start)| (start, starts.get(index + 1).copied().unwrap_or(end)))
        .collect()
}

pub struct PipedReader {
    receiver: Receiver<std::io::Result<Vec<u8>>>,
    chunk: Vec<u8>,
    at: usize,
}

impl Read for PipedReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let available = self.fill_buf()?;
        let taken = available.len().min(buffer.len());
        buffer[..taken].copy_from_slice(&available[..taken]);
        self.consume(taken);
        Ok(taken)
    }
}

impl BufRead for PipedReader {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        if self.at == self.chunk.len() {
            match self.receiver.recv() {
                Ok(chunk) => {
                    self.chunk = chunk?;
                    self.at = 0;
                }
                // 보내는 쪽이 끝났다 — 더 올 것이 없다
                Err(_) => return Ok(&[]),
            }
        }
        Ok(&self.chunk[self.at..])
    }

    fn consume(&mut self, taken: usize) {
        self.at += taken;
    }
}

/// 원천 묶음 안의 파일들을 하나씩 넘긴다. 배포 형태가 zip 한 덩이든(모두의 말뭉치),
/// gzip 한 장이든, 손으로 뽑은 평문 목록이든 추출기가 같은 코드를 쓰도록 여기서 흡수한다.
pub fn for_each_member(
    path: &Path,
    mut visit: impl FnMut(&str, &mut dyn BufRead) -> Result<(), String>,
) -> Result<(), String> {
    let file = std::fs::File::open(path)
        .map_err(|error| format!("{} 열기 실패: {error}", path.display()))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("source");
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("zip") => {
            let mut archive = zip::ZipArchive::new(file)
                .map_err(|error| format!("{} 열기 실패: {error}", path.display()))?;
            for index in 0..archive.len() {
                let entry = archive
                    .by_index(index)
                    .map_err(|error| format!("{} 항목 읽기 실패: {error}", path.display()))?;
                if !entry.is_file() {
                    continue;
                }
                let entry_name = entry.name().to_string();
                visit(&entry_name, &mut BufReader::new(entry))?;
            }
            Ok(())
        }
        Some("gz") => visit(
            name.trim_end_matches(".gz"),
            &mut piped(GzDecoder::new(file)),
        ),
        Some("bz2") => visit(
            name.trim_end_matches(".bz2"),
            &mut piped(BzDecoder::new(file)),
        ),
        _ => visit(name, &mut BufReader::new(file)),
    }
}

pub fn open_tar_gz(path: &Path) -> Result<tar::Archive<GzDecoder<std::fs::File>>, String> {
    let file = std::fs::File::open(path)
        .map_err(|error| format!("{} 열기 실패: {error}", path.display()))?;
    Ok(tar::Archive::new(GzDecoder::new(file)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn compress(text: &str) -> Vec<u8> {
        let mut encoder = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::fast());
        encoder.write_all(text.as_bytes()).unwrap();
        encoder.finish().unwrap()
    }

    fn read_all(path: &Path) -> String {
        let mut text = String::new();
        open(path).unwrap().read_to_string(&mut text).unwrap();
        text
    }

    /// 이어 붙인 스트림들은 실을 나눠 풀어도 원래 순서로 이어져야 한다.
    #[test]
    fn concatenated_streams_read_in_order() {
        let path = std::env::temp_dir().join("taza-multistream-test.bz2");
        let mut file = std::fs::File::create(&path).unwrap();
        for index in 0..16 {
            file.write_all(&compress(&format!("{index}번 스트림\n")))
                .unwrap();
        }
        drop(file);
        let expected: String = (0..16).map(|index| format!("{index}번 스트림\n")).collect();
        assert_eq!(read_all(&path), expected);
        std::fs::remove_file(&path).unwrap();
    }

    /// 한 덩이짜리 파일도 같은 길로 읽힌다.
    #[test]
    fn a_single_stream_reads_whole() {
        let path = std::env::temp_dir().join("taza-single-stream-test.bz2");
        std::fs::write(&path, compress("한 덩이뿐인 파일\n")).unwrap();
        assert_eq!(read_all(&path), "한 덩이뿐인 파일\n");
        std::fs::remove_file(&path).unwrap();
    }
}
