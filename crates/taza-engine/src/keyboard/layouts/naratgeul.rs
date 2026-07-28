//! 나랏글 한 벌. 조합 규칙이 두벌식과 다르므로(두벌식의 ㅏ 다음 ㅣ는 새 글자지만
//! 나랏글에서는 ㅐ다) 이 판은 자기 입력 방식을 밝힌다.
//!
//! 배치는 순정을 그대로 따랐다: 자음 여섯(ㄱㄴ / ㄹㅁ / ㅅㅇ)이 왼쪽 두 열에, 모음
//! 넷(ㅏㅓ · ㅗㅜ · ㅣ · ㅡ)이 오른쪽에 서고, 아랫줄을 획추가 · ㅡ · 쌍자음이 나눠 갖는다.
//! 나머지 자음은 키에 없다 — 획추가와 쌍자음으로 만드는 것이 이 방식이다.
//!
//! 격자는 천지인과 같은 네 줄 다섯 칸이라 두 판을 오가도 손이 같은 자리를 짚는다. 다만
//! 두 줄을 잇는 것이 천지인의 엔터가 아니라 스페이스이고, 엔터가 오른쪽 아래에 선다.

use crate::keyboard::layout::NamedLayoutSet;
use crate::lang::naratgeul::{NARATGEUL, STROKE, TENSE};

use super::builder::*;
use super::named_with_method;
use super::shared::set_of;

/// 다섯 칸 균등 — 천지인과 같은 격자다.
const COLUMN: f32 = 0.2;

/// 자음 키에는 밑글자만 선다 — 나머지 자음은 획추가·쌍자음으로 만드는 것이 이 방식이다.
/// 대신 그렇게 만들어지는 자음을 길게 눌러 고를 변형으로 달아 둔다: 어느 키에서 무엇이
/// 나오는지가 손에 익기 전에도 보이고, 두벌식이 시프트 짝을 변형으로 다는 것과 같은 길이다.
/// 모음 두 짝(ㅏㅓ·ㅗㅜ)은 이어 눌러 갈아 가므로 멀티탭이다.
///
/// 순정이 지구본과 마이크를 두는 왼쪽 아래 칸은 통합 검색면이 쓴다.
fn standard() -> NamedLayoutSet {
    named_with_method(
        "나랏글",
        &NARATGEUL,
        set_of(vec![
            uniform_row(
                COLUMN,
                vec![
                    layer(1),
                    character('ㄱ').alternates("ㅋㄲ"),
                    character('ㄴ').alternates("ㄷㅌㄸ"),
                    multitap("ㅏㅓ"),
                    backspace(),
                ],
            ),
            uniform_row(
                COLUMN,
                vec![
                    language_select("en", "ABC"),
                    character('ㄹ'),
                    character('ㅁ').alternates("ㅂㅍㅃ"),
                    multitap("ㅗㅜ"),
                    // 스페이스는 순정처럼 두 줄을 잇는다 — 이어진 줄의 그 자리는 비워 둔다
                    space().spanning(2),
                ],
            ),
            uniform_row(
                COLUMN,
                vec![
                    language_select("ko", "한글"),
                    character('ㅅ').alternates("ㅈㅊㅆㅉ"),
                    character('ㅇ').alternates("ㅎ"),
                    character('ㅣ'),
                    blank(),
                ],
            ),
            uniform_row(
                COLUMN,
                vec![
                    layer(3),
                    compose(STROKE, "획추가"),
                    character('ㅡ'),
                    compose(TENSE, "쌍자음"),
                    enter(),
                ],
            ),
        ]),
    )
}

pub(super) fn layouts() -> Vec<NamedLayoutSet> {
    vec![standard()]
}
