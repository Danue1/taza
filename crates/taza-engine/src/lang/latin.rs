use crate::contract::{
    Candidate, CandidateKind, CommittedText, Composer, ComposerEnvironment, ComposerEvent,
    ComposerOutput, ComposerState, EditorContext, double_space_period,
};

const SUGGESTION_LIMIT: usize = 3;
/// 자동교정을 시도하는 최소 단어 길이 — 짧은 단어는 오교정 위험이 이득보다 크다
const AUTOCORRECT_MINIMUM_LENGTH: usize = 2;

fn is_word_character(character: char) -> bool {
    character.is_alphabetic() || character == '\''
}

/// 라틴 골격: composing 없이 글자를 즉시 확정하고(플랫폼 관습 — 영어는 marked text를
/// 쓰지 않는다), 현재 단어를 내부에서 추적해 제안 스트립과 자동교정을 만든다.
/// 제안 채택·자동교정은 delete_before_commit으로 확정 텍스트를 치환한다.
/// 단어 경계에서는 언어모델 + 개인화로 다음 단어를 예측해 후보 바를 채운다.
#[derive(Debug, Default)]
pub struct LatinComposer {
    current_word: String,
    candidates: Vec<Candidate>,
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

    fn suggest(&mut self, environment: &ComposerEnvironment<'_>) {
        self.candidates.clear();
        if self.current_word.is_empty() || !environment.context.field.assistance_enabled() {
            return;
        }
        let personalization = &*environment.personalization;
        let lexicon = environment.pack.and_then(|pack| pack.lexicon());
        let in_lexicon = lexicon
            .as_ref()
            .is_some_and(|lexicon| lexicon.contains(&self.current_word));
        if in_lexicon || personalization.is_learned(&self.current_word) {
            self.candidates.push(Candidate {
                text: self.current_word.clone(),
                kind: CandidateKind::Prediction,
            });
        }
        // 효과 빈도 = 사전 빈도 + 개인화 가중치 — 랭킹 결합의 v1 형태
        let mut ranked: Vec<(u32, u32, String, CandidateKind)> = Vec::new();
        if let Some(lexicon) = &lexicon {
            for completion in lexicon.complete(&self.current_word, SUGGESTION_LIMIT + 1) {
                let weight = completion.frequency + personalization.weight(&completion.word);
                ranked.push((0, weight, completion.word, CandidateKind::Prediction));
            }
            for correction in lexicon.corrections(&self.current_word, 1, SUGGESTION_LIMIT + 1) {
                let weight = correction.frequency + personalization.weight(&correction.word);
                ranked.push((
                    correction.distance,
                    weight,
                    correction.word,
                    CandidateKind::Correction,
                ));
            }
        }
        for (word, weight) in personalization.complete(&self.current_word, SUGGESTION_LIMIT) {
            ranked.push((0, weight, word, CandidateKind::Prediction));
        }
        ranked.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| b.1.cmp(&a.1))
                .then_with(|| a.2.cmp(&b.2))
        });
        for (_, _, word, kind) in ranked {
            if self.candidates.len() >= SUGGESTION_LIMIT {
                break;
            }
            if self
                .candidates
                .iter()
                .any(|candidate| candidate.text == word)
            {
                continue;
            }
            self.candidates.push(Candidate { text: word, kind });
        }
        // 미등재 단어는 원문 그대로도 선택할 수 있게 끝에 노출 — 자동교정을 피해
        // 선택하는 것이 곧 학습 경로다. 자동교정이 없는 상황(사전 없음)에서는 불필요.
        if lexicon.is_some()
            && !in_lexicon
            && !self
                .candidates
                .iter()
                .any(|candidate| candidate.text == self.current_word)
        {
            self.candidates.push(Candidate {
                text: self.current_word.clone(),
                kind: CandidateKind::Prediction,
            });
        }
    }

    /// 단어 확정 직후 — 후보 바를 다음 단어 예측(LM + 개인화)으로 채운다.
    fn predict_after(&mut self, committed_word: &str, environment: &ComposerEnvironment<'_>) {
        self.candidates.clear();
        if !environment.context.field.assistance_enabled() {
            return;
        }
        let Some(language_model) = environment.pack.and_then(|pack| pack.language_model()) else {
            return;
        };
        let personalization = &*environment.personalization;
        let mut predictions = language_model.predict_next(committed_word, SUGGESTION_LIMIT);
        predictions.sort_by(|a, b| {
            let weight_a = a.weight + personalization.weight(&a.word);
            let weight_b = b.weight + personalization.weight(&b.word);
            weight_b.cmp(&weight_a).then_with(|| a.word.cmp(&b.word))
        });
        for prediction in predictions {
            self.candidates.push(Candidate {
                text: prediction.word,
                kind: CandidateKind::Prediction,
            });
        }
    }

    fn record(&self, word: &str, environment: &mut ComposerEnvironment<'_>) {
        if !environment.context.incognito && environment.context.field.assistance_enabled() {
            environment.personalization.record(word);
        }
    }

    fn output(&self, delete_before_commit: usize, commit: Option<CommittedText>) -> ComposerOutput {
        ComposerOutput {
            delete_before_commit,
            commit,
            composing: None,
            candidates: self.candidates.clone(),
        }
    }

    fn autocorrection(&self, environment: &ComposerEnvironment<'_>) -> Option<String> {
        if !environment.context.field.assistance_enabled() {
            return None;
        }
        // 학습된 단어(사용자 어휘)는 사전에 없어도 교정하지 않는다
        if environment.personalization.is_learned(&self.current_word) {
            return None;
        }
        let lexicon = environment.pack.and_then(|pack| pack.lexicon())?;
        if self.current_word.chars().count() < AUTOCORRECT_MINIMUM_LENGTH
            || lexicon.contains(&self.current_word)
        {
            return None;
        }
        lexicon
            .corrections(&self.current_word, 1, 1)
            .into_iter()
            .next()
            .map(|correction| correction.word)
    }
}

