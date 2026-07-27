//! 팩 섹션의 쓰기 짝. 섹션 바이트 레이아웃의 단일 출처는 `taza_engine::pack`이고,
//! 여기 있는 것은 그 위에 얹은 쓰기 경로뿐이다 — 읽기 코드가 익스텐션에 링크되고
//! 쓰기 코드는 링크되지 않는다.
//!
//! 파이프라인 단계(`recipe`→`source`→`parse`→`normalize`→`assemble`→`distribute`)와는
//! 다른 축이라 한 층에 두지 않는다. 단계는 순서를 가지지만 섹션은 서로를 모른다.
//! 섹션을 담는 그릇(`PackWriter`)은 섹션이 아니므로 크레이트 뿌리에 남는다.

pub mod annotation;
pub mod layout;
pub mod lexicon;
pub mod metadata;
pub mod ngram;
