//! 배열을 이루는 데이터 타입. 배열 자체는 `layouts`에 코드로 적혀 있다 — 조합 규칙이
//! 코드인 이상 그 규칙으로 치는 자판도 같은 자리에 있어야 하고, 그래야 배열을 고르는
//! 일이 사전을 받는 일과 무관해진다.
//!
//! 레이어 관례: 0 = 문자(언어별), 1 = 심볼 1면(숫자·기본 기호), 2 = 심볼 2면,
//! 3 = 통합 검색면(이모지·기호·얼굴 문자 — 키 대신 패널이 자리를 갖는다).

use crate::lang::ComposerSkeleton;

/// 글자 하나의 대문자. 대문자가 두 글자 이상이 되는 글자(ß→SS)는 키 한 칸에도 변형
/// 문자 팝업 한 칸에도 담을 수 없으므로 그대로 둔다 — 그런 글자는 배열이 시프트 표기를
/// 직접 적어야 한다.
pub fn uppercase(character: char) -> char {
    let mut upper = character.to_uppercase();
    match (upper.next(), upper.next()) {
        (Some(single), None) => single,
        _ => character,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    Character {
        base: char,
        shifted: char,
    },
    /// 한 번에 여러 글자를 넣는 키 (`.com` 등). 필드 성격이 불러오는 키가 주로 이것이라
    /// 히트 테스트의 이웃 확률에는 참여하지 않는다 — 어느 키인지가 이미 확실하다.
    Text(String),
    /// 이어 누를 때마다 글자가 갈리는 키 (천지인의 ㄱ→ㅋ→ㄲ). 무엇이 몇 번째인지는
    /// 배열이 정하고, 지금 몇 번째인지는 코어가 시간과 직전 키로 판정한다.
    Multitap(Vec<char>),
    Shift,
    Backspace,
    Space,
    Enter,
    LayerSwitch {
        target: u8,
    },
    /// 다음 언어로 전환. 언어 목록·순서는 셸이 소유하므로 코어는 요청만 낸다.
    LanguageSwitch,
    /// 정해진 언어로 곧장 전환 (천지인의 ABC·한글). 순환이 아니라 자리를 짚는 키라
    /// 어느 언어인지를 배열이 태그로 밝히고, 키에 적히는 말도 함께 적는다 — 코어는
    /// 자기가 쓰는 언어 하나만 알므로 다른 언어의 이름을 지어낼 길이 없다.
    LanguageSelect {
        tag: String,
        label: String,
    },
    /// 커서를 오른쪽으로 한 칸 옮긴다 — 천지인처럼 같은 키를 이어 눌러 글자를 갈아
    /// 끼우는 배열이 "여기서 끊는다"를 손으로 말하는 통로다. 옮기기 전에 조합은
    /// 확정되므로, 시한이 다 되기를 기다리지 않고 같은 자음을 잇달아 칠 수 있다.
    CursorRight,
    /// 자리만 차지하고 눌리지 않는 칸 — 숫자 패드 좌하단처럼 순정이 비워 두는 자리다.
    Blank,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutKey {
    pub action: KeyAction,
    pub width_ratio: f32,
    /// 이 키가 아래로 잇는 행 수. 1이 보통 키이고, 2면 다음 행까지 한 칸으로 선다
    /// (천지인의 큰 엔터). 이어진 행에서 그 자리는 `Blank`로 비워 둔다 — 행 폭이
    /// 유지되어야 나머지 키가 제자리에 선다.
    pub row_span: u8,
    /// 길게 눌러 고르는 변형 문자 (é, ¿ 등). 순서가 팝업 표시 순서다.
    pub alternates: Vec<char>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutRow {
    pub keys: Vec<LayoutKey>,
    /// 표준 행 높이(폼팩터가 정한다) 대비 배수. 1.0이 보통 행이고, 세벌식의 네 줄처럼
    /// 행이 하나 더 필요한 배열은 이 값을 눌러 담아 전체 높이를 맞춘다.
    pub height_ratio: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KeyboardLayout {
    pub rows: Vec<LayoutRow>,
    /// 키 위에 놓이는 패널(통합 검색면)의 높이 — 표준 행 대비 배수. 0이면 키만 있는
    /// 보통 레이어다. 패널 안을 무엇으로 채우는지는 배열이 아니라 코어가 정한다
    /// (검색어·최근 사용에 따라 달라지므로 배열에 담길 수 없다).
    pub panel_rows: f32,
}

/// 레이어 묶음 — 문자·심볼 등 전환 가능한 레이아웃들의 집합.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyboardLayoutSet {
    pub layers: Vec<KeyboardLayout>,
}

/// 이름이 붙은 배열 한 벌. 이름은 설정 화면에 그대로 나가므로 사람이 부르는 말이다
/// ("두벌식", "QWERTY").
#[derive(Debug, Clone, PartialEq)]
pub struct NamedLayoutSet {
    pub name: &'static str,
    /// 이 배열이 요구하는 조합 골격. 대개 비어 있고(언어가 밝힌 골격을 쓴다), 같은
    /// 언어 안에서 조합 규칙이 다른 배열(천지인)만 자기 골격을 밝힌다.
    pub skeleton: Option<ComposerSkeleton>,
    pub layouts: KeyboardLayoutSet,
}
