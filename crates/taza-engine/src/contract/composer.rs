//! 합성기 계약. 이것만은 **경계를 건너지 않는다** — 셸은 `Composer`도 `ComposerOutput`도
//! 보지 못하고, taza-ffi에 이들의 거울 타입이 없는 것이 그 증거다. 코어 안쪽에서 조립
//! (`engine`)과 스크립트 조합(`lang`)이 만나는 자리다.

use std::borrow::Cow;

use super::shell::EditorContext;
use crate::convert::Conversion;

/// 레이아웃이 물리 키를 언어별 논리 문자로 해석한 뒤 Composer에 전달한다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposerEvent {
    Key(char),
    Backspace,
    Separator(char),
    /// 후보 목록은 Engine이 소유하므로 인덱스가 아니라 고른 것 자체가 온다.
    /// 조회 키가 함께 오는 까닭은 **얼마만큼이 확정되는지**를 표시 텍스트가 말해 주지 않기
    /// 때문이다 — 변환에서 첫 문절만 고르면 나머지 읽기는 조합에 남아 다시 변환된다.
    CandidateSelected {
        text: String,
        key: String,
    },
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
    /// 지금 사람이 손대고 있는 구간(코드포인트 [시작, 끝)). 변환처럼 조합 창 안에서
    /// **한 도막만 골라 바꾸는** 방식이 그 도막을 밝히는 자리다. 없으면 조합 창 전체가
    /// 한 덩어리다.
    pub focus: Option<(usize, usize)>,
}

impl ComposingText {
    /// 캐럿이 끝에 있고 도막이 나뉘지 않은 조합 창 — 자모 오토마타가 내는 모양이다.
    pub fn whole(text: impl Into<String>) -> Self {
        let text = text.into();
        let caret = text.chars().count();
        ComposingText {
            text,
            caret,
            focus: None,
        }
    }
}

/// 조합 중인 글자를 문서에 어떻게 앉히는가. 플랫폼이 정하는 것이 아니라 **언어 관습**이라
/// 코어가 밝히고 셸은 옮기기만 한다.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ComposingDisplay {
    /// 밑줄 없이 글자를 그대로 — 한국어 순정이 그렇고, 그래서 조합 창을 지원하지 않는
    /// 앱에서도 글이 깨지지 않는다.
    #[default]
    Inline,
    /// 밑줄 친 조합 구간으로. 길이가 크게 출렁이고 주목 도막을 밝혀야 하는 변환이 쓴다 —
    /// 확정 전 텍스트를 지웠다 넣었다 하면 문서 이력이 그만큼 더럽혀진다.
    Marked,
}

/// 합성기가 후보 바에 요청하는 것. 랭킹은 언어와 직교한 `suggest`가 하므로 합성기는
/// "무엇을 기준으로 찾을지"만 말한다.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SuggestionRequest {
    #[default]
    None,
    /// 진행 중인 어휘의 완성·교정. key는 팩 인코딩에 맞춘 사전 조회 키다.
    Word { key: String },
    /// 합성기가 **이미 만들어 놓은** 후보들 — (조회 키, 표시 텍스트) 짝이다. 조합 결과가
    /// 곧 후보인 방식(가나-한자 변환)이 쓴다: 사전을 봐야 조합 창을 그릴 수 있는 방식에서는
    /// 후보를 다시 찾는 일이 같은 탐색을 두 번 하는 것이 된다. 순서는 그대로 두고 Engine은
    /// 갈래만 입힌다.
    ///
    /// 키가 후보마다 따로 붙는 까닭은 **확정되는 양이 후보마다 다르기 때문**이다 — 아직 다
    /// 치지 않은 읽기로 낱말을 미리 내놓을 때(予測変換) 고른 후보가 읽기보다 길 수 있다.
    Ready { candidates: Vec<(String, String)> },
}

