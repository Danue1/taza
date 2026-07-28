//! 온디바이스 입력 엔진 — sans-io. 파일 IO·플랫폼 API는 셸(taza-ffi)이, 팩 생성 같은
//! 오프라인 작업은 taza-corpus·taza-packbuild가 담당한다. 이 크레이트에 코드를 두는 기준은 하나다:
//! 키보드가 떠 있는 동안 기기에서 도는가.

pub mod contract;
pub mod engine;
pub mod keyboard;
pub mod lang;
pub mod pack;
pub mod personalization;
mod policy;
pub mod suggest;
