use super::jamo::{compose_word, encode_jamo_ascii, is_jamo, recompose, render_all};
use crate::contract::{
    CommittedText, Composer, ComposerEvent, ComposerOutput, ComposerState, ComposingText,
    EditorContext, SuggestionRequest, WordBoundary,
};

use super::jamo::decompose;

/// 스냅샷에서 composing 창과 어절 자모를 가르는 글자
const STATE_SEPARATOR: char = '\t';

/// 두벌식 자모 오토마타. composing 창은 최대 2음절(직전 + 현재) — 도깨비불 발생 후에도
/// Backspace로 이전 상태("가바" → "갑")로 복귀할 수 있게 marked text 안에 유지하고,
/// 세 번째 음절이 시작될 때 가장 오래된 음절을 확정한다.
#[derive(Debug, Default)]
pub struct HangulComposer {
    composing_jamo: Vec<char>,
    /// 현재 어절 전체의 자모 — composing 창(2음절)보다 길 수 있으며 제안의 기준이다
    word_jamo: Vec<char>,
}

impl HangulComposer {
    pub fn new() -> Self {
        HangulComposer::default()
    }

    /// composing이 없을 때 커서 앞을 분해해 합성을 재개한다. composing 창에는 마지막
    /// 1글자만 되가져오고(치환 1글자), 어절 자모는 커서 앞의 한글 연속 구간 전체를 채운다.
    /// 반환값은 치환을 위해 지워야 할 확정 글자 수.
    fn try_adopt(&mut self, context: &EditorContext) -> usize {
        if !self.composing_jamo.is_empty() {
            return 0;
        }
        let Some(text) = &context.text_before_cursor else {
            return 0;
        };
        let Some(last_character) = text.chars().last() else {
            return 0;
        };
        let Some(jamo) = decompose(last_character) else {
            return 0;
        };
        self.composing_jamo = jamo;
        let word_characters: Vec<char> = text
            .chars()
            .rev()
            .take_while(|&character| decompose(character).is_some())
            .collect();
        self.word_jamo = word_characters
            .into_iter()
            .rev()
            .flat_map(|character| decompose(character).unwrap())
            .collect();
        1
    }

    fn suggest(&self) -> SuggestionRequest {
        match encode_jamo_ascii(&self.word_jamo) {
            Some(key) if !key.is_empty() => SuggestionRequest::Word { key },
            _ => SuggestionRequest::None,
        }
    }

    fn composing_output(&self, commit: Option<CommittedText>) -> ComposerOutput {
        let syllables = recompose(&self.composing_jamo);
        let text = render_all(&syllables);
        let composing = if text.is_empty() {
            None
        } else {
            let caret = text.chars().count();
            Some(ComposingText { text, caret })
        };
        ComposerOutput {
            commit,
            composing,
            suggest: self.suggest(),
            ..ComposerOutput::default()
        }
    }

    fn push_jamo(&mut self, jamo: char) -> ComposerOutput {
        self.composing_jamo.push(jamo);
        self.word_jamo.push(jamo);
        let mut committed = String::new();
        loop {
            let syllables = recompose(&self.composing_jamo);
            if syllables.len() <= 2 {
                break;
            }
            let oldest_jamo_count = syllables[0].jamo_count;
            committed.push(syllables[0].render());
            self.composing_jamo.drain(..oldest_jamo_count);
        }
        let commit = (!committed.is_empty()).then(|| CommittedText::plain(committed));
        self.composing_output(commit)
    }

    fn commit_all(&mut self) -> Option<CommittedText> {
        let text = render_all(&recompose(&self.composing_jamo));
        self.composing_jamo.clear();
        (!text.is_empty()).then(|| CommittedText::plain(text))
    }

    fn end_word(&mut self, separator: char) -> ComposerOutput {
        let boundary = WordBoundary {
            separator,
            key: encode_jamo_ascii(&self.word_jamo).unwrap_or_default(),
            surface: compose_word(&self.word_jamo),
        };
        self.word_jamo.clear();
        ComposerOutput {
            commit: self.commit_all(),
            boundary: Some(boundary),
            ..ComposerOutput::default()
        }
    }
}

impl Composer for HangulComposer {
    fn feed(&mut self, event: ComposerEvent, context: &EditorContext) -> ComposerOutput {
        match event {
            ComposerEvent::Key(character) if is_jamo(character) => {
                let adopted = self.try_adopt(context);
                let mut output = self.push_jamo(character);
                output.delete_before_commit = adopted;
                output
            }
            ComposerEvent::Separator(' ') if self.composing_jamo.is_empty() => {
                self.word_jamo.clear();
                ComposerOutput {
                    commit: Some(CommittedText::plain(" ".to_string())),
                    ..ComposerOutput::default()
                }
            }
            ComposerEvent::Key(character) | ComposerEvent::Separator(character) => {
                self.end_word(character)
            }
            ComposerEvent::Backspace => {
                let adopted = self.try_adopt(context);
                if adopted == 0 && self.composing_jamo.is_empty() {
                    self.word_jamo.clear();
                    return ComposerOutput {
                        delete_before_commit: 1,
                        ..ComposerOutput::default()
                    };
                }
                self.composing_jamo.pop();
                self.word_jamo.pop();
                let mut output = self.composing_output(None);
                output.delete_before_commit = adopted;
                output
            }
            ComposerEvent::CandidateSelected(text) => {
                // 어절 중 composing 창 밖으로 이미 확정된 앞부분을 지우고, commit이
                // composing 구간을 치환한다. 선택 뒤 타이핑은 새 입력 시퀀스.
                let word_length = compose_word(&self.word_jamo).chars().count();
                let composing_length = render_all(&recompose(&self.composing_jamo)).chars().count();
                self.composing_jamo.clear();
                self.word_jamo.clear();
                ComposerOutput {
                    delete_before_commit: word_length - composing_length,
                    commit: Some(CommittedText::plain(format!("{text} "))),
                    ..ComposerOutput::default()
                }
            }
        }
    }

    fn finalize(&mut self) -> Option<CommittedText> {
        self.word_jamo.clear();
        self.commit_all()
    }

    fn is_composing(&self) -> bool {
        !self.composing_jamo.is_empty()
    }

    /// composing 창과 어절 자모를 구분자로 이어 담는다 — 자모에는 없는 글자다.
    fn snapshot(&self) -> ComposerState {
        let composing: String = self.composing_jamo.iter().collect();
        let word: String = self.word_jamo.iter().collect();
        ComposerState::from_text(&format!("{composing}{STATE_SEPARATOR}{word}"))
    }

    fn restore(&mut self, state: ComposerState) {
        let Some((composing, word)) = state
            .text()
            .and_then(|text| text.split_once(STATE_SEPARATOR))
        else {
            return;
        };
        self.composing_jamo = composing.chars().collect();
        self.word_jamo = word.chars().collect();
    }
}
