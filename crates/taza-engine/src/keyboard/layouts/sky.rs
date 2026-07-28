//! 베가(SKY-II) 한 벌. 조합 규칙이 두벌식과 다르므로(두벌식의 ㅏ 다음 ㅣ는 새 글자지만
//! 베가에서는 ㅐ다) 이 판은 자기 입력 방식을 밝힌다.
//!
//! 배치는 순정을 그대로 따랐다: 자음 여덟과 모음 넷이 열두 칸을 채우고, 기능 열이 좌우에
//! 선다. 격자는 천지인·나랏글과 같은 네 줄 다섯 칸이라 판을 오가도 손이 같은 자리를 짚는다.
//!
//! 커서를 옮기는 키는 순정에 없다 — 열두 칸이 모두 글자라 자리가 없다. 받침과 다음 초성이
//! 같은 키인 낱말("안남미")은 스페이스바를 밀어 커서로 끊는다.
//!
//! 순정이 지구본과 마이크를 두는 왼쪽 아래 칸은 통합 검색면이 쓴다.

use crate::keyboard::layout::{LayoutKey, NamedLayoutSet};
use crate::lang::sky::SKY;

use super::builder::*;
use super::named_with_method;
use super::shared::set_of;

/// 다섯 칸 균등 — 천지인·나랏글과 같은 격자다.
const COLUMN: f32 = 0.2;

/// 된소리까지 이어 눌러 내는 자음 키. 키캡에는 순정처럼 서로 다른 자모 둘만 찍고 된소리는
/// 세 번째 타건에 숨긴다 — 된소리는 밑글자를 세게 만든 것이라 적히지 않아도 손이 찾는다
/// (단모음과 같은 규칙). 시프트가 없는 판이므로 된소리에 닿는 길은 이것뿐이다.
fn tense(cycle: &str) -> LayoutKey {
    let visible: String = cycle.chars().take(2).collect();
    multitap(cycle).labeled(&visible)
}

fn standard() -> NamedLayoutSet {
    named_with_method(
        "베가",
        &SKY,
        set_of(vec![
            uniform_row(
                COLUMN,
                vec![
                    layer(1),
                    tense("ㄱㅋㄲ"),
                    multitap("ㅣㅡ"),
                    multitap("ㅏㅑ"),
                    backspace(),
                ],
            ),
            uniform_row(
                COLUMN,
                vec![
                    language_select("en", "ABC"),
                    tense("ㄷㅌㄸ"),
                    multitap("ㄴㄹ"),
                    multitap("ㅓㅕ"),
                    // 엔터는 순정처럼 두 줄을 잇는다 — 이어진 줄의 그 자리는 비워 둔다
                    enter().spanning(2),
                ],
            ),
            uniform_row(
                COLUMN,
                vec![
                    language_select("ko", "한글"),
                    tense("ㅁㅅㅆ"),
                    tense("ㅂㅍㅃ"),
                    multitap("ㅗㅛ"),
                    blank(),
                ],
            ),
            uniform_row(
                COLUMN,
                vec![
                    layer(3),
                    tense("ㅈㅊㅉ"),
                    multitap("ㅇㅎ"),
                    multitap("ㅜㅠ"),
                    space(),
                ],
            ),
        ]),
    )
}

pub(super) fn layouts() -> Vec<NamedLayoutSet> {
    vec![standard()]
}
