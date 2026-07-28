//! 배송되는 배열 전부. 조합 규칙(골격)이 코드인 이상 그 규칙으로 치는 자판도 코드에
//! 있어야 짝이 맞는다 — 배열을 고르는 일이 사전을 받는 일과 무관해지고, 배열이 요구하는
//! 골격이 이 빌드에 없는 경우가 아예 생기지 않는다.
//!
//! 배열을 늘리는 일은 파일 하나에 함수 하나를 더하고 그 언어의 `layouts()`에 한 줄을
//! 잇는 일이다. 대신 새 배열은 앱 판올림으로 나간다.

mod builder;
#[cfg(feature = "lang-hangul")]
mod cheonjiin;
#[cfg(feature = "lang-hangul")]
mod hangul;
#[cfg(feature = "lang-latin")]
mod latin;
mod shared;

use crate::keyboard::layout::{KeyboardLayoutSet, NamedLayoutSet};
use crate::lang::ComposerSkeleton;

fn named(name: &'static str, layouts: KeyboardLayoutSet) -> NamedLayoutSet {
    NamedLayoutSet {
        name,
        skeleton: None,
        layouts,
    }
}

/// 같은 언어 안에서 조합 규칙이 다른 배열은 자기 골격을 밝힌다 — 배열을 고르면 합성기가
/// 함께 갈린다.
fn named_with_skeleton(
    name: &'static str,
    skeleton: ComposerSkeleton,
    layouts: KeyboardLayoutSet,
) -> NamedLayoutSet {
    NamedLayoutSet {
        name,
        skeleton: Some(skeleton),
        layouts,
    }
}

/// 이 골격으로 칠 수 있는 배열들. 첫 항목이 기본 배열이고, 어느 것으로 칠지는 설정이
/// 정한다. 골격이 같으면 배열도 같으므로 언어 태그는 보지 않는다 — 팩이 새 언어를
/// 들고 와도 그 언어의 골격이 이미 있는 것이면 배열도 함께 온다.
pub fn for_skeleton(skeleton: ComposerSkeleton) -> Vec<NamedLayoutSet> {
    match skeleton {
        #[cfg(feature = "lang-hangul")]
        ComposerSkeleton::Hangul | ComposerSkeleton::HangulCheonjiin => hangul::layouts(),
        #[cfg(not(feature = "lang-hangul"))]
        ComposerSkeleton::Hangul | ComposerSkeleton::HangulCheonjiin => fallback(),
        ComposerSkeleton::Direct | ComposerSkeleton::Latin => {
            #[cfg(feature = "lang-latin")]
            {
                latin::layouts()
            }
            #[cfg(not(feature = "lang-latin"))]
            {
                fallback()
            }
        }
    }
}

/// 그 골격의 기본 배열 한 벌 — 배열을 고르는 자리가 아닌 곳(오타 합성 같은 오프라인
/// 계산)이 키 기하만 필요할 때 쓴다.
pub fn default_for(skeleton: ComposerSkeleton) -> KeyboardLayoutSet {
    for_skeleton(skeleton).swap_remove(0).layouts
}

/// 그 골격의 배열이 이 빌드에 없을 때 — 라틴이 가장 널리 통한다. 라틴마저 없으면 키가
/// 하나도 없는 판이 되므로, 프레임을 그리는 쪽이 빈 배열을 만나지 않도록 여기서 막는다.
#[cfg(any(not(feature = "lang-hangul"), not(feature = "lang-latin")))]
fn fallback() -> Vec<NamedLayoutSet> {
    #[cfg(feature = "lang-latin")]
    {
        latin::layouts()
    }
    #[cfg(not(feature = "lang-latin"))]
    {
        vec![named(
            "",
            KeyboardLayoutSet {
                layers: vec![builder::layer_of(vec![shared::bottom_row(0)])],
            },
        )]
    }
}
