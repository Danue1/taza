//! 코어와 셸이 주고받는 경계 타입 전부. 여기에는 타입과 trait만 두고, 판단 규칙은
//! `policy`에, 조립은 `engine`에 둔다.

pub use crate::pack::Pack;

use crate::personalization::PersonalizationStore;

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputEvent {
    Key(char),
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
    CandidateSelected(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedText {
    pub surface: String,
    /// 재변환·학습·확정 취소에 필요한 원 입력 (일본어 reading, 병음 등)
    pub reading: Option<String>,
    /// 자동교정으로 치환된 경우의 교정 전 원문
    pub corrected_from: Option<String>,
}

impl CommittedText {
    pub fn plain(surface: impl Into<String>) -> Self {
        CommittedText {
            surface: surface.into(),
            reading: None,
            corrected_from: None,
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
    Prediction,
    Conversion,
    Correction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub text: String,
    pub kind: CandidateKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ComposerOutput {
    /// commit 적용 전에 삭제할 확정 텍스트의 그래핌 수 (제안 치환·확정 취소용)
    pub delete_before_commit: usize,
    pub commit: Option<CommittedText>,
    pub composing: Option<ComposingText>,
    pub candidates: Vec<Candidate>,
}

/// 익스텐션 프로세스 kill 대비 직렬화 대상. 표현 형식(바이트 인코딩)은 FFI 계층에서 결정한다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposerState {
    Direct,
    Hangul {
        composing_jamo: Vec<char>,
        word_jamo: Vec<char>,
    },
    Latin {
        current_word: String,
    },
}

/// Composer가 판단에 쓰는 주변 상태 묶음. 새 의존이 생겨도 trait 시그니처는 불변.
pub struct ComposerEnvironment<'call> {
    /// composing이 없을 때 커서 앞 확정 텍스트를 채택(adopt)해 합성을 재개하는 통로
    pub context: &'call EditorContext,
    /// 활성 언어팩 — 미다운로드 언어에서는 None이며 합성 자체는 팩 없이도 동작해야 한다.
    /// 필요한 섹션(lexicon, language_model)만 꺼내 쓴다 — LM 교체는 팩 배포로 끝난다.
    pub pack: Option<&'call Pack<'call>>,
    /// 온디바이스 개인화 — 기록 여부는 context.incognito를 반드시 확인한다
    pub personalization: &'call mut PersonalizationStore,
}

pub trait Composer: Send {
    fn feed(&mut self, event: ComposerEvent, environment: &mut ComposerEnvironment<'_>)
    -> ComposerOutput;

    /// 커서 이동·포커스 이탈 시 진행 중 composing을 언어별 정책으로 강제 확정한다.
    fn finalize(&mut self) -> Option<CommittedText>;

    fn is_composing(&self) -> bool;

    fn snapshot(&self) -> ComposerState;

    fn restore(&mut self, state: ComposerState);
}
