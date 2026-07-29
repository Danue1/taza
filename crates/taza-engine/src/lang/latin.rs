use crate::contract::{
    CommittedText, Composer, ComposerEnvironment, ComposerEvent, ComposerOutput, ComposerState,
    SuggestionRequest, WordBoundary,
};
use crate::keyboard::{NamedLayoutSet, layouts};
use crate::lang::InputMethod;

/// 라틴 입력 방식 — 친 그대로 확정하되 어절을 추적해 제안·자동교정을 붙인다.
pub struct LatinMethod;

pub static LATIN: LatinMethod = LatinMethod;

impl InputMethod for LatinMethod {
    fn tag(&self) -> &'static str {
        "latin"
    }

    fn layouts(&self) -> Vec<NamedLayoutSet> {
        layouts::latin::layouts()
    }

    fn composer(&self) -> Box<dyn Composer> {
        Box::new(LatinComposer::new())
    }

    fn autocorrects(&self) -> bool {
        true
    }
}

/// 짝맞춤 아포스트로피는 곧은 것과 똑같이 어절 안에 선다 — 순정도 축약형에 이 글자를
/// 넣고 그 낱말을 계속 예측한다. 어절 글자로 보지 않으면 `don't`가 `don`과 `t`로 갈려
/// 축약형이 사전에 닿지 못한다. 조회 키로 옮길 때 곧은 것으로 접는다(`suggest::lookup`).
fn is_word_character(character: char) -> bool {
    character.is_alphabetic() || character == '\'' || character == '\u{2019}'
}

/// 라틴 합성기: composing 없이 글자를 즉시 확정하고(플랫폼 관습 — 영어는 marked text를
/// 쓰지 않는다), 현재 단어만 내부에서 추적한다. 무엇을 제안할지·교정할지는 Engine이
/// 정하므로 여기서는 조회 키와 어절 경계만 낸다.
#[derive(Debug, Default)]
pub struct LatinComposer {
    current_word: String,
}

impl LatinComposer {
    pub fn new() -> Self {
        LatinComposer::default()
    }

    /// 문맥이 곧 진실이다 — 커서 앞의 연속된 단어 문자가 지금 어절이고, 우리가 쥐고
    /// 있던 것과 다르면 그쪽을 따른다. 커서가 다른 자리로 옮겨 갔을 때(셸이 알려 주지
    /// 못한 경우까지) 남의 자리 어절을 들고 교정·학습하지 않기 위한 재동기화다.
    /// 문맥을 못 받는 앱에서는 추적하던 값을 그대로 믿는다.
    fn sync_with_context(&mut self, environment: &ComposerEnvironment<'_>) {
        let Some(text) = &environment.context().text_before_cursor else {
            return;
        };
        let word: Vec<char> = text
            .chars()
            .rev()
            .take_while(|&character| is_word_character(character))
            .collect();
        self.current_word = word.into_iter().rev().collect();
    }

    fn suggest(&self) -> SuggestionRequest {
        if self.current_word.is_empty() {
            SuggestionRequest::None
        } else {
            SuggestionRequest::Word {
                key: self.current_word.clone(),
            }
        }
    }

    /// 어절이 여기서 끝났다 — 경계 문자는 Engine이 교정 결과 뒤에 붙인다.
    fn end_word(&mut self, separator: char) -> ComposerOutput {
        let word = std::mem::take(&mut self.current_word);
        ComposerOutput {
            boundary: Some(WordBoundary {
                separator,
                key: word.clone(),
                surface: word,
            }),
            ..ComposerOutput::default()
        }
    }
}

impl Composer for LatinComposer {
    fn feed(
        &mut self,
        event: ComposerEvent,
        environment: &ComposerEnvironment<'_>,
    ) -> ComposerOutput {
        // 무엇을 하든 어절부터 문맥에 맞춘다 — 커서가 옮겨 간 뒤에도 남의 자리 어절을
        // 들고 교정·학습하지 않기 위해서다
        self.sync_with_context(environment);
        match event {
            ComposerEvent::Key(character) if is_word_character(character) => {
                self.current_word.push(character);
                ComposerOutput {
                    commit: Some(CommittedText::plain(character.to_string())),
                    suggest: self.suggest(),
                    ..ComposerOutput::default()
                }
            }
            // 어절 안에 설 수 없는 글자는 그 자리에서 어절을 끝낸다 — 순정도 마침표·
            // 쉼표에서 교정한다("teh." → "the."). 글자 자체는 경계 문자로 넘기므로
            // 교정 결과 뒤에 Engine이 붙여 넣는다.
            ComposerEvent::Key(character) => self.end_word(character),
            ComposerEvent::Separator(' ') if self.current_word.is_empty() => ComposerOutput {
                commit: Some(CommittedText::plain(" ".to_string())),
                ..ComposerOutput::default()
            },
            ComposerEvent::Separator(character) => self.end_word(character),
            ComposerEvent::Backspace => {
                self.current_word.pop();
                ComposerOutput {
                    delete_before_commit: 1,
                    suggest: self.suggest(),
                    ..ComposerOutput::default()
                }
            }
            ComposerEvent::CandidateSelected { text, .. } => {
                let original = std::mem::take(&mut self.current_word);
                // 선택 확정 뒤의 타이핑은 새 입력 시퀀스 — 후행 공백이 그 경계다
                ComposerOutput {
                    delete_before_commit: original.chars().count(),
                    commit: Some(CommittedText::plain(format!("{text} "))),
                    ..ComposerOutput::default()
                }
            }
        }
    }

    fn finalize(&mut self) -> Option<CommittedText> {
        self.current_word.clear();
        None
    }

    fn is_composing(&self) -> bool {
        false
    }

    fn snapshot(&self) -> ComposerState {
        ComposerState::from_text(&self.current_word)
    }

    fn restore(&mut self, state: ComposerState) {
        if let Some(text) = state.text() {
            self.current_word = text.to_string();
        }
    }
}
