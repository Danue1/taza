//! 언어팩 설치 — 내려받은 압축 아카이브를 검증해 풀어 놓는다.
//!
//! 팩은 기기에서 mmap 조회로 읽히므로 압축된 채로 둘 수 없다. 압축은 전송 구간에만
//! 쓰고, 설치는 (1) 아카이브 해시 확인 → (2) 해제 → (3) 팩 해시·헤더 확인 →
//! (4) 임시 파일에 쓰고 원자적 교체의 순서로 한다. 마지막 단계가 원자적이어야
//! 익스텐션이 반쯤 쓰인 팩을 mmap하는 일이 없다.
//!
//! 다운로드 자체는 컨테이너 앱(셸)의 일이다 — 익스텐션은 네트워크를 쓰지 않는다.

use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;
use taza_engine::pack::Pack;
use taza_engine::pack::metadata::keys;

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum FfiInstallError {
    #[error("파일을 다룰 수 없음: {message}")]
    Io { message: String },
    #[error("해시가 기대와 다름 ({subject}): 기대 {expected}, 실제 {actual}")]
    ChecksumMismatch {
        subject: String,
        expected: String,
        actual: String,
    },
    #[error("압축을 풀 수 없음: {message}")]
    Decompression { message: String },
    #[error("팩 형식 오류: {message}")]
    Invalid { message: String },
}

/// 설치된 팩의 신원 — 셸이 목록에 표시하고 갱신 여부를 판단하는 데 쓴다.
#[derive(uniffi::Record)]
pub struct FfiInstalledPack {
    pub path: String,
    pub language: String,
    pub pack_version: u32,
    pub word_count: u32,
    /// 고지 화면에 그대로 싣는 저작자 표시 문구
    pub attribution: String,
    pub byte_size: u64,
}

fn hex_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn verify(subject: &str, bytes: &[u8], expected: &str) -> Result<(), FfiInstallError> {
    let actual = hex_digest(bytes);
    if actual == expected {
        return Ok(());
    }
    Err(FfiInstallError::ChecksumMismatch {
        subject: subject.to_string(),
        expected: expected.to_string(),
        actual,
    })
}

fn describe(path: &Path, bytes: &[u8]) -> Result<FfiInstalledPack, FfiInstallError> {
    let pack = Pack::open(bytes).map_err(|error| FfiInstallError::Invalid {
        message: error.to_string(),
    })?;
    let metadata = pack.metadata();
    let read = |key: &str| {
        metadata
            .as_ref()
            .and_then(|metadata| metadata.get(key))
            .unwrap_or_default()
    };
    Ok(FfiInstalledPack {
        path: path.display().to_string(),
        language: pack.language().to_string(),
        pack_version: read(keys::PACK_VERSION).parse().unwrap_or(0),
        word_count: read(keys::WORD_COUNT).parse().unwrap_or(0),
        attribution: read(keys::ATTRIBUTION).to_string(),
        byte_size: bytes.len() as u64,
    })
}

/// 아카이브를 검증·해제해 `destination_path`에 놓는다. 이미 있는 팩은 교체된다.
#[uniffi::export]
pub fn install_pack_archive(
    archive_path: String,
    destination_path: String,
    expected_archive_sha256: String,
    expected_pack_sha256: String,
) -> Result<FfiInstalledPack, FfiInstallError> {
    let archive = std::fs::read(&archive_path).map_err(|error| FfiInstallError::Io {
        message: format!("{archive_path}: {error}"),
    })?;
    verify("아카이브", &archive, &expected_archive_sha256)?;

    let mut decoder =
        ruzstd::decoding::StreamingDecoder::new(archive.as_slice()).map_err(|error| {
            FfiInstallError::Decompression {
                message: error.to_string(),
            }
        })?;
    let mut pack_bytes = Vec::new();
    decoder
        .read_to_end(&mut pack_bytes)
        .map_err(|error| FfiInstallError::Decompression {
            message: error.to_string(),
        })?;
    verify("팩", &pack_bytes, &expected_pack_sha256)?;

    let destination = Path::new(&destination_path);
    // 열려 있는 팩을 덮어쓰지 않도록 임시 파일에 쓴 뒤 이름만 바꾼다
    let staging = destination.with_extension("tazapack.installing");
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|error| FfiInstallError::Io {
            message: format!("{}: {error}", parent.display()),
        })?;
    }
    let described = describe(destination, &pack_bytes)?;
    std::fs::write(&staging, &pack_bytes).map_err(|error| FfiInstallError::Io {
        message: format!("{}: {error}", staging.display()),
    })?;
    std::fs::rename(&staging, destination).map_err(|error| FfiInstallError::Io {
        message: format!("{}: {error}", destination.display()),
    })?;
    Ok(described)
}

/// 이 빌드가 읽을 수 있는 팩 포맷 버전 — 셸은 카탈로그를 이 값과 견주어 받을지 정한다.
#[uniffi::export]
pub fn supported_pack_format_version() -> u16 {
    taza_engine::pack::FORMAT_VERSION
}

/// 설치된 팩의 신원을 읽는다 — 갱신 판단과 고지 표시용.
#[uniffi::export]
pub fn read_installed_pack(path: String) -> Result<FfiInstalledPack, FfiInstallError> {
    let bytes = std::fs::read(&path).map_err(|error| FfiInstallError::Io {
        message: format!("{path}: {error}"),
    })?;
    describe(Path::new(&path), &bytes)
}
