//! 배송되는 배열 전부. 조합 규칙이 코드인 이상 그 규칙으로 치는 자판도 코드에 있어야
//! 짝이 맞는다 — 배열을 고르는 일이 사전을 받는 일과 무관해지고, 배열이 요구하는 방식이
//! 이 빌드에 없는 경우가 아예 생기지 않는다.
//!
//! 어느 배열이 어느 입력 방식에 딸리는지는 여기가 아니라 방식이 정한다
//! (`lang::InputMethod::layouts`) — 방식 하나가 자기 자판과 자기 합성기를 함께 갖는다.
//!
//! 배열을 늘리는 일은 파일 하나에 함수 하나를 더하고 그 스크립트의 `layouts()`에 한 줄을
//! 잇는 일이다. 대신 새 배열은 앱 판올림으로 나간다.

#[cfg(any(feature = "lang-latin", feature = "lang-hangul"))]
mod builder;
#[cfg(feature = "lang-hangul")]
mod cheonjiin;
#[cfg(feature = "lang-hangul")]
mod danmoeum;
#[cfg(feature = "lang-hangul")]
pub(crate) mod hangul;
#[cfg(feature = "lang-latin")]
pub(crate) mod latin;
#[cfg(feature = "lang-hangul")]
mod naratgeul;
#[cfg(any(feature = "lang-latin", feature = "lang-hangul"))]
mod shared;

#[cfg(any(feature = "lang-latin", feature = "lang-hangul"))]
use crate::keyboard::layout::{KeyboardLayoutSet, NamedLayoutSet};
#[cfg(feature = "lang-hangul")]
use crate::lang::InputMethod;

#[cfg(any(feature = "lang-latin", feature = "lang-hangul"))]
fn named(name: &'static str, layouts: KeyboardLayoutSet) -> NamedLayoutSet {
    NamedLayoutSet {
        name,
        method: None,
        layouts,
    }
}

/// 같은 언어 안에서 조합 규칙이 다른 배열은 자기 방식을 밝힌다 — 배열을 고르면 합성기가
/// 함께 갈린다.
#[cfg(feature = "lang-hangul")]
fn named_with_method(
    name: &'static str,
    method: &'static dyn InputMethod,
    layouts: KeyboardLayoutSet,
) -> NamedLayoutSet {
    NamedLayoutSet {
        name,
        method: Some(method),
        layouts,
    }
}
