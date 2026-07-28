//! 천지인 계열 네 벌. 조합 규칙이 두벌식과 다르므로(두벌식의 ㅏ 다음 ㅣ는 새 글자지만
//! 천지인에서는 ㅐ다) 이 판들만 자기 입력 방식을 밝힌다.
//!
//! 네 벌이 다른 것은 자음을 어떻게 나눠 담는가와 기능 열을 두는가뿐이다 — 어느 판으로
//! 치든 같은 자모에 닿고, 하늘·땅·사람의 조합 규칙은 하나다.
//!
//! 배치는 순정을 실측해 따랐다: 다섯 칸이 모두 같은 폭이고(키 폭 217px · 칸 간격
//! 235.75px, 네 줄 모두 동일), 엔터는 두 줄을 잇는다. 네 줄이 곧 키보드 전체 높이라
//! 다른 배열과 저절로 맞는다.

use crate::keyboard::layout::{LayoutRow, NamedLayoutSet};
use crate::lang::cheonjiin::CHEONJIIN;

use super::builder::*;
use super::named_with_method;
use super::shared::set_of;

/// 기능 열을 좌우에 두는 판의 격자 — 다섯 칸 균등
const NARROW_COLUMN: f32 = 0.2;
/// 자음을 한 글자씩 가른 판의 격자 — 여덟 칸 균등
const SPLIT_COLUMN: f32 = 0.125;
/// 왼쪽 기능 열을 걷어낸 판의 격자 — 네 칸 균등
const WIDE_COLUMN: f32 = 0.25;
/// 여덟 칸 격자에서 세 칸 — 넓은 판의 스페이스가 갖는 몫
const SPLIT_TRIPLE: f32 = SPLIT_COLUMN * 3.0;

fn cheonjiin(name: &'static str, letters: Vec<LayoutRow>) -> NamedLayoutSet {
    named_with_method(name, &CHEONJIIN, set_of(letters))
}

/// 표준 천지인 — 글자 세 열을 기능 키가 좌우에서 감싸는 네 줄 다섯 칸. 자음은 같은 키를
/// 이어 눌러 갈아 간다.
///
/// `cursor_right`는 커서를 오른쪽으로 옮기며 조합을 끊는다 — 같은 자음으로 시작하는
/// 글자를 잇달아 칠 때("가가") 멀티탭 시한이 다 되기를 기다리지 않고 손으로 끊는 길이다.
/// ABC·한글은 그 언어로 곧장 가는 키이고, 순정이 지구본과 마이크를 두는 왼쪽 아래 칸은
/// 통합 검색면이 쓴다.
fn standard() -> NamedLayoutSet {
    cheonjiin(
        "천지인",
        vec![
            uniform_row(
                NARROW_COLUMN,
                vec![
                    layer(1),
                    character('ㅣ'),
                    character('ㆍ'),
                    character('ㅡ'),
                    backspace(),
                ],
            ),
            uniform_row(
                NARROW_COLUMN,
                vec![
                    language_select("en", "ABC"),
                    multitap("ㄱㅋ"),
                    multitap("ㄴㄹ"),
                    multitap("ㄷㅌ"),
                    // 엔터는 순정처럼 두 줄을 잇는다 — 이어진 줄의 그 자리는 비워 둔다
                    enter().spanning(2),
                ],
            ),
            uniform_row(
                NARROW_COLUMN,
                vec![
                    language_select("ko", "한글"),
                    multitap("ㅂㅍ"),
                    multitap("ㅅㅎ"),
                    multitap("ㅈㅊ"),
                    blank(),
                ],
            ),
            uniform_row(
                NARROW_COLUMN,
                vec![
                    layer(3),
                    cursor_right(),
                    multitap("ㅇㅁ"),
                    multitap(".,?!"),
                    space(),
                ],
            ),
        ],
    )
}

