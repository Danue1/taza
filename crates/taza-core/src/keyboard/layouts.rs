//! 내장 레이아웃. 장기적으로는 언어팩 데이터로 옮겨 코드 수정 없이 배열을 추가한다.

use super::{KeyAction, KeyboardLayout, LayoutKey, LayoutRow};

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

fn bottom_row() -> LayoutRow {
    LayoutRow {
        keys: vec![
            control_key(KeyAction::Space, 0.7),
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

pub fn qwerty() -> KeyboardLayout {
    let uppercase = |c: char| c.to_ascii_uppercase();
    let pairs = |letters: &str| -> Vec<(char, char)> {
        letters.chars().map(|c| (c, uppercase(c))).collect()
    };
    KeyboardLayout {
        rows: vec![
            character_row(&pairs("qwertyuiop")),
            character_row(&pairs("asdfghjkl")),
            third_row(&pairs("zxcvbnm")),
            bottom_row(),
        ],
    }
}

pub fn dubeolsik() -> KeyboardLayout {
    KeyboardLayout {
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
            bottom_row(),
        ],
    }
}
