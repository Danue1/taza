//! 원천 하나를 **신호**로 바꾸는 크레이트 — 조달 → 캐시 → 압축 해제 → 파싱까지.
//!
//! 여기서 끝나는 이유는 그다음이 다른 종류의 앎이기 때문이다. 원천을 읽는 데 필요한
//! 것은 형식과 표기 지식이고, 그 신호들을 어떻게 합쳐 얼마만큼 실을지는 예산과 언어
//! 정책의 문제다 — 그쪽은 `taza-packbuild`가 안다.
//!
//! 그래서 원천이 하나 늘어나는 일이 이 크레이트 안에서 파일 하나가 늘어나는 일이 되고,
//! 예산 상수를 만지는 일은 여기를 다시 컴파일하지 않는다.

pub mod declaration;
pub mod lang;
pub mod parse;
pub mod source;

pub use declaration::{AnnotationGroupName, Extraction, Origin, SourceFile};
pub use parse::{Annotation, Signal};
pub use source::{Prepared, prepare};