/// 자음을 한 글자씩 갈라 놓은 판 — 손이 주기를 몇 번 돌리는지만 표준과 다르다(ㅋ을
/// 내려고 ㄱ을 두 번 치지 않는다). 구두점도 갈라 `.,`와 `?!` 두 키가 된다.
fn split() -> NamedLayoutSet {
    cheonjiin(
        "천지인+",
        vec![
            row(vec![
                layer(1).width(SPLIT_COLUMN),
                character('ㅣ').width(WIDE_COLUMN),
                character('ㆍ').width(WIDE_COLUMN),
                character('ㅡ').width(WIDE_COLUMN),
                backspace().width(SPLIT_COLUMN),
            ]),
            uniform_row(
                SPLIT_COLUMN,
                vec![
                    language_select("en", "ABC"),
                    character('ㄱ'),
                    character('ㅋ'),
                    character('ㄴ'),
                    character('ㄹ'),
                    character('ㄷ'),
                    character('ㅌ'),
                    enter().spanning(2),
                ],
            ),
            uniform_row(
                SPLIT_COLUMN,
                vec![
                    language_select("ko", "한글"),
                    character('ㅂ'),
                    character('ㅍ'),
                    character('ㅅ'),
                    character('ㅎ'),
                    character('ㅈ'),
                    character('ㅊ'),
                    blank(),
                ],
            ),
            row(vec![
                layer(3).width(SPLIT_COLUMN),
                cursor_right().width(SPLIT_COLUMN),
                character('ㅇ').width(SPLIT_COLUMN),
                character('ㅁ').width(SPLIT_COLUMN),
                multitap(".,").width(SPLIT_COLUMN),
                multitap("?!").width(SPLIT_COLUMN),
                space().width(WIDE_COLUMN),
            ]),
        ],
    )
}

/// 왼쪽 기능 열을 걷어내 글자 키를 넓게 쓰는 판. 언어를 곧장 고르던 ABC·한글 키가 그
/// 열과 함께 사라지므로 언어는 다음 언어로 돌리는 키가 맡고, 구두점은 오른쪽 끝으로 간다.
/// `cursor_right`는 자리가 없어 두지 않는다 — 같은 자음을 잇달아 칠 때는 멀티탭 시한이
/// 끊어 준다. 순정이 마이크를 두는 오른쪽 아래 칸은 통합 검색면이 쓴다.
fn wide() -> NamedLayoutSet {
    cheonjiin(
        "넓은 천지인",
        vec![
            uniform_row(
                WIDE_COLUMN,
                vec![
                    character('ㅣ'),
                    character('ㆍ'),
                    character('ㅡ'),
                    backspace(),
                ],
            ),
            uniform_row(
                WIDE_COLUMN,
                vec![
                    multitap("ㄱㅋ"),
                    multitap("ㄴㄹ"),
                    multitap("ㄷㅌ"),
                    enter(),
                ],
            ),
            uniform_row(
                WIDE_COLUMN,
                vec![
                    multitap("ㅂㅍ"),
                    multitap("ㅅㅎ"),
                    multitap("ㅈㅊ"),
                    multitap(".,?!"),
                ],
            ),
            // 아랫줄도 윗줄의 네 칸 격자에 얹는다 — 좁은 기능 키 둘이 한 칸을 나눠 갖고
            // 나머지는 한 칸씩이라, 어느 키도 윗줄의 칸 경계를 가로지르지 않는다
            row(vec![
                layer(1).width(SPLIT_COLUMN),
                language().width(SPLIT_COLUMN),
                multitap("ㅇㅁ").width(WIDE_COLUMN),
                space().width(WIDE_COLUMN),
                layer(3).width(WIDE_COLUMN),
            ]),
        ],
    )
}

/// 넓은 판에 자음을 한 글자씩 갈라 놓은 것 — 천지인+와 넓은 천지인의 규칙을 그대로 겹쳤다.
fn wide_split() -> NamedLayoutSet {
    cheonjiin(
        "넓은 천지인+",
        vec![
            uniform_row(
                WIDE_COLUMN,
                vec![
                    character('ㅣ'),
                    character('ㆍ'),
                    character('ㅡ'),
                    backspace(),
                ],
            ),
            row(vec![
                character('ㄱ').width(SPLIT_COLUMN),
                character('ㅋ').width(SPLIT_COLUMN),
                character('ㄴ').width(SPLIT_COLUMN),
                character('ㄹ').width(SPLIT_COLUMN),
                character('ㄷ').width(SPLIT_COLUMN),
                character('ㅌ').width(SPLIT_COLUMN),
                enter().width(WIDE_COLUMN),
            ]),
            uniform_row(
                SPLIT_COLUMN,
                vec![
                    character('ㅂ'),
                    character('ㅍ'),
                    character('ㅅ'),
                    character('ㅎ'),
                    character('ㅈ'),
                    character('ㅊ'),
                    multitap(".,"),
                    multitap("?!"),
                ],
            ),
            row(vec![
                layer(1).width(SPLIT_COLUMN),
                language().width(SPLIT_COLUMN),
                character('ㅇ').width(SPLIT_COLUMN),
                character('ㅁ').width(SPLIT_COLUMN),
                space().width(SPLIT_TRIPLE),
                layer(3).width(SPLIT_COLUMN),
            ]),
        ],
    )
}

pub(super) fn layouts() -> Vec<NamedLayoutSet> {
    vec![standard(), split(), wide(), wide_split()]
}
