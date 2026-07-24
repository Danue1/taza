//! 내장 레이아웃. 장기적으로는 언어팩 데이터로 옮겨 코드 수정 없이 배열을 추가한다.
//! 심볼 1·2면은 iOS 순정 배열을 따른다 (빌트인 UX 계승 원칙).

use super::{KeyAction, KeyboardLayout, KeyboardLayoutSet, LayoutKey, LayoutRow};

const LETTER_WIDTH: f32 = 0.1;
const CONTROL_WIDTH: f32 = 0.15;

fn character_key(base: char, shifted: char) -> LayoutKey {
    LayoutKey {
        action: KeyAction::Character { base, shifted },
        width_ratio: LETTER_WIDTH,
    }
}

fn control_key(action: KeyAction, width_ratio: f32) -> LayoutKey {
    LayoutKey {
        action,
        width_ratio,
    }
}

fn character_row(pairs: &[(char, char)]) -> LayoutRow {
    LayoutRow {
        keys: pairs
            .iter()
            .map(|&(base, shifted)| character_key(base, shifted))
            .collect(),
    }
}

fn simple_row(characters: &str) -> LayoutRow {
    LayoutRow {
        keys: characters.chars().map(|c| character_key(c, c)).collect(),
    }
}

/// 하단 행: 문자면 복귀 또는 심볼 진입 키 + 스페이스 + 엔터
fn bottom_row(switch_target: u8) -> LayoutRow {
    LayoutRow {
        keys: vec![
            control_key(KeyAction::LayerSwitch { target: switch_target }, 0.15),
            control_key(KeyAction::Space, 0.55),
            control_key(KeyAction::Enter, 0.3),
        ],
    }
}

fn third_row(pairs: &[(char, char)]) -> LayoutRow {
    let mut keys = vec![control_key(KeyAction::Shift, CONTROL_WIDTH)];
    keys.extend(
        pairs
            .iter()
            .map(|&(base, shifted)| character_key(base, shifted)),
    );
    keys.push(control_key(KeyAction::Backspace, CONTROL_WIDTH));
    LayoutRow { keys }
}

/// iOS 순정 심볼 1면: 숫자·기본 기호. 셋째 행 왼쪽이 #+= 진입.
fn symbols_first_layer() -> KeyboardLayout {
    let mut third = vec![control_key(KeyAction::LayerSwitch { target: 2 }, CONTROL_WIDTH)];
    third.extend(".,?!'".chars().map(|c| character_key(c, c)));
    third.push(control_key(KeyAction::Backspace, CONTROL_WIDTH));
    KeyboardLayout {
        rows: vec![
            simple_row("1234567890"),
            simple_row("-/:;()$&@\""),
            LayoutRow { keys: third },
            bottom_row(0),
        ],
    }
}

/// iOS 순정 심볼 2면. 셋째 행 왼쪽이 123 복귀.
fn symbols_second_layer() -> KeyboardLayout {
    let mut third = vec![control_key(KeyAction::LayerSwitch { target: 1 }, CONTROL_WIDTH)];
    third.extend(".,?!'".chars().map(|c| character_key(c, c)));
    third.push(control_key(KeyAction::Backspace, CONTROL_WIDTH));
    KeyboardLayout {
        rows: vec![
            simple_row("[]{}#%^*+="),
            simple_row("_\\|~<>€£¥·"),
            LayoutRow { keys: third },
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
    let uppercase = |c: char| c.to_ascii_uppercase();
    let pairs = |letters: &str| -> Vec<(char, char)> {
        letters.chars().map(|c| (c, uppercase(c))).collect()
    };
    with_symbol_layers(KeyboardLayout {
        rows: vec![
            character_row(&pairs("qwertyuiop")),
            character_row(&pairs("asdfghjkl")),
            third_row(&pairs("zxcvbnm")),
            bottom_row(1),
        ],
    })
}

pub fn dubeolsik() -> KeyboardLayoutSet {
    with_symbol_layers(KeyboardLayout {
        rows: vec![
            character_row(&[
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
            character_row(&[
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
            third_row(&[
                ('ㅋ', 'ㅋ'),
                ('ㅌ', 'ㅌ'),
                ('ㅊ', 'ㅊ'),
                ('ㅍ', 'ㅍ'),
                ('ㅠ', 'ㅠ'),
                ('ㅜ', 'ㅜ'),
                ('ㅡ', 'ㅡ'),
            ]),
            bottom_row(1),
        ],
    })
}