impl Composer for LatinComposer {
    fn feed(
        &mut self,
        event: ComposerEvent,
        environment: &mut ComposerEnvironment<'_>,
    ) -> ComposerOutput {
        match event {
            ComposerEvent::Key(character) if is_word_character(character) => {
                self.adopt_from_context(environment.context);
                self.current_word.push(character);
                self.suggest(environment);
                self.output(0, Some(CommittedText::plain(character.to_string())))
            }
            ComposerEvent::Key(character) => {
                self.current_word.clear();
                self.candidates.clear();
                self.output(0, Some(CommittedText::plain(character.to_string())))
            }
            ComposerEvent::Separator(' ') if self.current_word.is_empty() => {
                self.candidates.clear();
                match double_space_period(environment.context) {
                    Some(output) => output,
                    None => self.output(0, Some(CommittedText::plain(" ".to_string()))),
                }
            }
            ComposerEvent::Separator(character) => match self.autocorrection(environment) {
                Some(corrected) => {
                    let original = std::mem::take(&mut self.current_word);
                    self.record(&corrected, environment);
                    self.predict_after(&corrected, environment);
                    self.output(
                        original.chars().count(),
                        Some(CommittedText {
                            surface: format!("{corrected}{character}"),
                            reading: None,
                            corrected_from: Some(original),
                        }),
                    )
                }
                None => {
                    let word = std::mem::take(&mut self.current_word);
                    self.record(&word, environment);
                    self.predict_after(&word, environment);
                    self.output(0, Some(CommittedText::plain(character.to_string())))
                }
            },
            ComposerEvent::Backspace => {
                self.adopt_from_context(environment.context);
                if self.current_word.pop().is_some() {
                    self.suggest(environment);
                } else {
                    self.candidates.clear();
                }
                self.output(1, None)
            }
            ComposerEvent::CandidateSelected(index) => {
                let Some(candidate) = self.candidates.get(index).cloned() else {
                    return ComposerOutput::default();
                };
                let original = std::mem::take(&mut self.current_word);
                self.record(&candidate.text, environment);
                self.predict_after(&candidate.text, environment);
                // 선택 확정 뒤의 타이핑은 새 입력 시퀀스 — 후행 공백이 그 경계다
                self.output(
                    original.chars().count(),
                    Some(CommittedText {
                        surface: format!("{} ", candidate.text),
                        reading: None,
                        corrected_from: (!original.is_empty() && candidate.text != original)
                            .then_some(original),
                    }),
                )
            }
        }
    }

    fn finalize(&mut self) -> Option<CommittedText> {
        self.current_word.clear();
        self.candidates.clear();
        None
    }

    fn is_composing(&self) -> bool {
        false
    }

    fn snapshot(&self) -> ComposerState {
        ComposerState::Latin {
            current_word: self.current_word.clone(),
        }
    }

    fn restore(&mut self, state: ComposerState) {
        if let ComposerState::Latin { current_word } = state {
            self.current_word = current_word;
        }
    }
}
