//! 압축 컨테이너 해제 — 원천이 tar.gz 한 덩이든 zip이든 평문 한 장이든, 파서가
//! "이름과 읽기 스트림"만 보게 한다. 파서마다 제 압축을 여는 코드를 두지 않기 위해서다.

use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use std::io::{BufRead, BufReader};
use std::path::Path;

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
            &mut BufReader::new(GzDecoder::new(file)),
        ),
        Some("bz2") => visit(
            name.trim_end_matches(".bz2"),
            &mut BufReader::new(BzDecoder::new(file)),
        ),
        _ => visit(name, &mut BufReader::new(file)),
    }
}

pub fn open_tar_gz(path: &Path) -> Result<tar::Archive<GzDecoder<std::fs::File>>, String> {
    let file = std::fs::File::open(path)
        .map_err(|error| format!("{} 열기 실패: {error}", path.display()))?;
    Ok(tar::Archive::new(GzDecoder::new(file)))
}
