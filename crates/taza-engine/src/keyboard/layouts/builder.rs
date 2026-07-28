//! 배열을 적는 도구. 배열은 격자이므로 코드로 적어도 격자로 읽혀야 한다 — 키 하나가
//! 낱말 하나, 행 하나가 줄 하나이고, 폭·변형 문자·세로 병합은 그 뒤에 덧붙는다.
//!
//! 폭을 적지 않은 키는 표준 글자 폭(0.1)이다. 행 전체가 같은 폭인 격자(천지인)는
//! `uniform_row`가 한 번에 정한다.

use crate::keyboard::layout::{KeyAction, KeyboardLayout, LayoutKey, LayoutRow, uppercase};

/// 글자 키 하나의 표준 폭 — 한 행에 열 칸이 서는 배열이 기준이다.
pub(super) const LETTER_WIDTH: f32 = 0.1;
/// 표준 행 높이 — 폼팩터가 정한 행 높이 그대로다.
const STANDARD_ROW_HEIGHT: f32 = 1.0;

impl LayoutKey {
    pub(super) fn width(mut self, ratio: f32) -> Self {
        self.width_ratio = ratio;
        self
    }

    /// 길게 눌러 고르는 변형 문자. 적은 순서가 팝업 표시 순서다.
    pub(super) fn alternates(mut self, characters: &str) -> Self {
        self.alternates = characters.chars().collect();
        self
    }

    /// 아래 행까지 한 칸으로 세운다 — 덮인 자리는 그 행에서 `blank`로 비워 둔다.
    pub(super) fn spanning(mut self, rows: u8) -> Self {
        self.row_span = rows;
        self
    }
}

impl LayoutRow {
    /// 표준 행 대비 높이. 행이 하나 더 필요한 배열(세벌식의 네 줄)이 눌러 담는 통로다.
    pub(super) fn height(mut self, ratio: f32) -> Self {
        self.height_ratio = ratio;
        self
    }
}

fn key(action: KeyAction) -> LayoutKey {
    LayoutKey {
        action,
        width_ratio: LETTER_WIDTH,
        row_span: 1,
        alternates: Vec::new(),
    }
}

/// 시프트를 켜면 대문자가 나오는 글자 키. 대소문자가 없는 스크립트에서는 같은 글자다.
pub(super) fn character(base: char) -> LayoutKey {
    character_pair(base, uppercase(base))
}

/// 시프트가 다른 글자를 내는 키 (한글 자모, 세벌식의 겹받침).
pub(super) fn character_pair(base: char, shifted: char) -> LayoutKey {
    key(KeyAction::Character { base, shifted })
}

/// 이어 누를 때마다 글자가 갈리는 키. 적은 순서가 주기 순서다.
pub(super) fn multitap(cycle: &str) -> LayoutKey {
    key(KeyAction::Multitap(cycle.chars().collect()))
}

pub(super) fn shift() -> LayoutKey {
    key(KeyAction::Shift)
}

pub(super) fn backspace() -> LayoutKey {
    key(KeyAction::Backspace)
}

pub(super) fn space() -> LayoutKey {
    key(KeyAction::Space)
}

pub(super) fn enter() -> LayoutKey {
    key(KeyAction::Enter)
}

/// 다음 언어로 돌리는 키.
pub(super) fn language() -> LayoutKey {
    key(KeyAction::LanguageSwitch)
}

/// 정해진 언어로 곧장 가는 키 — 키에 적히는 말도 배열이 함께 댄다.
pub(super) fn language_select(tag: &str, label: &str) -> LayoutKey {
    key(KeyAction::LanguageSelect {
        tag: tag.to_string(),
        label: label.to_string(),
    })
}

pub(super) fn layer(target: u8) -> LayoutKey {
    key(KeyAction::LayerSwitch { target })
}

pub(super) fn cursor_right() -> LayoutKey {
    key(KeyAction::CursorRight)
}

pub(super) fn blank() -> LayoutKey {
    key(KeyAction::Blank)
}

/// 글자 여럿을 한 줄로 — 변형 문자가 붙는 글자는 뒤에서 `alternates`로 덧붙인다.
pub(super) fn characters(text: &str) -> Vec<LayoutKey> {
    text.chars().map(character).collect()
}

/// 표에 적힌 변형 문자를 글자 키들에 입힌다. 배열 여럿이 같은 변형을 나눠 쓸 때
/// (라틴 네 벌, 심볼 두 면) 표가 단일 출처가 된다 — 배열마다 다시 적으면 한 배열에서만
/// 변형이 빠지는 드리프트가 생긴다.
pub(super) fn with_alternates(keys: Vec<LayoutKey>, table: &[(char, &str)]) -> Vec<LayoutKey> {
    keys.into_iter()
        .map(|key| match key.action {
            KeyAction::Character { base, .. } => match table.iter().find(|(at, _)| *at == base) {
                Some((_, characters)) => key.alternates(characters),
                None => key,
            },
            _ => key,
        })
        .collect()
}

pub(super) fn row(keys: Vec<LayoutKey>) -> LayoutRow {
    LayoutRow {
        keys,
        height_ratio: STANDARD_ROW_HEIGHT,
    }
}

/// 칸이 모두 같은 폭인 행 — 천지인처럼 격자가 균등한 판이 쓴다.
pub(super) fn uniform_row(width: f32, keys: Vec<LayoutKey>) -> LayoutRow {
    row(keys.into_iter().map(|key| key.width(width)).collect())
}

/// 키만 있는 보통 레이어.
pub(super) fn layer_of(rows: Vec<LayoutRow>) -> KeyboardLayout {
    KeyboardLayout {
        rows,
        panel_rows: 0.0,
    }
}
