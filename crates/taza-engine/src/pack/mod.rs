//! 언어팩 바이너리 포맷 — 읽기 경로. 쓰기(컴파일)는 taza-toolchain 전용이며,
//! 포맷 상수·섹션 태그는 이 모듈이 단일 출처다.
//!
//! 익스텐션 메모리 상한(최악 48MB) 때문에 팩은 힙에 파싱하지 않는다 — mmap된 바이트
//! 슬라이스 위에서 그대로 조회한다.
//! 섹션 태그 레지스트리로 확장한다: 미지 태그는 무시되므로 구버전 앱도 신버전 팩을 읽는다.
//!
//! 팩 레이아웃:
//! ```text
//! magic "TAZA" | format_version u16 | language_length u8 | language utf-8
//! section_count u16 | (tag u32 | offset u64 | length u64) × n | 섹션 바이트들
//! ```
//! 모든 정수는 little-endian이며 offset은 파일 시작 기준.

pub mod layout;
pub mod lexicon;
pub mod metadata;
pub mod ngram;
mod reader;

pub use reader::{Pack, PackError};

pub const FORMAT_VERSION: u16 = 3;

pub const MAGIC: &[u8; 4] = b"TAZA";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    Lexicon,
    NgramModel,
    Layout,
    Metadata,
}

impl SectionKind {
    pub fn tag(self) -> u32 {
        match self {
            SectionKind::Lexicon => 1,
            SectionKind::NgramModel => 2,
            SectionKind::Layout => 3,
            SectionKind::Metadata => 4,
        }
    }

    pub(crate) fn from_tag(tag: u32) -> Option<Self> {
        match tag {
            1 => Some(SectionKind::Lexicon),
            2 => Some(SectionKind::NgramModel),
            3 => Some(SectionKind::Layout),
            4 => Some(SectionKind::Metadata),
            _ => None,
        }
    }
}