/// 어절이 끝났다는 신호. 자동교정 여부·학습·다음 단어 예측은 Engine이 판단하므로
/// 합성기는 무엇이 끝났는지만 알린다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordBoundary {
    /// 경계를 만든 문자 — Engine이 확정 텍스트 끝에 붙인다
    pub separator: char,
    /// 끝난 어휘의 사전 조회 키
    pub key: String,
    /// 그 어휘의 표시 형태. 자동교정을 쓰는 방식에서는 이만큼을 지우고 치환한다.
    pub surface: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ComposerOutput {
    /// commit 적용 전에 삭제할 확정 텍스트의 **코드포인트 수** (제안 치환·확정 취소용).
    /// 그대로 `Effect::DeleteBackward`가 되므로 단위가 그것과 같아야 한다.
    ///
    /// 셸은 이 수를 플랫폼의 한 글자 삭제(iOS `deleteBackward()`)로 옮기는데 그쪽 단위는
    /// **그래핌**이다. 지금 어긋나지 않는 것은 합성기가 내는 어절에 그래핌을 늘리는 글자가
    /// 들어오지 않기 때문이다 — 라틴은 결합 부호(U+0301)가 `is_alphabetic`이 아니라 거기서
    /// 어절이 끊기고, 한글 음절은 코드포인트 하나다. 데바나가리 모음 기호(U+093E)나 타이
    /// 사라 암(U+0E33)은 `is_alphabetic`이라 어절 안에 서므로, 그 스크립트를 실을 때
    /// 셸의 삭제를 코드포인트 단위로 바꾸지 않으면 한 번에 한 글자씩 더 지운다.
    pub delete_before_commit: usize,
    pub commit: Option<CommittedText>,
    pub composing: Option<ComposingText>,
    pub boundary: Option<WordBoundary>,
    pub suggest: SuggestionRequest,
}

/// 익스텐션 프로세스 kill 대비 직렬화 대상. 무엇을 담을지는 각 합성기가 정하고 코어는
/// 바이트로만 다룬다 — 방식이 늘어도 이 계약은 그대로다.
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

/// 합성기가 이벤트를 푸는 동안 볼 수 있는 바깥.
///
/// 지금 나르는 것은 커서 앞 문맥 하나뿐이고, 그것으로 composing이 없을 때 확정 텍스트를
/// 채택(adopt)해 합성을 재개한다. 이 자리를 값 하나가 아니라 묶음으로 둔 까닭은 합성기가
/// 볼 것이 문맥만은 아니기 때문이다 — 조합 결과가 곧 사전 조회 결과인 스크립트
/// (가나-한자 변환)는 사전 없이 자기 조합 창을 만들 수 없다.
#[derive(Clone)]
pub struct ComposerEnvironment<'call> {
    context: Cow<'call, EditorContext>,
    conversion: Option<Conversion<'call>>,
}

impl<'call> ComposerEnvironment<'call> {
    pub fn new(context: &'call EditorContext) -> Self {
        ComposerEnvironment {
            context: Cow::Borrowed(context),
            conversion: None,
        }
    }

    /// 변환표를 낼 수 있는 팩이 꽂혀 있을 때 Engine이 창구를 함께 준다.
    pub fn with_conversion(mut self, conversion: Option<Conversion<'call>>) -> Self {
        self.conversion = conversion;
        self
    }

    pub fn context(&self) -> &EditorContext {
        &self.context
    }

    /// 읽기를 표기로 옮기는 창구. 사전에 닿는 길은 이것 하나뿐이다 — 완성·교정·랭킹은
    /// 여전히 바깥(`suggest`)의 일이라 합성기에서 그리로 가는 길이 없다.
    pub fn conversion(&self) -> Option<&Conversion<'call>> {
        self.conversion.as_ref()
    }

    /// 방금 낸 Effect가 셸에 **아직 적용되지 않은** 시점의 바깥. 한 번의 입력을 지우기·
    /// 넣기 두 걸음으로 푸는 합성기가 두 번째 걸음에 쓴다 — 지운 글자가 그대로 남아 있는
    /// 문맥으로 합성을 재개하면 그것을 도로 주워 온다.
    pub fn unapplied(&self) -> ComposerEnvironment<'_> {
        ComposerEnvironment {
            context: Cow::Owned(self.context.unapplied()),
            conversion: self.conversion,
        }
    }
}

/// 스크립트 조합만 맡는다 — 완성·교정·랭킹·학습은 전부 바깥(suggest, engine)의 일이다.
pub trait Composer: Send {
    fn feed(
        &mut self,
        event: ComposerEvent,
        environment: &ComposerEnvironment<'_>,
    ) -> ComposerOutput;

    /// 커서 이동·포커스 이탈 시 진행 중 composing을 언어별 정책으로 강제 확정한다.
    fn finalize(&mut self) -> Option<CommittedText>;

    fn is_composing(&self) -> bool;

    fn snapshot(&self) -> ComposerState;

    fn restore(&mut self, state: ComposerState);
}
