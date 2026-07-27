//! 합성기 계약. 이것만은 **경계를 건너지 않는다** — 셸은 `Composer`도 `ComposerOutput`도
//! 보지 못하고, taza-ffi에 이들의 거울 타입이 없는 것이 그 증거다. 코어 안쪽에서 조립
//! (`engine`)과 스크립트 조합(`lang`)이 만나는 자리다.

use super::shell::EditorContext;

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
