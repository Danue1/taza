//! 한글 배열 — 두벌식과 세벌식 최종. 조합 규칙은 둘 다 `Hangul` 골격 하나가 맡는다:
//! 세벌식은 자리를 밝힌 조합용 자모를 내므로 합성기가 초성인지 종성인지를 추론할 일이
//! 없고, 그래서 새 골격이 아니라 배열 하나로 끝난다.

use crate::keyboard::layout::{LayoutKey, LayoutRow, NamedLayoutSet};

use super::builder::*;
use super::named;
use super::shared::{CONTROL_WIDTH, bottom_row, set_of, shift_row};

/// 세벌식은 숫자열 자리까지 네 줄이라 행 높이를 눌러 담아 다른 배열·다른 면과 키보드
/// 전체 높이를 맞춘다.
const SEBEOLSIK_ROW_HEIGHT: f32 = 0.75;

/// 두벌식 글자 행 — shift로 나오는 짝(ㄱ→ㄲ, ㅐ→ㅒ)이 곧 길게 눌러 고를 변형이다.
/// shift를 켜지 않고도 된소리·이중모음에 닿는 길을 준다.
fn dubeolsik_keys(pairs: &[(char, char)]) -> Vec<LayoutKey> {
    pairs
        .iter()
        .map(|&(base, shifted)| {
            let key = character_pair(base, shifted);
            if shifted == base {
                key
            } else {
                key.alternates(&shifted.to_string())
            }
        })
        .collect()
}

/// 세벌식 글자 행 — 시프트가 겹받침을 내므로 변형 문자는 두지 않는다. 짝이 없는 자리는
/// 같은 글자를 두 번 적는다.
fn sebeolsik_row(pairs: &[(char, char)]) -> LayoutRow {
    row(pairs
        .iter()
        .map(|&(base, shifted)| character_pair(base, shifted))
        .collect())
    .height(SEBEOLSIK_ROW_HEIGHT)
}

fn dubeolsik() -> NamedLayoutSet {
    named(
        "두벌식",
        set_of(vec![
            row(dubeolsik_keys(&[
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
            ])),
            row(dubeolsik_keys(&[
                ('ㅁ', 'ㅁ'),
                ('ㄴ', 'ㄴ'),
                ('ㅇ', 'ㅇ'),
                ('ㄹ', 'ㄹ'),
                ('ㅎ', 'ㅎ'),
                ('ㅗ', 'ㅗ'),
                ('ㅓ', 'ㅓ'),
                ('ㅏ', 'ㅏ'),
                ('ㅣ', 'ㅣ'),
            ])),
            shift_row(
                dubeolsik_keys(&[
                    ('ㅋ', 'ㅋ'),
                    ('ㅌ', 'ㅌ'),
                    ('ㅊ', 'ㅊ'),
                    ('ㅍ', 'ㅍ'),
                    ('ㅠ', 'ㅠ'),
                    ('ㅜ', 'ㅜ'),
                    ('ㅡ', 'ㅡ'),
                ]),
                CONTROL_WIDTH,
            ),
            bottom_row(1),
        ]),
    )
}

/// 세벌식 최종(3-91). 배치는 libhangul의 자판표(hangul-keyboard-3f)를 기준으로 삼았고,
/// 데스크톱에서 치던 손이 그대로 통하도록 숫자열 자리까지 네 줄을 싣는다. 시프트가
/// 기호를 내던 자리는 기호가 이미 심볼면에 있으므로 비웠다.
///
/// 글자는 자리를 밝힌 조합용 자모(초성 U+1100·중성 U+1161·종성 U+11A8)다. 키캡에 찍히는
/// 글자는 코어가 호환 자모로 옮겨 그린다. 각 행 위의 주석이 그 줄의 읽기다.
fn sebeolsik() -> NamedLayoutSet {
    named(
        "세벌식 최종",
        set_of(vec![
            // 종ㅎ/종ㄲ 종ㅆ/종ㄺ 종ㅂ/종ㅈ 중ㅛ/종ㄿ 중ㅠ/종ㄾ 중ㅑ 중ㅖ 중ㅢ 중ㅜ 초ㅋ
            sebeolsik_row(&[
                ('ᇂ', 'ᆩ'),
                ('ᆻ', 'ᆰ'),
                ('ᆸ', 'ᆽ'),
                ('ᅭ', 'ᆵ'),
                ('ᅲ', 'ᆴ'),
                ('ᅣ', 'ᅣ'),
                ('ᅨ', 'ᅨ'),
                ('ᅴ', 'ᅴ'),
                ('ᅮ', 'ᅮ'),
                ('ᄏ', 'ᄏ'),
            ]),
            // 종ㅅ/종ㅍ 종ㄹ/종ㅌ 중ㅕ/종ㄵ 중ㅐ/종ㅀ 중ㅓ/종ㄽ 초ㄹ 초ㄷ 초ㅁ 초ㅊ 초ㅍ
            sebeolsik_row(&[
                ('ᆺ', 'ᇁ'),
                ('ᆯ', 'ᇀ'),
                ('ᅧ', 'ᆬ'),
                ('ᅢ', 'ᆶ'),
                ('ᅥ', 'ᆳ'),
                ('ᄅ', 'ᄅ'),
                ('ᄃ', 'ᄃ'),
                ('ᄆ', 'ᄆ'),
                ('ᄎ', 'ᄎ'),
                ('ᄑ', 'ᄑ'),
            ]),
            // 종ㅇ/종ㄷ 종ㄴ/종ㄶ 중ㅣ/종ㄼ 중ㅏ/종ㄻ 중ㅡ/중ㅒ 초ㄴ 초ㅇ 초ㄱ 초ㅈ 초ㅂ
            sebeolsik_row(&[
                ('ᆼ', 'ᆮ'),
                ('ᆫ', 'ᆭ'),
                ('ᅵ', 'ᆲ'),
                ('ᅡ', 'ᆱ'),
                ('ᅳ', 'ᅤ'),
                ('ᄂ', 'ᄂ'),
                ('ᄋ', 'ᄋ'),
                ('ᄀ', 'ᄀ'),
                ('ᄌ', 'ᄌ'),
                ('ᄇ', 'ᄇ'),
            ]),
            // 종ㅁ/종ㅊ 종ㄱ/종ㅄ 중ㅔ/종ㅋ 중ㅗ/종ㄳ 중ㅜ 초ㅅ 초ㅎ
            shift_row(
                vec![
                    character_pair('ᆷ', 'ᆾ'),
                    character_pair('ᆨ', 'ᆹ'),
                    character_pair('ᅦ', 'ᆿ'),
                    character_pair('ᅩ', 'ᆪ'),
                    character('ᅮ'),
                    character('ᄉ'),
                    character('ᄒ'),
                ],
                CONTROL_WIDTH,
            )
            .height(SEBEOLSIK_ROW_HEIGHT),
            bottom_row(1),
        ]),
    )
}

pub(super) fn layouts() -> Vec<NamedLayoutSet> {
    let mut layouts = vec![dubeolsik(), sebeolsik()];
    layouts.extend(super::cheonjiin::layouts());
    layouts
}
