//! 필드 성격이 배열에 하는 일. 순정 키보드는 필드마다 배열을 통째로 갈아 끼우지 않고
//! (숫자 계열만 예외) 하단 행 한두 키와 리턴키만 바꾼다 — 손 위치를 다시 배우지 않게
//! 하려는 것이고, 실측 근거는 `docs/inputmode.md`에 있다.
//!
//! 여기서 만든 배열은 프레임과 히트 테스트가 함께 쓴다. 필드가 화면을 바꾸는 규칙이
//! 한 곳에만 있어야 눌린 자리와 그려진 자리가 어긋나지 않는다.

use crate::contract::FieldKind;
use crate::keyboard::layout::{KeyAction, KeyboardLayout, LayoutKey, LayoutRow};

/// 숫자 패드 한 칸의 폭 — 순정처럼 3열이다.
const NUMBER_PAD_COLUMN: f32 = 1.0 / 3.0;
/// 이메일 필드가 스페이스 양옆에 끼우는 키의 폭 (순정 실측)
const EMAIL_AFFIX_WIDTH: f32 = 0.1;
/// URL 필드의 `.com` 키 — 스페이스가 사라진 자리를 나눠 갖는다
const URL_DOMAIN_WIDTH: f32 = 0.175;
/// 통합 검색면 레이어 — 비밀번호 필드에서 걷어내는 이모지 키의 대상이다
const PANEL_LAYER: u8 = 3;

fn control(action: KeyAction, width_ratio: f32) -> LayoutKey {
    LayoutKey {
        action,
        width_ratio,
        row_span: 1,
        alternates: Vec::new(),
    }
}

fn character(base: char, width_ratio: f32) -> LayoutKey {
    LayoutKey {
        action: KeyAction::Character {
            base,
            shifted: base,
        },
        width_ratio,
        row_span: 1,
        alternates: Vec::new(),
    }
}

/// 숫자 계열 필드의 배열. 언어와 무관하므로 언어팩이 아니라 코어가 갖는다 —
/// 숫자 패드에는 배열마다 달라질 글자가 없다.
fn number_pad(field: FieldKind) -> KeyboardLayout {
    let row = |keys: Vec<LayoutKey>| LayoutRow {
        keys,
        height_ratio: 1.0,
    };
    let digits = |characters: &str| {
        row(characters
            .chars()
            .map(|digit| character(digit, NUMBER_PAD_COLUMN))
            .collect())
    };
    // 좌하단만 필드마다 다르다 (순정 실측: 숫자=빈칸, 소수=`.`, 전화=`+ * #`)
    let leading = match field {
        FieldKind::Decimal => character('.', NUMBER_PAD_COLUMN),
        FieldKind::Phone => control(KeyAction::Text("+*#".to_string()), NUMBER_PAD_COLUMN),
        _ => control(KeyAction::Blank, NUMBER_PAD_COLUMN),
    };
    KeyboardLayout {
        panel_rows: 0.0,
        rows: vec![
            digits("123"),
            digits("456"),
            digits("789"),
            row(vec![
                leading,
                character('0', NUMBER_PAD_COLUMN),
                control(KeyAction::Backspace, NUMBER_PAD_COLUMN),
            ]),
        ],
    }
}

/// 스페이스 키가 있는 행에서 그 자리를 필드 전용 키들로 바꾼다. 폭의 합은 유지되므로
/// 행 전체 폭이 흐트러지지 않는다.
fn replace_space(row: &LayoutRow, replacement: Vec<LayoutKey>) -> LayoutRow {
    let mut keys = Vec::with_capacity(row.keys.len() + replacement.len());
    for key in &row.keys {
        if key.action == KeyAction::Space {
            keys.extend(replacement.iter().cloned());
        } else {
            keys.push(key.clone());
        }
    }
    LayoutRow {
        keys,
        height_ratio: row.height_ratio,
    }
}

/// 비밀번호 필드는 이모지 키와 언어 키를 걷어낸다(순정 실측). 걷어낸 폭은 스페이스가
/// 가져가 행 폭이 유지된다.
fn strip_for_password(row: &LayoutRow) -> LayoutRow {
    let stripped = |key: &LayoutKey| {
        key.action == KeyAction::LanguageSwitch
            || matches!(key.action, KeyAction::LayerSwitch { target } if target == PANEL_LAYER)
    };
    let removed: f32 = row
        .keys
        .iter()
        .filter(|key| stripped(key))
        .map(|key| key.width_ratio)
        .sum();
    let keys = row
        .keys
        .iter()
        .filter(|key| !stripped(key))
        .map(|key| {
            let mut key = key.clone();
            if key.action == KeyAction::Space {
                key.width_ratio += removed;
            }
            key
        })
        .collect();
    LayoutRow {
        keys,
        height_ratio: row.height_ratio,
    }
}

/// 필드에 맞춰 배열을 고친다. 패널이 있는 레이어(통합 검색면)는 건드리지 않는다 —
/// 그 자리는 필드가 아니라 검색면이 소유한다.
pub(crate) fn apply(layout: &KeyboardLayout, field: FieldKind) -> KeyboardLayout {
    if uses_number_pad(field) {
        return number_pad(field);
    }
    if layout.panel_rows > 0.0 {
        return layout.clone();
    }
    let rows = layout
        .rows
        .iter()
        .map(|row| match field {
            FieldKind::Email => replace_space(
                row,
                vec![
                    character('@', EMAIL_AFFIX_WIDTH),
                    control(
                        KeyAction::Space,
                        (row_space_width(row) - EMAIL_AFFIX_WIDTH * 2.0).max(0.1),
                    ),
                    character('.', EMAIL_AFFIX_WIDTH),
                ],
            ),
            FieldKind::Url => replace_space(
                row,
                vec![
                    character('.', EMAIL_AFFIX_WIDTH),
                    character('/', EMAIL_AFFIX_WIDTH),
                    control(KeyAction::Text(".com".to_string()), URL_DOMAIN_WIDTH),
                ],
            ),
            FieldKind::Password => strip_for_password(row),
            _ => row.clone(),
        })
        .collect();
    KeyboardLayout {
        rows,
        panel_rows: layout.panel_rows,
    }
}

/// 그 행의 스페이스 폭 — 없으면 0이라 치환도 일어나지 않는다.
fn row_space_width(row: &LayoutRow) -> f32 {
    row.keys
        .iter()
        .find(|key| key.action == KeyAction::Space)
        .map(|key| key.width_ratio)
        .unwrap_or(0.0)
}

/// 배열을 통째로 숫자 패드로 가는 필드인가. 이 필드들에는 글자 행이 없으므로 문자면을
/// 전제한 손질(숫자 행 추가 등)이 끼어들 자리도 없다.
pub(crate) fn uses_number_pad(field: FieldKind) -> bool {
    matches!(
        field,
        FieldKind::Number | FieldKind::Decimal | FieldKind::Phone
    )
}

/// 필드가 후보 바 자리를 갖는가. 순정은 이메일·URL·숫자 필드에서 예측 바 영역 자체를
/// 없애 키보드를 낮추고, 비밀번호 필드에서는 자리를 남긴다.
pub(crate) fn shows_candidate_bar(field: FieldKind) -> bool {
    matches!(
        field,
        FieldKind::Text | FieldKind::Search | FieldKind::Password
    )
}
