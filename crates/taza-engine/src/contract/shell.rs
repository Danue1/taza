//! 셸이 주인인 값들 — 코어는 주입받아 읽기만 한다.
//!
//! 두 갈래가 섞여 있지 않다는 점이 중요하다: `FieldTraits`는 **앱**이 밝힌 것이고
//! `UserPreferences`는 **사용자**가 정한 것이라, 둘은 AND로 결합한다. 설정을 켜 두어도
//! 앱이 끄라고 하면 꺼진다. `EditorContext`는 이벤트마다 오는 그때그때의 사정이다.

/// 입력 필드의 종류 — 플랫폼 inputmode/keyboardType/inputType을 셸이 매핑한다.
/// 필드는 보조 기능뿐 아니라 화면도 정한다(`keyboard::field`) — 순정 키보드가
/// 필드마다 다른 배열·하단 행·리턴키를 쓰기 때문이다.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FieldKind {
    #[default]
    Text,
    Email,
    Url,
    /// 검색 — 배열은 일반 텍스트와 같고 리턴키만 "검색"이 된다. 예측도 그대로 둔다.
    Search,
    Number,
    /// 소수 — 숫자 패드에 소수점이 붙는다
    Decimal,
    Phone,
    Password,
}

impl FieldKind {
    /// 제안·자동교정·학습 같은 보조 기능을 제공할 필드인가. 순정은 검색 필드에서도
    /// 예측을 그대로 둔다 — 검색어야말로 예측이 가장 쓸모 있는 자리다.
    pub fn assistance_enabled(self) -> bool {
        matches!(self, FieldKind::Text | FieldKind::Search)
    }

    /// 문장 첫 글자를 자동으로 대문자로 올릴 필드인가. 순정은 이메일·URL·비밀번호에서
    /// 올리지 않는다 — 그 자리에 들어갈 값은 문장이 아니기 때문이다.
    pub fn auto_capitalizes(self) -> bool {
        matches!(self, FieldKind::Text | FieldKind::Search)
    }
}

/// 리턴키가 시키는 동작. 순정은 앱이 밝힌 것을 그대로 적는다(iOS `returnKeyType`,
/// Android `imeOptions`) — 낱말 자체는 화면 언어를 타므로 셸이 옮긴다.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReturnKey {
    #[default]
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

/// 앱이 요구하는 자동 대문자화의 범위. 순정은 이것을 그대로 따른다 — 이름 칸에서
/// 낱말마다 대문자를 올리는 것도, 코드 칸에서 아예 올리지 않는 것도 앱이 정한 일이다.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Capitalization {
    None,
    Words,
    #[default]
    Sentences,
    AllCharacters,
}

/// 편집 대상이 스스로 밝힌 성격. 값의 주인이 **앱**이라는 점에서 사용자 설정과 다르다 —
/// 둘은 AND로 결합한다: 사용자가 자동 수정을 켜 두었어도 앱이 끄라고 하면 꺼진다.
///
/// 필드가 바뀔 때 한 번 주입된다(이벤트마다 오는 `EditorContext`와 달리 화면을 정하는
/// 값이라, 이벤트가 오기 전에 이미 맞아 있어야 한다).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldTraits {
    pub kind: FieldKind,
    pub return_key: ReturnKey,
    pub capitalization: Capitalization,
    /// 앱이 자동 수정을 허용하는가 (iOS `autocorrectionType`)
    pub autocorrect: bool,
    /// 앱이 짝맞춤 부호를 허용하는가 (iOS `smartQuotesType`·`smartDashesType`)
    pub smart_punctuation: bool,
}

impl Default for FieldTraits {
    /// 앱이 아무 말도 하지 않을 때 — 순정의 기본값과 같다.
    fn default() -> Self {
        FieldTraits {
            kind: FieldKind::default(),
            return_key: ReturnKey::default(),
            capitalization: Capitalization::default(),
            autocorrect: true,
            smart_punctuation: true,
        }
    }
}

impl FieldTraits {
    /// 앱이 리턴키를 따로 밝히지 않았을 때의 값 — 검색 칸이면 "검색"이다. 순정도
    /// 앱이 말이 없으면 칸의 성격에서 미룬다.
    pub fn of(kind: FieldKind) -> Self {
        FieldTraits {
            kind,
            return_key: match kind {
                FieldKind::Search => ReturnKey::Search,
                _ => ReturnKey::Return,
            },
            ..FieldTraits::default()
        }
    }
}

/// 키보드가 차지하는 높이. 순정에는 없는 설정이지만 서드파티 키보드에서는 흔하다 —
/// 실제 치수는 폼팩터가 정하고 이 값은 거기에 곱해진다.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum KeyboardHeight {
    Compact,
    #[default]
    Standard,
    Tall,
}

impl KeyboardHeight {
    pub fn scale(self) -> f32 {
        match self {
            KeyboardHeight::Compact => 0.86,
            KeyboardHeight::Standard => 1.0,
            KeyboardHeight::Tall => 1.16,
        }
    }
}

