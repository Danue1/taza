//! 오프라인 파이프라인 — 원천 조달부터 언어팩 배포 산출물까지. 기기에서 돌지 않는
//! 코드만 여기에 둔다. 바이트를 어떻게 적을지는 `taza-pack`이 알고(그 위의 단일 출처는
//! `taza_engine::pack`이다), 이 크레이트는 **무엇을 적을지**를 정한다.
//!
//! 단계: `recipe`(무엇을 쓸지 선언) → **원천을 신호로**(`taza-corpus`) →
//! `normalize`(병합·승격·점수 정규화) → `assemble`(팩 조립) →
//! `distribute`(압축 아카이브·카탈로그·고지). 그 **순서**를 소유하는 것은 `pipeline`이다 —
//! 실행 파일에 두면 문서로만 남고 회귀 테스트로 고정할 수 없다.
//! `taza build`가 이 순서를 그대로 실행하고, `taza compile`은 이미 만들어진 TSV를
//! 팩으로 굽는 낮은 층 도구다.

pub mod assemble;
pub mod distribute;
pub mod normalize;
pub mod pipeline;
pub mod recipe;
