//! 코어와 셸이 주고받는 경계 타입 전부. 여기에는 타입과 trait만 두고, 판단 규칙은
//! `policy`에, 조립은 `engine`에 둔다.

pub use crate::keyboard::KeySignal;
pub use crate::pack::Pack;

/// 입력 필드의 종류 — 플랫폼 inputmode/keyboardType/inputType을 셸이 매핑한다.
/// Text 외 필드에서는 제안·자동교정·개인화 학습을 끈다 (순정 키보드 관습).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FieldKind {
    #[default]
    Text,
    Email,
    Url,
    Number,
    Phone,
    Password,
}

impl FieldKind {
    /// 제안·자동교정·학습 같은 보조 기능을 제공할 필드인가
    pub fn assistance_enabled(self) -> bool {
        self == FieldKind::Text
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
}

impl Default for UserPreferences {
    /// 순정 키보드의 공장 초기값 — 넷 다 켜져 있다.
    fn default() -> Self {
        UserPreferences {
            auto_correction: true,
            predictions: true,
            double_space_period: true,
            personalized_learning: true,
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
}

/// 셸이 코어로 보내는 입력. 터치 좌표를 키로 판정하는 일은 코어가 하므로
/// (`Engine::press_at`) 셸이 이 값을 직접 만드는 경로는 물리 키보드·접근성뿐이다.
#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    /// 터치가 만든 키 신호 — 확률 판정은 코어의 히트 테스트가 한다. 물리 키보드·접근성
    /// 경로처럼 어느 키인지 확실할 때는 `KeySignal::certain`으로 만든다.
    Key(KeySignal),
    Backspace,
    Separator(char),
    CandidateSelected(usize),
    CursorMoved,
    /// 스페이스바를 길게 눌러 끄는 커서 이동. 값은 논리적 이동 칸수(부호 = 방향)로,
    /// 코어가 포인터 이동량에서 산출한다.
    CursorDrag(i32),
    FocusLost,
}

/// 셸이 플랫폼 API로 번역하는 선언적 명령. 셸은 번역만 하고 판단하지 않는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// 활성 composing 구간이 있으면 그것을 치환하며 확정한다
    /// (iOS insertText / Android commitText의 공통 의미론)
    CommitText(String),
    SetComposing(ComposingText),
    /// composing 구간의 텍스트를 제거하고 composing 상태를 끝낸다.
    /// 주의: iOS unmarkText / Android finishComposingText는 "확정"이므로 그대로 쓰면
    /// 안 된다 — 빈 문자열로 치환 후 종료해야 한다 (셸 계약).
    ClearComposing,
    /// 코드포인트 수. iOS deleteBackward는 count 미보장이므로 셸은 적용 후 문맥 재동기화 필요
    DeleteBackward(usize),
    UpdateCandidates(Vec<Candidate>),
    /// 커서를 논리적으로 옮긴다(부호 = 방향, 단위 = 코드포인트).
    /// RTL에서도 의미는 "논리적 이동"으로 고정 — 시각적 방향은 플랫폼이 해석한다.
    MoveCursor(i32),
}

/// 레이아웃이 물리 키를 언어별 논리 문자로 해석한 뒤 Composer에 전달한다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposerEvent {
    Key(char),
    Backspace,
    Separator(char),
    /// 후보 목록은 Engine이 소유하므로 인덱스가 아니라 고른 표시 텍스트가 온다.
    CandidateSelected(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedText {
    pub surface: String,
    /// 재변환·학습·확정 취소에 필요한 원 입력 (일본어 reading, 병음 등)
    pub reading: Option<String>,
}

impl CommittedText {
    pub fn plain(surface: impl Into<String>) -> Self {
        CommittedText {
            surface: surface.into(),
            reading: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposingText {
    pub text: String,
    /// 코드포인트 단위 캐럿 위치
    pub caret: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateKind {
    /// 친 그대로의 원문. 교정·완성을 물리고 원문을 지키는 길이며 언제나 첫 자리다.
    Typed,
    Prediction,
    Conversion,
    Correction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub text: String,
    pub kind: CandidateKind,
}

/// 합성기가 후보 바에 요청하는 것. 랭킹은 언어와 직교한 `suggest`가 하므로 합성기는
/// "무엇을 기준으로 찾을지"만 말한다.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SuggestionRequest {
    #[default]
    None,
    /// 진행 중인 어휘의 완성·교정. key는 팩 인코딩에 맞춘 사전 조회 키다.
    Word { key: String },
}

/// 어절이 끝났다는 신호. 자동교정 여부·학습·다음 단어 예측은 Engine이 판단하므로
/// 합성기는 무엇이 끝났는지만 알린다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordBoundary {
    /// 경계를 만든 문자 — Engine이 확정 텍스트 끝에 붙인다
    pub separator: char,
    /// 끝난 어휘의 사전 조회 키
    pub key: String,
    /// 그 어휘의 표시 형태. 자동교정을 쓰는 골격에서는 이만큼을 지우고 치환한다.
    pub surface: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ComposerOutput {
    /// commit 적용 전에 삭제할 확정 텍스트의 그래핌 수 (제안 치환·확정 취소용)
    pub delete_before_commit: usize,
    pub commit: Option<CommittedText>,
    pub composing: Option<ComposingText>,
    pub boundary: Option<WordBoundary>,
    pub suggest: SuggestionRequest,
}

/// 익스텐션 프로세스 kill 대비 직렬화 대상. 무엇을 담을지는 각 합성기가 정하고 코어는
/// 바이트로만 다룬다 — 골격이 늘어도 이 계약은 그대로다.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ComposerState(pub Vec<u8>);

impl ComposerState {
    pub fn from_text(text: &str) -> Self {
        ComposerState(text.as_bytes().to_vec())
    }

    pub fn text(&self) -> Option<&str> {
        std::str::from_utf8(&self.0).ok()
    }
}

/// 스크립트 조합만 맡는다 — 사전 조회·랭킹·자동교정·학습은 전부 바깥(suggest, engine)의 일이다.
/// `context`는 composing이 없을 때 커서 앞 확정 텍스트를 채택(adopt)해 합성을 재개하는 통로다.
pub trait Composer: Send {
    fn feed(&mut self, event: ComposerEvent, context: &EditorContext) -> ComposerOutput;

    /// 커서 이동·포커스 이탈 시 진행 중 composing을 언어별 정책으로 강제 확정한다.
    fn finalize(&mut self) -> Option<CommittedText>;

    fn is_composing(&self) -> bool;

    fn snapshot(&self) -> ComposerState;

    fn restore(&mut self, state: ComposerState);
}
