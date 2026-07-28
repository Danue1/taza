//! 섹션 하나씩의 빌더. 섹션은 서로를 모르므로 순서가 없다 — 그것들을 담는 그릇
//! (`PackWriter`)은 섹션이 아니므로 크레이트 뿌리에 있다.

pub mod annotation;
pub mod lexicon;
pub mod metadata;
pub mod ngram;
