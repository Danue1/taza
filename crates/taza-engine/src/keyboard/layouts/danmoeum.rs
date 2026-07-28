//! 단모음 두 벌 — 두벌식에서 야행 모음(ㅑㅕㅛㅠ) 키를 걷어내고 여덟 칸으로 좁힌 판.
//! 걷어낸 모음은 밑모음을 이어 눌러 내고(ㅏ→ㅑ), 된소리는 판마다 다르다.
//!
//! 이 판들은 자기 입력 방식을 밝히지 않는다 — 천지인·나랏글이 방식을 가져야 했던 까닭
//! (두벌식에서 ㅏ 다음 ㅣ는 새 글자지만 그 배열들에서는 ㅐ다)이 여기에는 없다. ㅐ·ㅔ가
//! 제 키로 남아 있고 ㅘ·ㅢ는 두벌식 오토마타가 이미 만든다. 배열이 하는 일은 한 키에
//! 두 글자를 주기로 담는 것뿐이라 데이터로 끝난다.
//!
//! 키캡에 주기를 다 적지 않는 것도 순정을 따랐다: 천지인의 ㄱㅋ은 서로 다른 자모라 몇 번
//! 눌러야 무엇이 나오는지 적어야 하지만, 여기 주기는 밑글자를 세게(ㅂ→ㅃ) 또는 길게
//! (ㅏ→ㅑ) 만든 것이라 밑글자만 선다.
//!
//! 배치는 순정을 실측해 따랐다: 세 줄이 모두 여덟 칸 균등이고, 두벌식에서 둘째 줄에
//! 있던 ㅗ가 첫 줄로 올라간다.

use crate::keyboard::layout::{LayoutKey, LayoutRow, NamedLayoutSet};

use super::builder::*;
use super::named;
use super::shared::{bottom_row, set_of, shift_row};

/// 여덟 칸 균등 격자 — 세 줄이 모두 같은 폭이라 shift·backspace도 글자 폭으로 선다
const COLUMN: f32 = 0.125;

/// 이어 눌러 짝을 내는 키. 키캡에는 순정처럼 밑글자만 찍는다.
fn paired(cycle: &str) -> LayoutKey {
    let base: String = cycle.chars().take(1).collect();
    multitap(cycle).labeled(&base)
}

/// 시프트로 짝을 내는 키 — 두벌식과 같은 규칙이라 길게 눌러도 같은 글자에 닿는다.
fn shifted(base: char, pair: char) -> LayoutKey {
    character_pair(base, pair).alternates(&pair.to_string())
}

fn danmoeum_rows(first_row: Vec<LayoutKey>, third_row: LayoutRow) -> Vec<LayoutRow> {
    vec![
        uniform_row(COLUMN, first_row),
        uniform_row(
            COLUMN,
            vec![
                character('ㅁ'),
                character('ㄴ'),
                character('ㅇ'),
                character('ㄹ'),
                character('ㅎ'),
                paired("ㅓㅕ"),
                paired("ㅏㅑ"),
                character('ㅣ'),
            ],
        ),
        third_row,
        bottom_row(1),
    ]
}

/// 셋째 줄의 글자 여섯 — 두 판이 나눠 갖고, 왼쪽 끝에 무엇이 서는지만 다르다. 폭을 여기서
/// 정하는 것은 이 줄만 `uniform_row`를 거치지 않는 판(단모음+)이 있기 때문이다.
fn third_row_letters() -> Vec<LayoutKey> {
    [
        character('ㅋ'),
        character('ㅌ'),
        character('ㅊ'),
        character('ㅍ'),
        paired("ㅜㅠ"),
        character('ㅡ'),
    ]
    .map(|key| key.width(COLUMN))
    .to_vec()
}

/// 된소리까지 이어 눌러 내는 판 — 시프트가 없으므로 자음 짝도 주기에 담긴다.
fn danmoeum() -> NamedLayoutSet {
    let third_row = uniform_row(
        COLUMN,
        // 순정은 이 자리를 비워 둔다 — 시프트가 서는 자리이고 이 판에는 시프트가 없다
        std::iter::once(blank())
            .chain(third_row_letters())
            .chain(std::iter::once(backspace()))
            .collect(),
    );
    named(
        "단모음",
        set_of(danmoeum_rows(
            vec![
                paired("ㅂㅃ"),
                paired("ㅈㅉ"),
                paired("ㄷㄸ"),
                paired("ㄱㄲ"),
                paired("ㅅㅆ"),
                paired("ㅗㅛ"),
                paired("ㅐㅒ"),
                paired("ㅔㅖ"),
            ],
            third_row,
        )),
    )
}

/// 시프트를 되찾은 판 — 된소리와 ㅒ·ㅖ가 시프트로 오므로 그 키들은 이어 누르지 않는다.
/// 받침과 다음 초성이 같은 낱말("학교")에서 주기가 끼어들 자리가 없어지는 것이 이 판의
/// 값이다. 야행 모음은 시프트에 짝이 없어 여전히 이어 눌러 낸다.
fn danmoeum_with_shift() -> NamedLayoutSet {
    named(
        "단모음+",
        set_of(danmoeum_rows(
            vec![
                shifted('ㅂ', 'ㅃ'),
                shifted('ㅈ', 'ㅉ'),
                shifted('ㄷ', 'ㄸ'),
                shifted('ㄱ', 'ㄲ'),
                shifted('ㅅ', 'ㅆ'),
                paired("ㅗㅛ"),
                shifted('ㅐ', 'ㅒ'),
                shifted('ㅔ', 'ㅖ'),
            ],
            shift_row(third_row_letters(), COLUMN),
        )),
    )
}

pub(super) fn layouts() -> Vec<NamedLayoutSet> {
    vec![danmoeum(), danmoeum_with_shift()]
}
