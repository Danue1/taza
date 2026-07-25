//! 오프라인 파이프라인 — 원천 조달부터 언어팩 배포 산출물까지. 기기에서 돌지 않는
//! 코드만 여기에 둔다. 포맷 상수·섹션 태그·와이어 레이아웃의 단일 출처는
//! `taza_engine::pack`이며, 이 크레이트는 그 위에 쓰기 경로만 얹는다.
//!
//! 단계: `recipe`(무엇을 쓸지 선언) → `source`(조달·캐시·압축 해제) →
//! `parse`(원천 형식별 파싱) → `normalize`(병합·승격·점수 정규화) →
//! `assemble`(팩 조립) → `distribute`(압축 아카이브·카탈로그·고지).
//! `lang`은 언어별 표기 지식을 모아 두는 자리로, `parse`와 `normalize`가 그것을 읽는다.
//! `taza-packs`가 이 순서를 그대로 실행하고, `taza-packc`는 이미 만들어진 TSV를
//! 팩으로 굽는 낮은 층 도구다.

pub mod assemble;
pub mod distribute;
pub mod emoji;
pub mod lang;
pub mod layout;
pub mod lexicon;
pub mod metadata;
pub mod ngram;
pub mod normalize;
pub mod parse;
pub mod recipe;
pub mod source;
mod writer;

pub use writer::PackWriter;
