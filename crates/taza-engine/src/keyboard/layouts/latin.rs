//! 라틴 배열 네 벌. 배열끼리 다른 것은 글자 배치뿐이므로 변형 문자는 표 하나에서
//! 오고, 심볼면·검색면은 공용 면을 그대로 물려받는다.

use crate::keyboard::layout::{LayoutKey, LayoutRow, NamedLayoutSet};

use super::builder::*;
use super::named;
use super::shared::{CONTROL_WIDTH, bottom_row, set_of, shift_row};

/// AZERTY 셋째 행은 글자가 여섯뿐이라 shift·backspace가 그 몫을 가져간다 — 넓히지
/// 않으면 두 키가 가장자리에서 떨어져 순정과 달리 양옆에 빈 틈이 생긴다.
const WIDE_CONTROL_WIDTH: f32 = 0.2;

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

fn letter_keys(text: &str) -> Vec<LayoutKey> {
    with_alternates(characters(text), LATIN_ALTERNATES)
}

fn letter_row(text: &str) -> LayoutRow {
    row(letter_keys(text))
}

pub(super) fn layouts() -> Vec<NamedLayoutSet> {
    vec![
        named(
            "QWERTY",
            set_of(vec![
                letter_row("qwertyuiop"),
                letter_row("asdfghjkl"),
                shift_row(letter_keys("zxcvbnm"), CONTROL_WIDTH),
                bottom_row(1),
            ]),
        ),
        named(
            "QWERTZ",
            set_of(vec![
                letter_row("qwertzuiop"),
                letter_row("asdfghjkl"),
                shift_row(letter_keys("yxcvbnm"), CONTROL_WIDTH),
                bottom_row(1),
            ]),
        ),
        named(
            "AZERTY",
            set_of(vec![
                letter_row("azertyuiop"),
                letter_row("qsdfghjklm"),
                shift_row(letter_keys("wxcvbn"), WIDE_CONTROL_WIDTH),
                bottom_row(1),
            ]),
        ),
        named(
            "Colemak",
            set_of(vec![
                letter_row("qwfpgjluy"),
                letter_row("arstdhneio"),
                shift_row(letter_keys("zxcvbkm"), CONTROL_WIDTH),
                bottom_row(1),
            ]),
        ),
    ]
}
