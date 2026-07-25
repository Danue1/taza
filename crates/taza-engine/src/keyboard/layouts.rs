//! 내장 레이아웃. 장기적으로는 언어팩 데이터로 옮겨 코드 수정 없이 배열을 추가한다.
//! 심볼 1·2면과 길게 눌러 나오는 변형 문자는 iOS 순정 배열을 따른다
//! (빌트인 UX 계승 원칙).

use super::{KeyAction, KeyboardLayout, KeyboardLayoutSet, LayoutKey, LayoutRow};

/// 내장 배열은 모두 표준 높이 행이다 — 높이가 다른 행(숫자행 등)은 팩 데이터로 온다.
const STANDARD_ROW_HEIGHT: f32 = 1.0;
const LETTER_WIDTH: f32 = 0.1;
const CONTROL_WIDTH: f32 = 0.15;
/// 하단 행: 심볼 · 언어 · 스페이스 · 엔터
const LAYER_KEY_WIDTH: f32 = 0.125;
const LANGUAGE_KEY_WIDTH: f32 = 0.125;
const SPACE_WIDTH: f32 = 0.45;
const ENTER_WIDTH: f32 = 0.3;
/// 심볼면 셋째 행 — 순정은 양끝(#+=·⌫)을 가장자리에 붙이고 가운데 5키를 넓게 편다
const SYMBOL_PUNCTUATION_WIDTH: f32 = 0.14;

/// 길게 눌러 고르는 변형 문자. 없는 글자는 표에 넣지 않는다.
const LATIN_ALTERNATES: &[(char, &str)] = &[
    ('a', "àáâäæãåā"),
    ('c', "çćč"),
    ('e', "èéêëēėę"),
    ('i', "îïíīįì"),
    ('l', "ł"),
    ('n', "ñń"),
    ('o', "ôöòóœøōõ"),
    ('s', "ßśš"),
    ('u', "ûüùúū"),
    ('y', "ÿ"),
    ('z', "žźż"),
];

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

fn alternates_for(table: &[(char, &str)], base: char) -> Vec<char> {
    table
        .iter()
        .find(|(key, _)| *key == base)
        .map(|(_, characters)| characters.chars().collect())
        .unwrap_or_default()
}

fn character_key(base: char, shifted: char, alternates: Vec<char>) -> LayoutKey {
    LayoutKey {
        action: KeyAction::Character { base, shifted },
        width_ratio: LETTER_WIDTH,
        alternates,
    }
}

fn control_key(action: KeyAction, width_ratio: f32) -> LayoutKey {
    LayoutKey {
        action,
        width_ratio,
        alternates: Vec::new(),
    }
}

fn row(keys: Vec<LayoutKey>) -> LayoutRow {
    LayoutRow {
        keys,
        height_ratio: STANDARD_ROW_HEIGHT,
    }
}

fn latin_row(letters: &str) -> LayoutRow {
    row(letters
        .chars()
        .map(|base| {
            character_key(
                base,
                base.to_ascii_uppercase(),
                alternates_for(LATIN_ALTERNATES, base),
            )
        })
        .collect())
}

fn hangul_row(pairs: &[(char, char)]) -> LayoutRow {
    row(pairs
        .iter()
        .map(|&(base, shifted)| character_key(base, shifted, Vec::new()))
        .collect())
}

fn symbol_row(characters: &str) -> LayoutRow {
    symbol_row_with_width(characters, LETTER_WIDTH)
}

fn symbol_row_with_width(characters: &str, width_ratio: f32) -> LayoutRow {
    row(characters
        .chars()
        .map(|base| {
            let mut key = character_key(base, base, alternates_for(SYMBOL_ALTERNATES, base));
            key.width_ratio = width_ratio;
            key
        })
        .collect())
}

/// 하단 행 — 순서는 심볼 · 언어 · 스페이스 · 엔터로 고정한다.
fn bottom_row(switch_target: u8) -> LayoutRow {
    row(vec![
        control_key(
            KeyAction::LayerSwitch {
                target: switch_target,
            },
            LAYER_KEY_WIDTH,
        ),
        control_key(KeyAction::LanguageSwitch, LANGUAGE_KEY_WIDTH),
        control_key(KeyAction::Space, SPACE_WIDTH),
        control_key(KeyAction::Enter, ENTER_WIDTH),
    ])
}

