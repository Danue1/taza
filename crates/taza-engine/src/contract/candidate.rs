//! 후보 바와 통합 검색면에 서는 것들. 헤더 문구는 여기에 없다 — 계약이 나르는 것은
//! 신원(갈래·묶음)이지 어느 나라 말이 아니고, 그것을 무엇으로 적을지는 셸이 정한다.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateKind {
    /// 친 그대로의 원문. 교정·완성을 물리고 원문을 지키는 길이며 언제나 첫 자리다.
    Typed,
    Prediction,
    Conversion,
    Correction,
}

/// 후보 바에서 이 후보가 서는 자리. 셸은 갈래별로 묶어(그룹 단위) 한 줄에 인라인으로
/// 늘어놓고, 통합 검색도 같은 갈래로 결과를 나눈다. `kind`가 "어떻게 얻은 후보인가"라면
/// 이쪽은 "무엇으로서 보이는가"다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateGroup {
    Word,
    Emoji,
    Symbol,
    /// (^_^)·orz처럼 글자로 그린 얼굴
    Emoticon,
}

impl CandidateGroup {
    /// annotation 섹션의 와이어 태그. 낱말은 그 표에 담기지 않으므로 태그가 없다.
    pub fn tag(self) -> Option<u8> {
        match self {
            CandidateGroup::Word => None,
            CandidateGroup::Emoji => Some(1),
            CandidateGroup::Symbol => Some(2),
            CandidateGroup::Emoticon => Some(3),
        }
    }

    pub fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(CandidateGroup::Emoji),
            2 => Some(CandidateGroup::Symbol),
            3 => Some(CandidateGroup::Emoticon),
            _ => None,
        }
    }

    /// 후보 바·검색 결과에 나오는 순서. 낱말이 먼저이고 곁들이는 것이 뒤따른다.
    pub const DISPLAY_ORDER: [CandidateGroup; 4] = [
        CandidateGroup::Word,
        CandidateGroup::Emoji,
        CandidateGroup::Symbol,
        CandidateGroup::Emoticon,
    ];
}

/// 이모지가 검색면에서 서는 묶음. 갈래(CandidateGroup)가 "무엇으로서 보이는가"라면 이쪽은
/// 이모지 안에서의 자리다. 묶음과 순서는 빌트인 키보드 관례를 그대로 따른다(계승 원칙) —
/// 유니코드 정본 순서와 달리 활동이 여행보다 앞이고, 사람은 스마일리와 한 묶음이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmojiCategory {
    SmileysAndPeople,
    AnimalsAndNature,
    FoodAndDrink,
    Activities,
    TravelAndPlaces,
    Objects,
    Symbols,
    Flags,
}

impl EmojiCategory {
    /// annotation catalog 섹션의 와이어 태그. 0은 "묶음 없음"이라 1부터 쓴다.
    pub fn tag(self) -> u8 {
        match self {
            EmojiCategory::SmileysAndPeople => 1,
            EmojiCategory::AnimalsAndNature => 2,
            EmojiCategory::FoodAndDrink => 3,
            EmojiCategory::Activities => 4,
            EmojiCategory::TravelAndPlaces => 5,
            EmojiCategory::Objects => 6,
            EmojiCategory::Symbols => 7,
            EmojiCategory::Flags => 8,
        }
    }

    pub fn from_tag(tag: u8) -> Option<Self> {
        Self::DISPLAY_ORDER
            .into_iter()
            .find(|category| category.tag() == tag)
    }

    pub const DISPLAY_ORDER: [EmojiCategory; 8] = [
        EmojiCategory::SmileysAndPeople,
        EmojiCategory::AnimalsAndNature,
        EmojiCategory::FoodAndDrink,
        EmojiCategory::Activities,
        EmojiCategory::TravelAndPlaces,
        EmojiCategory::Objects,
        EmojiCategory::Symbols,
        EmojiCategory::Flags,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub text: String,
    pub kind: CandidateKind,
    pub group: CandidateGroup,
}

/// 통합 검색면(이모지·기호·얼굴 문자)에 담기는 항목 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationPanelItem {
    pub group: CandidateGroup,
    pub text: String,
}

/// 검색면의 한 그룹. 그룹 단위로 인라인 배치되므로 셸은 헤더와 항목만 그린다.
/// 헤더 문구는 싣지 않는다 — 갈래(`group`)와 묶음(`category`)이 곧 신원이고, 그것을
/// 어느 나라 말로 적을지는 화면의 일이다. 둘 다 비면 최근에 고른 것들이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationPanelGroup {
    /// 이 그룹의 갈래. 최근 사용처럼 갈래가 섞이는 그룹은 None이다.
    pub group: Option<CandidateGroup>,
    /// 이모지 묶음이면 그 자리 — 셸이 묶음마다 다른 표식을 세우는 통로다.
    pub category: Option<EmojiCategory>,
    pub items: Vec<AnnotationPanelItem>,
}

/// 검색면 내용. 검색어가 없으면 자주 쓰는 것과 갈래별 표시 순서가 담긴다.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnnotationPanel {
    pub groups: Vec<AnnotationPanelGroup>,
}
