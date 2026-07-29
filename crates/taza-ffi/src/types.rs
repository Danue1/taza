//! 셸이 보는 타입들. 코어 계약(`taza_engine::contract`)의 거울이며, 옮기는 일은
//! `convert`가 한다 — 선언과 번역을 갈라 두면 계약이 바뀔 때 어디를 고쳐야 하는지가
//! 한눈에 보인다.

/// 코어가 알려 주는 언어 선언. 표시 이름·키캡 표기는 팩이 밝히므로 셸은 이 값을
/// 그대로 쓰고 자기 표를 따로 두지 않는다.
#[derive(uniffi::Record)]
pub struct FfiLanguageDescriptor {
    pub tag: String,
    pub display_name: String,
    pub keycap_label: String,
    pub layout_name: String,
    /// 조합 중인 글자를 문서에 어떻게 앉힐지 — 언어 관습이므로 코어가 정한다.
    pub composing_display: FfiComposingDisplay,
}

/// 조합 중인 글자를 문서에 앉히는 방식.
#[derive(uniffi::Enum, PartialEq, Eq)]
pub enum FfiComposingDisplay {
    /// 밑줄 없이 글자를 그대로 — 한국어 순정이 그렇다. 셸은 조합 구간을 지우고 다시 넣는다.
    Inline,
    /// 밑줄 친 조합 구간으로 — 변환이 그렇다. 셸은 marked text로 옮긴다.
    Marked,
}

#[derive(uniffi::Enum)]
pub enum FfiFieldKind {
    Text,
    Email,
    Url,
    Search,
    Number,
    Decimal,
    Phone,
    Password,
}

/// 리턴키가 시키는 동작 — iOS `returnKeyType`, Android `imeOptions`를 셸이 옮긴다.
#[derive(uniffi::Enum)]
pub enum FfiReturnKey {
    Return,
    Go,
    Search,
    Send,
    Next,
    Done,
    Join,
    Route,
    Continue,
}

/// 앱이 요구하는 자동 대문자화의 범위 — iOS `autocapitalizationType`.
#[derive(uniffi::Enum)]
pub enum FfiCapitalization {
    None,
    Words,
    Sentences,
    AllCharacters,
}

/// 편집 대상이 스스로 밝힌 성격. 값의 주인이 앱이라 사용자 설정과 AND로 묶인다.
#[derive(uniffi::Record)]
pub struct FfiFieldTraits {
    pub kind: FfiFieldKind,
    pub return_key: FfiReturnKey,
    pub capitalization: FfiCapitalization,
    pub autocorrect: bool,
    pub smart_punctuation: bool,
}

/// 친 말과 그 자리에 들어갈 말. 순정의 "텍스트 대치"와 같다.
#[derive(uniffi::Record)]
pub struct FfiShortcut {
    pub trigger: String,
    pub replacement: String,
}

#[derive(uniffi::Record)]
pub struct FfiEditorContext {
    pub text_before_cursor: Option<String>,
    pub incognito: bool,
    pub field: FfiFieldKind,
}

/// 설정 화면이 소유하는 값 — 순정 키보드의 설정은 서드파티가 읽을 수 없으므로
/// 셸이 자기 저장소에서 읽어 세션에 넣는다.
#[derive(uniffi::Record)]
pub struct FfiUserPreferences {
    pub auto_correction: bool,
    pub predictions: bool,
    pub double_space_period: bool,
    pub personalized_learning: bool,
    pub auto_capitalization: bool,
    pub smart_punctuation: bool,
    pub auto_pairing: bool,
    pub annotation_candidates: bool,
    pub key_alternates: bool,
    pub number_row: bool,
    pub candidate_bar_always: bool,
    pub keyboard_height: FfiKeyboardHeight,
    pub cursor_sensitivity: FfiCursorSensitivity,
}

#[derive(uniffi::Enum)]
pub enum FfiKeyboardHeight {
    Compact,
    Standard,
    Tall,
}

#[derive(uniffi::Enum)]
pub enum FfiCursorSensitivity {
    Low,
    Standard,
    High,
}