fn shift_row(keys: Vec<LayoutKey>) -> LayoutRow {
    let mut all = vec![control_key(KeyAction::Shift, CONTROL_WIDTH)];
    all.extend(keys);
    all.push(control_key(KeyAction::Backspace, CONTROL_WIDTH));
    row(all)
}

/// iOS 순정 심볼 1면: 숫자·기본 기호. 셋째 행 왼쪽이 #+= 진입.
fn symbols_first_layer() -> KeyboardLayout {
    let mut third = vec![control_key(
        KeyAction::LayerSwitch { target: 2 },
        CONTROL_WIDTH,
    )];
    third.extend(symbol_row_with_width(".,?!'", SYMBOL_PUNCTUATION_WIDTH).keys);
    third.push(control_key(KeyAction::Backspace, CONTROL_WIDTH));
    KeyboardLayout {
        rows: vec![
            symbol_row("1234567890"),
            symbol_row("-/:;()$&@\""),
            row(third),
            bottom_row(0),
        ],
    }
}

/// iOS 순정 심볼 2면. 셋째 행 왼쪽이 123 복귀.
fn symbols_second_layer() -> KeyboardLayout {
    let mut third = vec![control_key(
        KeyAction::LayerSwitch { target: 1 },
        CONTROL_WIDTH,
    )];
    third.extend(symbol_row_with_width(".,?!'", SYMBOL_PUNCTUATION_WIDTH).keys);
    third.push(control_key(KeyAction::Backspace, CONTROL_WIDTH));
    KeyboardLayout {
        rows: vec![
            symbol_row("[]{}#%^*+="),
            symbol_row("_\\|~<>€£¥·"),
            row(third),
            bottom_row(0),
        ],
    }
}

fn with_symbol_layers(letters: KeyboardLayout) -> KeyboardLayoutSet {
    KeyboardLayoutSet {
        layers: vec![letters, symbols_first_layer(), symbols_second_layer()],
    }
}

pub fn qwerty() -> KeyboardLayoutSet {
    with_symbol_layers(KeyboardLayout {
        rows: vec![
            latin_row("qwertyuiop"),
            latin_row("asdfghjkl"),
            shift_row(latin_row("zxcvbnm").keys),
            bottom_row(1),
        ],
    })
}

pub fn dubeolsik() -> KeyboardLayoutSet {
    with_symbol_layers(KeyboardLayout {
        rows: vec![
            hangul_row(&[
                ('ㅂ', 'ㅃ'),
                ('ㅈ', 'ㅉ'),
                ('ㄷ', 'ㄸ'),
                ('ㄱ', 'ㄲ'),
                ('ㅅ', 'ㅆ'),
                ('ㅛ', 'ㅛ'),
                ('ㅕ', 'ㅕ'),
                ('ㅑ', 'ㅑ'),
                ('ㅐ', 'ㅒ'),
                ('ㅔ', 'ㅖ'),
            ]),
            hangul_row(&[
                ('ㅁ', 'ㅁ'),
                ('ㄴ', 'ㄴ'),
                ('ㅇ', 'ㅇ'),
                ('ㄹ', 'ㄹ'),
                ('ㅎ', 'ㅎ'),
                ('ㅗ', 'ㅗ'),
                ('ㅓ', 'ㅓ'),
                ('ㅏ', 'ㅏ'),
                ('ㅣ', 'ㅣ'),
            ]),
            shift_row(
                hangul_row(&[
                    ('ㅋ', 'ㅋ'),
                    ('ㅌ', 'ㅌ'),
                    ('ㅊ', 'ㅊ'),
                    ('ㅍ', 'ㅍ'),
                    ('ㅠ', 'ㅠ'),
                    ('ㅜ', 'ㅜ'),
                    ('ㅡ', 'ㅡ'),
                ])
                .keys,
            ),
            bottom_row(1),
        ],
    })
}
