//! 원천 조달과 캐시. 같은 sha256이 이미 캐시에 있으면 네트워크를 타지 않으므로
//! 파이프라인 재실행이 값싸다. 해시가 어긋나면 실패로 끝낸다 — 원천이 조용히 바뀌는
//! 것을 빌드 실패로 드러내는 것이 재현성의 최소 조건이다.
//!
//! 모든 원천을 URL로 받을 수 있는 것은 아니다. 이용 신청과 승인을 거쳐야 하는 말뭉치는
//! 사람이 손으로 갖다 놓고, 파이프라인은 그것을 있으면 쓰고 없으면 건너뛴다.

use crate::declaration::{Origin, SourceFile};
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
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// 자리를 잡은 원천 파일. 해시를 함께 내는 이유는 추출 결과 캐시의 키가 되기 때문이다 —
/// 원천이 바뀌면 그 해시가 바뀌고, 캐시가 저절로 무효가 된다.
pub struct Located {
    pub path: PathBuf,
    pub digest: String,
}

/// 원천 파일의 자리를 마련한다. 자리에 없고 선택 원천이면 `None`이다 — 손으로 받아야
/// 하는 말뭉치가 아직 없어도 나머지 원천만으로 팩이 나와야 한다.
///
/// 이름은 사람에게 보일 말에만 쓴다 — 원천을 구하는 일 자체는 그것이 무엇으로 불리는지
/// 알 필요가 없다.
pub fn locate(
    declared: &SourceFile,
    name: &str,
    cache_directory: &Path,
) -> Result<Option<Located>, String> {
    match &declared.origin {
        Origin::Remote { url, sha256 } => match fetch(url, sha256, cache_directory) {
            Ok(path) => Ok(Some(Located {
                path,
                digest: sha256.clone(),
            })),
            Err(error) if declared.is_optional() => {
                println!("  건너뜀 {name} — {error}");
                Ok(None)
            }
            Err(error) => Err(error),
        },
        Origin::Local { file, sha256 } => {
            if !file.exists() {
                if declared.is_optional() {
                    println!("  건너뜀 {name} — {} 없음", file.display());
                    return Ok(None);
                }
                return Err(format!("{name}: {} 없음", file.display()));
            }
            // 손으로 받은 판은 사람마다 다를 수 있어 해시를 필수로 두지 않는다. 다만
            // 캐시 키로는 늘 필요하므로 적혀 있지 않아도 계산한다.
            let digest = file_digest(file)?;
            if let Some(expected) = sha256
                && &digest != expected
            {
                return Err(format!("{name}: sha256 불일치 — {}", file.display()));
            }
            Ok(Some(Located {
                path: file.clone(),
                digest,
            }))
        }
    }
}

/// 캐시에 있으면 그대로, 없으면 내려받아 검증한 뒤 캐시에 넣고 경로를 돌려준다.
/// 캐시 파일 이름에 해시를 넣어, 같은 URL의 다른 판이 서로를 덮지 않게 한다.
pub fn fetch(url: &str, expected_sha256: &str, cache_directory: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(cache_directory)
        .map_err(|error| format!("{} 만들기 실패: {error}", cache_directory.display()))?;
    let file_name = url.rsplit('/').next().unwrap_or("source");
    let cached = cache_directory.join(format!("{}-{file_name}", &expected_sha256[..12]));
    // 캐시에 있는 것은 받을 때 이미 해시를 맞춰 보고 그 해시로 이름 붙여 넣은 것이므로
    // 다시 세지 않는다 — 기가바이트짜리 원천을 실행마다 해싱하면 캐시가 아끼려던 시간을
    // 도로 내놓게 된다. 캐시 파일이 의심스러우면 지우면 다시 받아 검증한다.
    if cached.exists() {
        return Ok(cached);
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