/// 스페이스바를 끌어 커서를 옮길 때 손가락 이동이 커서에 얼마나 빨리 옮겨붙는가.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CursorSensitivity {
    Low,
    #[default]
    Standard,
    High,
}

impl CursorSensitivity {
    /// 한 칸을 옮기는 데 필요한 이동 거리에 곱하는 값 — 감도가 높을수록 짧아진다.
    pub fn step_scale(self) -> f32 {
        match self {
            CursorSensitivity::Low => 1.6,
            CursorSensitivity::Standard => 1.0,
            CursorSensitivity::High => 0.65,
        }
    }
}

/// 사용자가 설정 화면에서 켜고 끄는 것. 양 플랫폼 모두 순정 키보드의 이 설정들을
/// 서드파티가 읽을 수 없으므로(iOS는 시스템 설정 비공개, Android는 IME마다 자기 설정)
/// 값의 주인은 우리 설정 화면이고 셸이 세션에 주입한다. 필드 성격(`FieldKind`)과는
/// AND로 결합한다 — 설정을 켜 두어도 비밀번호 필드에서는 보조 기능이 꺼진다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserPreferences {
    /// 어절이 끝날 때 오타를 사전 표제어로 갈아치운다
    pub auto_correction: bool,
    /// 후보 바에 완성·교정·다음 단어 예측을 띄운다
    pub predictions: bool,
    /// 공백 두 번을 ". "로 바꾼다 (iOS "." 단축키 / Gboard 더블 스페이스 마침표)
    pub double_space_period: bool,
    /// 확정한 어휘를 개인 사전에 기록해 이후 제안에 반영한다
    pub personalized_learning: bool,
    /// 문장이 시작되는 자리에서 shift를 미리 올려 둔다
    pub auto_capitalization: bool,
    /// 곧은 따옴표를 짝맞춤 따옴표로, `--`를 줄표로 바꾼다
    pub smart_punctuation: bool,
    /// 여는 괄호·따옴표를 치면 닫는 짝을 함께 넣고 그 사이로 커서를 옮긴다
    pub auto_pairing: bool,
    /// 후보 바에 낱말과 함께 이모지·기호·얼굴 문자를 곁들인다
    pub annotation_candidates: bool,
    /// 글자 키를 길게 눌러 변형 문자를 고를 수 있게 한다
    pub key_alternates: bool,
    /// 문자면 위에 숫자 행을 한 줄 둔다
    pub number_row: bool,
    /// 후보를 내지 않는 필드에서도 후보 바 자리를 남긴다 — 필드를 옮길 때 키보드
    /// 높이가 변하는 것이 거슬리는 경우의 탈출구다
    pub candidate_bar_always: bool,
    pub keyboard_height: KeyboardHeight,
    pub cursor_sensitivity: CursorSensitivity,
}

impl Default for UserPreferences {
    /// 순정 키보드의 공장 초기값. 순정에 없는 항목은 순정과 같아 보이는 쪽을 고른다 —
    /// 자동 짝 넣기·숫자 행처럼 화면이나 입력 결과를 눈에 띄게 바꾸는 것은 꺼 둔다.
    fn default() -> Self {
        UserPreferences {
            auto_correction: true,
            predictions: true,
            double_space_period: true,
            personalized_learning: true,
            auto_capitalization: true,
            smart_punctuation: true,
            auto_pairing: false,
            annotation_candidates: true,
            key_alternates: true,
            number_row: false,
            candidate_bar_always: false,
            keyboard_height: KeyboardHeight::Standard,
            cursor_sensitivity: CursorSensitivity::Standard,
        }
    }
}

/// 커서 주변 문맥. 플랫폼이 제공하지 못하면 None (iOS는 앱에 따라 0자·nil이 일상).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditorContext {
    pub text_before_cursor: Option<String>,
    /// 시크릿 필드 등 학습 금지 신호 (Android IME_FLAG_NO_PERSONALIZED_LEARNING 등) —
    /// true면 개인화 기록을 하지 않는다
    pub incognito: bool,
    pub field: FieldKind,
}

impl EditorContext {
    pub fn unavailable() -> Self {
        EditorContext::default()
    }

    /// 방금 낸 Effect가 셸에 **아직 적용되지 않은** 시점의 문맥. 커서 앞 텍스트는 이미
    /// 실제와 어긋나 있으므로 없는 것으로 다룬다 — 지운 글자가 그대로 남아 있는 문맥으로
    /// 합성을 재개하면(채택) 그것을 도로 주워 온다.
    ///
    /// 한 번의 입력을 지우기·넣기 두 걸음으로 푸는 자리가 이 문맥을 쓴다: 멀티탭을 이어
    /// 누를 때와 천지인 모음을 갈아 끼울 때다.
    pub fn unapplied(&self) -> Self {
        EditorContext {
            text_before_cursor: None,
            ..self.clone()
        }
    }
}
