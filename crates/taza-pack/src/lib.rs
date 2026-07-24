//! 언어팩 바이너리 포맷.
//!
//! 익스텐션 메모리 상한(최악 48MB) 때문에 팩은 힙에 파싱하지 않는다 — mmap된 바이트
//! 슬라이스 위에서 그대로 조회한다. 쓰기(컴파일)는 오프라인 파이프라인 전용.
//! 섹션 태그 레지스트리로 확장한다: 미지 태그는 무시되므로 구버전 앱도 신버전 팩을 읽는다.

pub mod lexicon;
mod reader;
mod writer;

pub use reader::{Pack, PackError};
pub use writer::PackWriter;

pub const FORMAT_VERSION: u16 = 1;

pub(crate) const MAGIC: &[u8; 4] = b"TAZA";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    Lexicon,
}

impl SectionKind {
    pub(crate) fn tag(self) -> u32 {
        match self {
            SectionKind::Lexicon => 1,
        }
    }

    pub(crate) fn from_tag(tag: u32) -> Option<Self> {
        match tag {
            1 => Some(SectionKind::Lexicon),
            _ => None,
        }
    }
}
