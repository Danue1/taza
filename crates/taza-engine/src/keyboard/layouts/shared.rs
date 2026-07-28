//! 어느 배열이나 함께 쓰는 면들 — 심볼 1·2면, 통합 검색면, 그리고 그 셋과 문자면이
//! 나눠 쓰는 하단 행. 배열끼리 다른 것은 대개 문자면뿐이므로 나머지는 여기 한 벌만 둔다.
//!
//! 심볼 배치와 변형 문자는 iOS 순정을 따른다(빌트인 UX 계승 원칙).

use crate::keyboard::layout::{KeyboardLayout, KeyboardLayoutSet, LayoutKey, LayoutRow};

use super::builder::*;

/// 하단 행: 심볼 · 이모지 · 언어 · 스페이스 · 엔터 (순정 실측 폭)
const LAYER_KEY_WIDTH: f32 = 0.115;
const SPACE_WIDTH: f32 = 0.375;
const ENTER_WIDTH: f32 = 0.28;
/// shift·backspace가 가장자리에서 갖는 폭
pub(super) const CONTROL_WIDTH: f32 = 0.15;
/// 심볼면 셋째 행 — 순정은 양끝(#+=·⌫)을 가장자리에 붙이고 가운데 다섯 키를 넓게 편다
const PUNCTUATION_WIDTH: f32 = 0.14;
/// 통합 검색면 레이어 번호
pub(super) const PANEL_LAYER: u8 = 3;
/// 통합 검색면 패널이 차지하는 높이 — 표준 행 셋. 하단 행까지 더하면 문자면과 같은
/// 높이가 되어 레이어를 넘나들 때 키보드가 커지거나 작아지지 않는다.
const PANEL_ROWS: f32 = 3.0;
/// 통합 검색면 하단 행: 문자 복귀 · (빈자리) · 삭제. 순정 이모지 화면처럼 낱말을 치는
/// 키(스페이스·엔터)는 두지 않는다 — 이 면에서 하는 일은 고르는 것뿐이다.
const PANEL_RETURN_WIDTH: f32 = 0.2;
const PANEL_GAP_WIDTH: f32 = 0.6;

const SYMBOL_ALTERNATES: &[(char, &str)] = &[
    ('0', "°"),
    ('-', "–—•"),
    ('/', "\\"),
    ('$', "₩¢£¥€"),
    ('&', "§"),
    ('"', "\u{201C}\u{201D}«»„"),
    ('\'', "\u{2018}\u{2019}`"),
    ('(', "[{<"),
    (')', "]}>"),
    ('.', "…"),
    ('?', "¿"),
    ('!', "¡"),
    ('%', "‰"),
    ('=', "≠≈"),
];

fn symbol_row(characters_of_row: &str) -> LayoutRow {
    row(with_alternates(
        characters(characters_of_row),
        SYMBOL_ALTERNATES,
    ))
}

/// 심볼 두 면이 함께 쓰는 셋째 행 — 왼쪽 키만 다른 면을 가리킨다.
fn punctuation_row(switch_target: u8) -> LayoutRow {
    let mut keys = vec![layer(switch_target).width(CONTROL_WIDTH)];
    keys.extend(
        with_alternates(characters(".,?!'"), SYMBOL_ALTERNATES)
            .into_iter()
            .map(|key| key.width(PUNCTUATION_WIDTH)),
    );
    keys.push(backspace().width(CONTROL_WIDTH));
    row(keys)
}

/// 하단 행 — 순서는 심볼 · 이모지 · 언어 · 스페이스 · 엔터로 고정한다. 이모지 키는
/// 낱말을 치지 않고도 통합 검색면에 한 번에 닿는 길이다.
pub(super) fn bottom_row(switch_target: u8) -> LayoutRow {
    row(vec![
        layer(switch_target).width(LAYER_KEY_WIDTH),
        layer(PANEL_LAYER).width(LAYER_KEY_WIDTH),
        language().width(LAYER_KEY_WIDTH),
        space().width(SPACE_WIDTH),
        enter().width(ENTER_WIDTH),
    ])
}

/// 가장자리에 shift·backspace가 서는 글자 행. 두 키의 폭은 글자 수가 정한다 — 행이
/// 폭 1에 닿지 않으면 가운데로 밀려 순정에는 없는 틈이 가장자리에 생긴다.
pub(super) fn shift_row(letters: Vec<LayoutKey>, control_width: f32) -> LayoutRow {
    let mut keys = vec![shift().width(control_width)];
    keys.extend(letters);
    keys.push(backspace().width(control_width));
    row(keys)
}

/// 심볼 1면: 숫자·기본 기호. 셋째 행 왼쪽이 #+= 진입.
fn symbols_first() -> KeyboardLayout {
    layer_of(vec![
        symbol_row("1234567890"),
        symbol_row("-/:;()$&@\""),
        punctuation_row(2),
        bottom_row(0),
    ])
}

/// 심볼 2면. 셋째 행 왼쪽이 123 복귀.
fn symbols_second() -> KeyboardLayout {
    layer_of(vec![
        symbol_row("[]{}#%^*+="),
        symbol_row("_\\|~<>€£¥·"),
        punctuation_row(1),
        bottom_row(0),
    ])
}

/// 통합 검색면 — 키는 하단 행 하나뿐이고 나머지 자리는 패널이 갖는다. 패널 내용은
/// 검색어·최근 사용에 따라 달라지므로 배열이 아니라 코어가 정한다.
fn annotation_panel() -> KeyboardLayout {
    KeyboardLayout {
        rows: vec![row(vec![
            layer(0).width(PANEL_RETURN_WIDTH),
            blank().width(PANEL_GAP_WIDTH),
            backspace().width(PANEL_RETURN_WIDTH),
        ])],
        panel_rows: PANEL_ROWS,
    }
}

/// 문자면 행들에 공용 면 셋을 붙여 배열 한 벌로 만든다 — 배열이 스스로 적는 것은
/// 문자면뿐이다.
pub(super) fn set_of(letters: Vec<LayoutRow>) -> KeyboardLayoutSet {
    KeyboardLayoutSet {
        layers: vec![
            layer_of(letters),
            symbols_first(),
            symbols_second(),
            annotation_panel(),
        ],
    }
}
