//! 언어별 합성기. 각 언어는 feature로 켜고 끈다 — 익스텐션 바이너리에는 그 셸이
//! 지원하는 언어만 링크된다.

#[cfg(feature = "lang-latin")]
pub mod direct;
#[cfg(feature = "lang-hangul")]
pub mod hangul;
#[cfg(feature = "lang-hangul")]
pub mod jamo;
#[cfg(feature = "lang-latin")]
pub mod latin;

use crate::contract::Composer;
use crate::keyboard::{KeyboardLayoutSet, layouts};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    Korean,
}

impl Language {
    /// 언어 키에 찍히는 짧은 표기 (순정 관례: 스크립트를 대표하는 한 글자)
    pub fn keycap_label(self) -> &'static str {
        match self {
            Language::English => "A",
            Language::Korean => "한",
        }
    }

    /// 스페이스바·언어 목록에 쓰는 이름. 언어는 자기 이름으로 표기한다(순정 관례).
    pub fn display_name(self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Korean => "한국어",
        }
    }

    /// 이 빌드에 언어가 포함되지 않았으면 None — 셸은 해당 언어를 비활성 처리한다.
    pub fn composer(self) -> Option<Box<dyn Composer>> {
        match self {
            Language::English => {
                #[cfg(feature = "lang-latin")]
                {
                    Some(Box::new(latin::LatinComposer::new()))
                }
                #[cfg(not(feature = "lang-latin"))]
                {
                    None
                }
            }
            Language::Korean => {
                #[cfg(feature = "lang-hangul")]
                {
                    Some(Box::new(hangul::HangulComposer::new()))
                }
                #[cfg(not(feature = "lang-hangul"))]
                {
                    None
                }
            }
        }
    }

    /// 팩의 레이아웃 섹션이 없을 때 쓰는 내장 배열.
    pub fn builtin_layout(self) -> KeyboardLayoutSet {
        match self {
            Language::English => layouts::qwerty(),
            Language::Korean => layouts::dubeolsik(),
        }
    }
}