/// 스냅샷에 무엇이 얼마나 들어 있는지. 형식의 주인이 여기이므로 세는 일도 여기서 한다 —
/// 셸이 줄을 뜯어보기 시작하면 형식이 두 곳에 살게 된다.
#[derive(uniffi::Record)]
pub struct FfiPersonalizationSummary {
    pub learned_words: u32,
    pub recent_annotations: u32,
}

#[derive(uniffi::Enum)]
pub enum FfiInputEvent {
    /// 한 번에 여러 글자를 넣는 키 (`.com` 등)
    Text {
        text: String,
    },
    Key {
        character: String,
    },
    Backspace,
    Separator {
        character: String,
    },
    CandidateSelected {
        index: u32,
    },
    CursorMoved,
    FocusLost,
}

#[derive(uniffi::Enum)]
pub enum FfiCandidateKind {
    Typed,
    Prediction,
    Conversion,
    Correction,
}

/// 후보 바에서 이 후보가 서는 자리 — 셸은 갈래별로 묶어 인라인으로 늘어놓는다.
#[derive(uniffi::Enum)]
pub enum FfiCandidateGroup {
    Word,
    Emoji,
    Symbol,
    Emoticon,
}

#[derive(uniffi::Record)]
pub struct FfiCandidate {
    pub text: String,
    pub kind: FfiCandidateKind,
    pub group: FfiCandidateGroup,
}

#[derive(uniffi::Enum)]
pub enum FfiEffect {
    CommitText {
        text: String,
    },
    SetComposing {
        text: String,
        caret: u32,
        /// 지금 사람이 손대고 있는 구간(코드포인트 [시작, 끝)) — 변환의 주목 문절이다.
        /// 셸은 이 값이 있을 때 조합 구간의 선택 범위로 옮긴다.
        focus_start: Option<u32>,
        focus_end: Option<u32>,
    },
    ClearComposing,
    DeleteBackward {
        code_points: u32,
    },
    UpdateCandidates {
        candidates: Vec<FfiCandidate>,
    },
    MoveCursor {
        offset: i32,
    },
    /// 밀리초 뒤에 `timer_fired`를 부르라는 요청. 앞선 타이머는 갈아 끼운다 —
    /// 끄는 명령은 없다(이미 끝난 주기에 울린 타이머는 아무 일도 하지 않는다).
    SetTimer {
        milliseconds: u32,
    },
}

/// 통합 검색면에 담기는 항목 하나.
#[derive(uniffi::Record)]
pub struct FfiAnnotationPanelItem {
    pub group: FfiCandidateGroup,
    pub text: String,
}

/// 검색면의 한 그룹. 헤더 문구는 싣지 않는다 — 갈래(`group`)와 묶음(`category`)이 곧
/// 신원이고, 그것을 어느 나라 말로 적을지는 화면의 일이다. 둘 다 비면 최근에 고른 것들이다.
#[derive(uniffi::Record)]
pub struct FfiAnnotationPanelGroup {
    pub group: Option<FfiCandidateGroup>,
    /// 이모지 묶음이면 그 자리 — 셸이 묶음마다 다른 표식을 세운다
    pub category: Option<FfiEmojiCategory>,
    pub items: Vec<FfiAnnotationPanelItem>,
}

/// 이모지가 검색면에서 서는 묶음 — 빌트인 키보드와 같은 갈래다
#[derive(uniffi::Enum)]
pub enum FfiEmojiCategory {
    SmileysAndPeople,
    AnimalsAndNature,
    FoodAndDrink,
    Activities,
    TravelAndPlaces,
    Objects,
    Symbols,
    Flags,
}

#[derive(uniffi::Record)]
pub struct FfiAnnotationPanel {
    pub groups: Vec<FfiAnnotationPanelGroup>,
}

