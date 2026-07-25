use crate::contract::{
    CommittedText, Composer, ComposerEvent, ComposerOutput, ComposerState, EditorContext,
    SuggestionRequest, WordBoundary,
};
use crate::policy::double_space_period;

fn is_word_character(character: char) -> bool {
    character.is_alphabetic() || character == '\''
}

/// 라틴 골격: composing 없이 글자를 즉시 확정하고(플랫폼 관습 — 영어는 marked text를
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

    /// 커서 이동 뒤 합성 재개 — 커서 앞의 연속된 단어 문자를 현재 단어로 되가져온다.
    fn adopt_from_context(&mut self, context: &EditorContext) {
        if !self.current_word.is_empty() {
            return;
        }
        let Some(text) = &context.text_before_cursor else {
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
}

impl Composer for LatinComposer {
    fn feed(&mut self, event: ComposerEvent, context: &EditorContext) -> ComposerOutput {
        match event {
            ComposerEvent::Key(character) if is_word_character(character) => {
                self.adopt_from_context(context);
                self.current_word.push(character);
                ComposerOutput {
                    commit: Some(CommittedText::plain(character.to_string())),
                    suggest: self.suggest(),
                    ..ComposerOutput::default()
                }
            }
            ComposerEvent::Key(character) => {
                self.current_word.clear();
                ComposerOutput {
                    commit: Some(CommittedText::plain(character.to_string())),
                    ..ComposerOutput::default()
                }
            }
            ComposerEvent::Separator(' ') if self.current_word.is_empty() => {
                match double_space_period(context) {
                    Some(output) => output,
                    None => ComposerOutput {
                        commit: Some(CommittedText::plain(" ".to_string())),
                        ..ComposerOutput::default()
                    },
                }
            }
            ComposerEvent::Separator(character) => {
                let word = std::mem::take(&mut self.current_word);
                ComposerOutput {
                    boundary: Some(WordBoundary {
                        separator: character,
                        key: word.clone(),
                        surface: word,
                    }),
                    ..ComposerOutput::default()
                }
            }
            ComposerEvent::Backspace => {
                self.adopt_from_context(context);
                self.current_word.pop();
                ComposerOutput {
                    delete_before_commit: 1,
                    suggest: self.suggest(),
                    ..ComposerOutput::default()
                }
            }
            ComposerEvent::CandidateSelected(text) => {
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
