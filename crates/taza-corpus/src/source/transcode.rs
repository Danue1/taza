//! 큰 원천을 더 싸게 풀리는 형식으로 한 번 옮겨 담는다.
//!
//! 위키백과 덤프는 bzip2로 배포되는데, 그 해제가 파이프라인 CPU 시간의 절반을 넘는다.
//! 같은 내용을 zstd로 담으면 푸는 값이 열여섯 배 싸다(실측 59MB/s → 967MB/s). 파일이
//! 조금 커지는 대신, 파서를 고쳐 원천을 다시 훑을 때마다 그 차이를 돌려받는다.
//!
//! 옮겨 담은 사본은 우리가 만든 파생물이라 원천의 신원이 아니다 — 원천의 sha256 검증은
//! 내려받을 때 그대로 하고, 사본은 추출 신호 캐시와 같은 급으로 믿는다. 사본이 의심스러우면
//! 지우면 다시 만들어진다.
//!
//! 덩이 나눔은 그대로 옮긴다. 원천의 스트림 하나가 사본의 프레임 하나가 되므로, 스트림
//! 경계가 곧 내용의 경계라는 성질(위키백과는 스트림 하나가 문서 100편)이 사본에도 남아
//! 손질을 나눠 맡기는 길이 그대로 쓰인다.

use crate::source::container;
use std::io::Write;
use std::path::{Path, PathBuf};

/// 이 크기를 넘는 bzip2 원천만 옮겨 담는다. 작은 원천은 옮겨 담는 값이 푸는 값보다 크다.
const WORTH_TRANSCODING: u64 = 64 * 1024 * 1024;

/// 사본의 압축 수준. 이 사본은 오래 두는 것이 아니라 다시 훑을 때를 위한 것이므로,
/// 압축률보다 만드는 속도와 푸는 속도가 중요하다.
const COMPRESSION: i32 = 3;

/// 원천을 더 싸게 풀리는 사본으로 바꿔 그 경로를 돌려준다. 옮겨 담을 값이 없거나 옮기다
/// 실패하면 원천 경로를 그대로 돌려준다 — 사본은 빠른 길일 뿐 없어도 되는 것이다.
pub fn faster_copy(path: &Path, cache_directory: &Path) -> PathBuf {
    if !worth_transcoding(path) {
        return path.to_path_buf();
    }
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return path.to_path_buf();
    };
    let copy = cache_directory.join(format!("{}.zst", name.trim_end_matches(".bz2")));
    if copy.exists() && container::chunk_index_path(&copy).exists() {
        return copy;
    }
    match rewrite(path, &copy) {
        Ok(()) => copy,
        Err(error) => {
            println!("  사본 만들기를 건너뜀 — {error}");
            let _ = std::fs::remove_file(&copy);
            path.to_path_buf()
        }
    }
}

/// 옮겨 담을 값이 있는 원천인가 — 나뉘어 담긴 bzip2 덩치만 해당한다. 다른 형식은 해제가
/// 이미 싸거나(gzip·zip) 옮겨 담을 덩이 나눔이 없다.
fn worth_transcoding(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension == "bz2")
        && std::fs::metadata(path).is_ok_and(|metadata| metadata.len() >= WORTH_TRANSCODING)
}

/// 원천의 덩이들을 실을 나눠 풀고 다시 눌러, 차례대로 이어 쓴다.
fn rewrite(path: &Path, copy: &Path) -> Result<(), String> {
    let chunks = container::compressed_chunks(path)?;
    if chunks.len() < 2 {
        return Err(format!("{}: 나뉘어 있지 않음", path.display()));
    }
    println!(
        "  더 싸게 풀리도록 옮겨 담는 중: {} 덩이 → {}",
        chunks.len(),
        copy.display()
    );

    let lanes = std::thread::available_parallelism()
        .map_or(4, |count| count.get())
        .min(chunks.len());
    let receivers: Vec<std::sync::mpsc::Receiver<Result<Vec<u8>, String>>> = (0..lanes)
        .map(|lane| {
            let (sender, receiver) = std::sync::mpsc::sync_channel(2);
            let path = path.to_path_buf();
            let mine: Vec<(u64, u64)> = chunks.iter().copied().skip(lane).step_by(lanes).collect();
            std::thread::spawn(move || {
                for (start, stop) in mine {
                    let squeezed = container::decode_chunk(&path, start, stop)
                        .map_err(|error| format!("{} 푸는 데 실패: {error}", path.display()))
                        .and_then(|decoded| {
                            zstd::encode_all(decoded.as_slice(), COMPRESSION)
                                .map_err(|error| format!("다시 누르기 실패: {error}"))
                        });
                    let failed = squeezed.is_err();
                    if sender.send(squeezed).is_err() || failed {
                        return;
                    }
                }
            });
            receiver
        })
        .collect();

    let mut file = std::io::BufWriter::new(
        std::fs::File::create(copy)
            .map_err(|error| format!("{} 만들기 실패: {error}", copy.display()))?,
    );
    let mut starts = Vec::with_capacity(chunks.len());
    let mut at = 0u64;
    // 맡긴 차례대로 거두면 덩이 순서가 그대로 지켜진다
    for lane in (0..lanes).cycle().take(chunks.len()) {
        let frame = receivers[lane]
            .recv()
            .map_err(|_| format!("{}: 옮겨 담다 말았음", path.display()))??;
        starts.extend_from_slice(&at.to_le_bytes());
        at += frame.len() as u64;
        file.write_all(&frame)
            .map_err(|error| format!("{} 쓰기 실패: {error}", copy.display()))?;
    }
    file.flush()
        .map_err(|error| format!("{} 마무리 실패: {error}", copy.display()))?;

    // 자리표를 뒤에 쓴다 — 이것이 있어야 사본을 다 만든 것으로 친다
    let index = container::chunk_index_path(copy);
    std::fs::write(&index, &starts)
        .map_err(|error| format!("{} 쓰기 실패: {error}", index.display()))?;
    Ok(())
}
