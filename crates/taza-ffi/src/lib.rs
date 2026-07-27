//! 플랫폼 셸(Swift/Kotlin)이 소비하는 FFI 표면. 코어는 sans-io를 유지하고,
//! 파일 IO(팩 mmap·팩 설치)는 이 계층이 담당한다. 이벤트당 1회 왕복 계약을 그대로
//! 노출한다. 네트워크는 셸의 일이다 — 이 계층은 이미 내려받은 바이트만 다룬다.
//!
//! 세 갈래로 나뉜다: `types`가 셸이 보는 타입을 선언하고, `convert`가 코어 계약과의
//! 번역을 맡으며, `session`이 셸이 쥐는 손잡이 하나를 낸다.

mod convert;
mod install;
mod session;
mod types;

pub use convert::default_user_preferences;
pub use install::{
    FfiInstallError, FfiInstalledPack, install_pack_archive, read_installed_pack,
    supported_pack_format_version,
};
pub use session::{KeyboardSession, personalization_summary};
pub use types::*;

uniffi::setup_scaffolding!();