#[derive(uniffi::Record)]
pub struct FfiKeyBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// 셸이 길게 누르기 같은 플랫폼 관습을 붙일 때 쓰는 갈래 (화이트리스트 분기)
#[derive(uniffi::Enum)]
pub enum FfiKeyRole {
    Character,
    Shift,
    Backspace,
    Space,
    Enter,
    LayerSwitch,
    LanguageSwitch,
    /// 정해진 언어로 곧장 가는 키 (천지인의 ABC·한글)
    LanguageSelect,
    /// 커서를 오른쪽으로 옮기는 키 (천지인의 →)
    CursorRight,
    /// 눌리지 않는 빈 자리 — 셸은 키를 그리지 않는다
    Blank,
}

/// 낱말로 적히는 키의 갈래 — 셸이 화면 언어로 옮긴다.
#[derive(uniffi::Enum)]
pub enum FfiKeyLegend {
    Return,
    Go,
    Search,
    Send,
    Next,
    Done,
    Join,
    Route,
    Continue,
}

#[derive(uniffi::Record)]
pub struct FfiFrameKey {
    pub row: u32,
    pub index: u32,
    pub label: String,
    /// 낱말로 적히는 키 — 화면 언어를 타므로 셸이 자기 말로 옮긴다. 없으면 `label`이
    /// 그대로 나가는 글자·기호 키다.
    pub legend: Option<FfiKeyLegend>,
    pub bounds: FfiKeyBounds,
    pub shift_active: bool,
    /// 이 필드에서 강조색으로 그릴 키 (검색 필드의 리턴키 등)
    pub emphasized: bool,
    pub role: FfiKeyRole,
    /// 이 키 라벨의 글꼴 크기(pt) — 글자 키·기호 제어 키·낱말 제어 키가 서로 다르다
    pub font_size: f32,
    pub alternates: Vec<String>,
}

/// 셸이 판별해 넘기는 표시 폼팩터 — 플랫폼 값(size class 등)의 번역일 뿐이고,
/// 이 갈래로 무엇을 할지는 코어가 정한다.
#[derive(uniffi::Enum)]
pub enum FfiFormFactor {
    PhonePortrait,
    PhoneLandscape,
    Tablet,
}

/// 코어가 정한 실측 치수(pt). 셸은 그대로 제약·글꼴에 쓴다.
#[derive(uniffi::Record)]
pub struct FfiFrameMetrics {
    /// 키 그리드 높이 — 키의 정규화 높이에 곱하면 실제 높이다
    pub grid_height: f32,
    pub candidate_bar_height: f32,
    /// 후보 바까지 포함한 입력 뷰 전체 높이
    pub total_height: f32,
    /// 글자 키 글꼴 — 키 밖에서 같은 크기를 써야 하는 자리(변형 문자 팝업)가 쓴다
    pub letter_font_size: f32,
}

#[derive(uniffi::Record)]
pub struct FfiKeyboardFrame {
    pub rows: Vec<Vec<FfiFrameKey>>,
    pub metrics: FfiFrameMetrics,
    /// 키 위에 놓이는 통합 검색면이 차지하는 높이(키보드 높이 기준 정규화값). 0이면
    /// 패널이 없는 레이어다 — 내용은 `annotation_panel`로 따로 받는다.
    pub panel_height_ratio: f32,
}

#[derive(uniffi::Record)]
pub struct FfiPressResult {
    pub effects: Vec<FfiEffect>,
    pub layout_changed: bool,
    /// 코어가 셸에 낸 요청 — 언어 목록·순서는 셸이 소유하므로 전환 자체는 셸이 한다
    pub requests_next_language: bool,
    /// 곧장 가라고 지목된 언어의 태그 (천지인의 ABC·한글). 쓰고 있지 않은 언어면
    /// 셸이 흘려보낸다 — 무엇을 쓰고 있는지는 셸만 안다.
    pub requests_language: Option<String>,
}

/// 빌드에 포함되지 않은 언어를 요청한 경우 — 셸은 해당 언어를 목록에서 제외한다.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum FfiLanguageError {
    #[error("이 빌드에 포함되지 않은 언어")]
    Unsupported,
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum FfiPackError {
    #[error("팩 파일을 열 수 없음: {message}")]
    Io { message: String },
    #[error("팩 형식 오류: {message}")]
    Invalid { message: String },
}
