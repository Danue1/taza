//! 원천 다운로드와 캐시. 같은 sha256이 이미 캐시에 있으면 네트워크를 타지 않으므로
//! 파이프라인 재실행이 값싸다. 해시가 어긋나면 실패로 끝낸다 — 원천이 조용히 바뀌는
//! 것을 빌드 실패로 드러내는 것이 재현성의 최소 조건이다.

use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

/// 스트리밍 다운로드에서 한 번에 읽는 크기
const CHUNK: usize = 64 * 1024;

pub fn hex_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// 캐시 파일의 sha256 — 원천이 기가바이트 단위라 통째로 메모리에 올리지 않고 흘려 읽는다.
fn file_digest(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path)
        .map_err(|error| format!("{} 열기 실패: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; CHUNK];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("{} 읽기 실패: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// 캐시에 있으면 그대로, 없으면 내려받아 검증한 뒤 캐시에 넣고 경로를 돌려준다.
/// 캐시 파일 이름에 해시를 넣어, 같은 URL의 다른 판이 서로를 덮지 않게 한다.
pub fn fetch(url: &str, expected_sha256: &str, cache_directory: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(cache_directory)
        .map_err(|error| format!("{} 만들기 실패: {error}", cache_directory.display()))?;
    let file_name = url.rsplit('/').next().unwrap_or("source");
    let cached = cache_directory.join(format!("{}-{file_name}", &expected_sha256[..12]));
    if cached.exists() {
        if file_digest(&cached)? == expected_sha256 {
            return Ok(cached);
        }
        // 캐시가 깨졌으면 지우고 다시 받는다
        let _ = std::fs::remove_file(&cached);
    }

    println!("  내려받기 {url}");
    let mut response = ureq::get(url)
        .call()
        .map_err(|error| format!("{url} 요청 실패: {error}"))?;
    let mut body = Vec::new();
    let mut reader = response.body_mut().as_reader();
    let mut buffer = vec![0u8; CHUNK];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("{url} 수신 실패: {error}"))?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&buffer[..read]);
    }

    let digest = hex_digest(&body);
    if digest != expected_sha256 {
        return Err(format!(
            "{url}: sha256 불일치\n  기대: {expected_sha256}\n  실제: {digest}"
        ));
    }
    std::fs::write(&cached, &body)
        .map_err(|error| format!("{} 쓰기 실패: {error}", cached.display()))?;
    Ok(cached)
}
