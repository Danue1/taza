//! 일본어 배열 두 벌 — 로마자와 12키 가나.
//!
//! 두 벌이 **다른 입력 방식을 가리킨다**. 로마자 판은 친 알파벳을 오토마톤이 가나로 옮기고,
//! 12키 판은 배열이 이미 가나를 내므로 옮길 것이 없다. 한 방식으로 둘을 겸하려면 배열이
//! 무엇을 냈는지를 합성기가 되짚어야 하는데, 그것은 배열이 데이터로 적은 것을 코드가 다시
//! 읽는 일이다.
//!
//! 12키 배치는 순정을 따랐다: 왼쪽 열에 기능, 가운데 세 열에 あ~わ행, 「小゛゜」가 아래
//! 가운데다. 같은 키를 이어 누르면 행을 돌고(あ→い→う→え→お), 그 값은 배열이 적는다.

use crate::keyboard::layout::NamedLayoutSet;
use crate::lang::japanese::{FORM_MARKER, KANA, ROMAJI};

use super::builder::*;
use super::named_with_method;
use super::shared::{CONTROL_WIDTH, bottom_row, set_of, shift_row};

/// 12키 판의 격자 — 다섯 칸 균등. 천지인과 같은 판이라 손이 옮겨 앉지 않는다.
const COLUMN: f32 = 0.2;

pub(crate) fn layouts() -> Vec<NamedLayoutSet> {
    vec![romaji(), kana()]
}

/// 로마자 — 데스크톱 자판 그대로다. 변형 문자를 두지 않는 까닭은 일본어에서 알파벳이
/// 글자가 아니라 **읽기를 적는 수단**이기 때문이다: é를 쳐도 갈 곳이 없다.
fn romaji() -> NamedLayoutSet {
    named_with_method(
        "ローマ字",
        &ROMAJI,
        set_of(vec![
            row(characters("qwertyuiop")),
            row(characters("asdfghjkl")),
            shift_row(characters("zxcvbnm"), CONTROL_WIDTH),
            bottom_row(1),
        ]),
    )
}

/// 12키 가나 — 순정의 「かな」 판. 플릭은 아직 없고 이어 누르기로만 행을 돈다.
///
/// 「小゛゜」는 자기 글자를 내지 않고 **직전 가나를 갈아 끼운다**(つ→っ→づ). 나랏글의 획
/// 추가와 같은 자리이므로 같은 계약(`KeyAction::Compose`)을 쓴다 — 코어는 표식을 평범한
/// 글자로 흘리고 그 뜻은 합성기만 안다.
fn kana() -> NamedLayoutSet {
    named_with_method(
        "かな",
        &KANA,
        set_of(vec![
            uniform_row(
                COLUMN,
                vec![
                    layer(1),
                    multitap("あいうえお"),
                    multitap("かきくけこ"),
                    multitap("さしすせそ"),
                    backspace(),
                ],
            ),
            uniform_row(
                COLUMN,
                vec![
                    language_select("en", "ABC"),
                    multitap("たちつてと"),
                    multitap("なにぬねの"),
                    multitap("はひふへほ"),
                    // 엔터는 순정처럼 두 줄을 잇는다 — 이어진 줄의 그 자리는 비워 둔다
                    enter().spanning(2),
                ],
            ),
            uniform_row(
                COLUMN,
                vec![
                    language_select("ja", "かな"),
                    multitap("まみむめも"),
                    multitap("やゆよ"),
                    multitap("らりるれろ"),
                    blank(),
                ],
            ),
            uniform_row(
                COLUMN,
                vec![
                    layer(3),
                    compose(FORM_MARKER, "小゛゜"),
                    multitap("わをん"),
                    multitap("、。？！"),
                    space(),
                ],
            ),
        ]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyboard::layout::KeyAction;

    /// 12키가 가나 오십음을 모두 낼 수 있어야 한다 — 어느 하나가 빠지면 그 글자를 칠 길이
    /// 아예 없다. 탁점·작은 글자는 「小゛゜」가 만들므로 여기서 세지 않는다.
    #[test]
    fn 십이키가_오십음을_모두_낸다() {
        let kana_layout = &kana().layouts.layers[0];
        let mut reachable: Vec<char> = Vec::new();
        for row in &kana_layout.rows {
            for key in &row.keys {
                if let KeyAction::Multitap(cycle) = &key.action {
                    reachable.extend(cycle);
                }
            }
        }
        for character in "あいうえおかきくけこさしすせそたちつてとなにぬねのはひふへほまみむめもやゆよらりるれろわをん".chars()
        {
            assert!(reachable.contains(&character), "{character}에 닿을 수 없다");
        }
    }
}
