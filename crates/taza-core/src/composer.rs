pub mod direct;
pub mod hangul;

/// 커서 주변 문맥. 플랫폼이 제공하지 못하면 None (iOS는 앱에 따라 0자·nil이 일상).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditorContext {
    pub text_before_cursor: Option<String>,
}

impl EditorContext {
    pub fn unavailable() -> Self {
        EditorContext::default()
    }
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
    Hangul { composing_jamo: Vec<char> },
}

pub trait Composer {
    /// composing이 없을 때 커서 앞 확정 텍스트를 채택(adopt)해 합성을 재개할 수 있다 —
    /// 채택분은 delete_before_commit으로 치환하고 composing으로 되가져온다.
    /// (한국어 자모 분해 삭제·도깨비불 재개, 추후 텔렉스 성조 소급 수정 등 언어 공통 통로)
    fn feed(&mut self, event: ComposerEvent, context: &EditorContext) -> ComposerOutput;

    /// 커서 이동·포커스 이탈 시 진행 중 composing을 언어별 정책으로 강제 확정한다.
    fn finalize(&mut self) -> Option<CommittedText>;

    fn is_composing(&self) -> bool;

    fn snapshot(&self) -> ComposerState;

    fn restore(&mut self, state: ComposerState);
}
