use super::jamo::{
    compose_word, decode_jamo_ascii, decompose, encode_jamo_ascii, is_jamo, recompose, render_all,
};
use crate::contract::{
    Candidate, CandidateKind, CommittedText, Composer, ComposerEnvironment, ComposerEvent,
    ComposerOutput, ComposerState, ComposingText, EditorContext,
};
use crate::policy::double_space_period;

const SUGGESTION_LIMIT: usize = 3;

/// 두벌식 자모 오토마타. composing 창은 최대 2음절(직전 + 현재) — 도깨비불 발생 후에도
/// Backspace로 이전 상태("가바" → "갑")로 복귀할 수 있게 marked text 안에 유지하고,
/// 세 번째 음절이 시작될 때 가장 오래된 음절을 확정한다.
#[derive(Debug, Default)]
pub struct HangulComposer {
    composing_jamo: Vec<char>,
    /// 현재 어절 전체의 자모 — composing 창(2음절)보다 길 수 있으며 제안의 기준이다
    word_jamo: Vec<char>,
    candidates: Vec<Candidate>,
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

    fn suggest(&mut self, environment: &ComposerEnvironment<'_>) {
        self.candidates.clear();
        if self.word_jamo.is_empty() || !environment.context.field.assistance_enabled() {
            return;
        }
        let Some(lexicon) = environment.pack.and_then(|pack| pack.lexicon()) else {
            return;
        };
        let Some(encoded) = encode_jamo_ascii(&self.word_jamo) else {
            return;
        };
        let personalization = &*environment.personalization;
        let mut ranked: Vec<(u32, u32, String)> = Vec::new();
        let mut push = |distance: u32, frequency: u32, encoded_word: &str| {
            let Some(jamo) = decode_jamo_ascii(encoded_word) else {
                return;
            };
            let display = compose_word(&jamo);
            let weight = frequency + personalization.weight(&display);
            ranked.push((distance, weight, display));
        };
        for completion in lexicon.complete(&encoded, SUGGESTION_LIMIT + 1) {
            push(0, completion.frequency, &completion.word);
        }
        for correction in lexicon.corrections(&encoded, 1, SUGGESTION_LIMIT + 1) {
            push(correction.distance, correction.frequency, &correction.word);
        }
        ranked.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| b.1.cmp(&a.1))
                .then_with(|| a.2.cmp(&b.2))
        });
        for (_, _, display) in ranked {
            if self.candidates.len() >= SUGGESTION_LIMIT {
                break;
            }
            if self
                .candidates
                .iter()
                .any(|candidate| candidate.text == display)
            {
                continue;
            }
            self.candidates.push(Candidate {
                text: display,
                kind: CandidateKind::Prediction,
            });
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
            candidates: self.candidates.clone(),
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

    fn record_word(&self, environment: &mut ComposerEnvironment<'_>) {
        if environment.context.incognito || !environment.context.field.assistance_enabled() {
            return;
        }
        let display = compose_word(&self.word_jamo);
        if !display.is_empty() {
            environment.personalization.record(&display);
        }
    }

    fn clear_word(&mut self) {
        self.word_jamo.clear();
        self.candidates.clear();
    }

    fn commit_all(&mut self, trailing: Option<char>) -> Option<CommittedText> {
        let mut text = render_all(&recompose(&self.composing_jamo));
        self.composing_jamo.clear();
        if let Some(character) = trailing {
            text.push(character);
        }
        (!text.is_empty()).then(|| CommittedText::plain(text))
    }
}

impl Composer for HangulComposer {
    fn feed(
        &mut self,
        event: ComposerEvent,
        environment: &mut ComposerEnvironment<'_>,
    ) -> ComposerOutput {
        match event {
            ComposerEvent::Key(character) if is_jamo(character) => {
                let adopted = self.try_adopt(environment.context);
                let mut output = self.push_jamo(character);
                self.suggest(environment);
                output.candidates = self.candidates.clone();
                output.delete_before_commit = adopted;
                output
            }
            ComposerEvent::Separator(' ') if self.composing_jamo.is_empty() => {
                self.clear_word();
                match double_space_period(environment.context) {
                    Some(output) => output,
                    None => ComposerOutput {
                        commit: Some(CommittedText::plain(" ".to_string())),
                        ..ComposerOutput::default()
                    },
                }
            }
            ComposerEvent::Key(character) | ComposerEvent::Separator(character) => {
                self.record_word(environment);
                self.clear_word();
                ComposerOutput {
                    commit: self.commit_all(Some(character)),
                    ..ComposerOutput::default()
                }
            }
            ComposerEvent::Backspace => {
                let adopted = self.try_adopt(environment.context);
                if adopted == 0 && self.composing_jamo.is_empty() {
                    self.clear_word();
                    return ComposerOutput {
                        delete_before_commit: 1,
                        ..ComposerOutput::default()
                    };
                }
                self.composing_jamo.pop();
                self.word_jamo.pop();
                self.suggest(environment);
                let mut output = self.composing_output(None);
                output.delete_before_commit = adopted;
                output
            }
            ComposerEvent::CandidateSelected(index) => {
                let Some(candidate) = self.candidates.get(index).cloned() else {
                    return ComposerOutput::default();
                };
                // 어절 중 composing 창 밖으로 이미 확정된 앞부분을 지우고, commit이
                // composing 구간을 치환한다. 선택 뒤 타이핑은 새 입력 시퀀스.
                let word_length = compose_word(&self.word_jamo).chars().count();
                let composing_length = render_all(&recompose(&self.composing_jamo)).chars().count();
                if !environment.context.incognito {
                    environment.personalization.record(&candidate.text);
                }
                self.composing_jamo.clear();
                self.clear_word();
                ComposerOutput {
                    delete_before_commit: word_length - composing_length,
                    commit: Some(CommittedText::plain(format!("{} ", candidate.text))),
                    ..ComposerOutput::default()
                }
            }
        }
    }

    fn finalize(&mut self) -> Option<CommittedText> {
        self.clear_word();
        self.commit_all(None)
    }

    fn is_composing(&self) -> bool {
        !self.composing_jamo.is_empty()
    }

    fn snapshot(&self) -> ComposerState {
        ComposerState::Hangul {
            composing_jamo: self.composing_jamo.clone(),
            word_jamo: self.word_jamo.clone(),
        }
    }

    fn restore(&mut self, state: ComposerState) {
        if let ComposerState::Hangul {
            composing_jamo,
            word_jamo,
        } = state
        {
            self.composing_jamo = composing_jamo;
            self.word_jamo = word_jamo;
        }
    }
}
