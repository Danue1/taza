use std::sync::Arc;
use taza_engine::contract::{CandidateKind, EditorContext, Effect, FieldKind, InputEvent};
use taza_engine::engine::Engine;
use taza_engine::keyboard::KeySignal;
use taza_engine::lang::LanguageDescriptor;
use taza_engine::pack::{Pack, SectionKind};
use taza_toolchain::PackWriter;
use taza_toolchain::lexicon::LexiconBuilder;
use taza_toolchain::ngram::NgramModelBuilder;

fn english_pack() -> Vec<u8> {
    let mut lexicon = LexiconBuilder::new();
    for (word, frequency) in [
        ("the", 100),
        ("then", 70),
        ("they", 60),
        ("theme", 40),
        ("hello", 80),
        ("help", 50),
        ("quick", 30),
        ("best", 20),
        ("say", 10),
    ] {
        lexicon.insert(word, frequency);
    }
    let mut ngram = NgramModelBuilder::new();
    for (left, right, weight) in [
        ("the", "quick", 50),
        ("the", "best", 30),
        ("quick", "help", 10),
        // hello(80) > help(50)를 뒤집을 만한 문맥 — 재랭킹 검증용
        ("say", "help", 100),
    ] {
        ngram.insert_bigram(left, right, weight);
    }
    let mut writer = PackWriter::new("en");
    writer.add_section(SectionKind::Lexicon, lexicon.build());
    writer.add_section(SectionKind::NgramModel, ngram.build());
    writer.finish()
}

struct Harness {
    engine: Engine,
    committed: String,
    candidates: Vec<String>,
    incognito: bool,
    field: FieldKind,
}

impl Harness {
    fn new(pack_bytes: &[u8]) -> Self {
        let mut engine = Engine::new(LanguageDescriptor::builtin("en").unwrap()).unwrap();
        engine.load_pack(Arc::new(pack_bytes.to_vec())).unwrap();
        Harness {
            engine,
            committed: String::new(),
            candidates: Vec::new(),
            incognito: false,
            field: FieldKind::Text,
        }
    }

    fn send(&mut self, event: InputEvent) {
        let context = EditorContext {
            text_before_cursor: Some(self.committed.clone()),
            incognito: self.incognito,
            field: self.field,
        };
        for effect in self.engine.handle(event, &context) {
            match effect {
                Effect::CommitText(text) => self.committed.push_str(&text),
                Effect::DeleteBackward(count) => {
                    for _ in 0..count {
                        self.committed.pop();
                    }
                }
                Effect::UpdateCandidates(candidates) => {
                    self.candidates = candidates
                        .into_iter()
                        .map(|candidate| candidate.text)
                        .collect();
                }
                Effect::MoveCursor(_) => panic!("이 하네스는 커서를 옮기지 않는다"),
                Effect::SetComposing(_) | Effect::ClearComposing => {
                    panic!("라틴 골격은 composing을 쓰지 않는다")
                }
            }
        }
    }

    fn type_text(&mut self, text: &str) {
        for character in text.chars() {
            let event = if character == ' ' {
                InputEvent::Separator(' ')
            } else {
                InputEvent::Key(KeySignal::certain(character))
            };
            self.send(event);
        }
    }
}

#[test]
fn typing_commits_immediately_and_suggests() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.type_text("th");
    assert_eq!(harness.committed, "th");
    // 미등재 단어는 원문("th")이 끝에 붙는다 — 선택 시 학습 경로
    assert_eq!(harness.candidates, vec!["the", "then", "they", "th"]);
}

#[test]
fn exact_word_ranks_first() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.type_text("the");
    assert_eq!(harness.candidates[0], "the");
}

#[test]
fn autocorrects_on_separator() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.type_text("teh ");
    assert_eq!(harness.committed, "the ");
    // 교정된 단어 기준으로 다음 단어 예측이 이어진다
    assert_eq!(harness.candidates, vec!["quick", "best"]);
}

#[test]
fn known_word_is_not_autocorrected() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.type_text("then ");
    assert_eq!(harness.committed, "then ");
}

#[test]
fn word_without_nearby_match_is_kept() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.type_text("zzzz ");
    assert_eq!(harness.committed, "zzzz ");
}

#[test]
fn candidate_selection_replaces_word_and_starts_new_sequence() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.type_text("th");
    let selected = harness.candidates[0].clone();
    assert_eq!(selected, "the");
    harness.send(InputEvent::CandidateSelected(0));
    assert_eq!(harness.committed, "the ");
    assert_eq!(harness.candidates, vec!["quick", "best"]);

    // 띄어쓰기 없이 이어 타이핑해도 새 단어로 인지
    harness.type_text("he");
    assert_eq!(harness.committed, "the he");
    assert_eq!(harness.candidates, vec!["hello", "help", "the", "he"]);
}

#[test]
fn previous_word_reranks_current_suggestions() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    // 문맥이 없으면 사전 빈도대로 hello(80) > help(50)
    harness.type_text("hel");
    assert_eq!(harness.candidates[0], "hello");

    // "say" 뒤에서는 언어모델이 help를 끌어올린다
    let mut harness = Harness::new(&bytes);
    harness.type_text("say hel");
    assert_eq!(harness.candidates[0], "help");
}

#[test]
fn backspace_shrinks_word_and_updates_suggestions() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.type_text("thex");
    harness.send(InputEvent::Backspace);
    assert_eq!(harness.committed, "the");
    assert_eq!(harness.candidates[0], "the");
}

#[test]
fn resumes_word_after_cursor_move() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.type_text("the");
    harness.send(InputEvent::CursorMoved);
    assert!(harness.candidates.is_empty());

    harness.send(InputEvent::Key(KeySignal::certain('m')));
    assert_eq!(harness.committed, "them");
    harness.send(InputEvent::Key(KeySignal::certain('e')));
    assert_eq!(harness.committed, "theme");
    assert_eq!(harness.candidates[0], "theme");
}

#[test]
fn predicts_next_word_and_chains_selections() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.type_text("the ");
    assert_eq!(harness.candidates, vec!["quick", "best"]);

    harness.send(InputEvent::CandidateSelected(0));
    assert_eq!(harness.committed, "the quick ");
    assert_eq!(harness.candidates, vec!["help"]);

    harness.send(InputEvent::CandidateSelected(0));
    assert_eq!(harness.committed, "the quick help ");
    assert!(harness.candidates.is_empty());
}

#[test]
fn double_space_inserts_period() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.type_text("the  ");
    assert_eq!(harness.committed, "the. ");

    // 공백 하나 더 → 마침표 뒤라 치환 없음
    harness.send(InputEvent::Separator(' '));
    assert_eq!(harness.committed, "the.  ");
}

#[test]
fn email_field_disables_assistance() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.field = FieldKind::Email;
    harness.type_text("teh ");
    // 자동교정·제안 없음 — 주소를 건드리면 안 된다
    assert_eq!(harness.committed, "teh ");
    assert!(harness.candidates.is_empty());
}

#[test]
fn password_field_disables_learning() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.field = FieldKind::Password;
    harness.type_text("help help ");
    harness.field = FieldKind::Text;
    harness.type_text("he");
    // 비밀번호 입력은 학습되지 않아 기본 순위(hello 우선) 유지
    assert_eq!(harness.candidates[0], "hello");
}

#[test]
fn works_without_lexicon() {
    let mut engine = Engine::new(LanguageDescriptor::builtin("en").unwrap()).unwrap();
    let context = EditorContext::unavailable();
    let effects = engine.handle(InputEvent::Key(KeySignal::certain('h')), &context);
    assert_eq!(effects, vec![Effect::CommitText("h".to_string())]);
}

#[test]
fn suggestion_kinds_distinguish_completion_from_correction() {
    use taza_engine::personalization::PersonalizationStore;
    use taza_engine::suggest::{Suggester, SuggestionSources};
    let bytes = english_pack();
    let pack = Pack::open(&bytes).unwrap();
    let suggester = Suggester::new(
        LanguageDescriptor::builtin("en")
            .unwrap()
            .suggestion_policy(),
    );
    let personalization = PersonalizationStore::new();
    let suggestions = suggester.suggest(
        "teh",
        &SuggestionSources {
            pack: Some(&pack),
            personalization: &personalization,
            previous_word: None,
            touches: &[],
        },
    );
    let correction = suggestions
        .iter()
        .find(|suggestion| suggestion.text == "the")
        .unwrap();
    assert_eq!(correction.kind, CandidateKind::Correction);
}

#[test]
fn learned_word_outranks_static_frequency() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    // hello(80) > help(50)가 기본 — help를 두 번 확정해 학습시키면 역전
    harness.type_text("help help ");
    harness.type_text("he");
    assert_eq!(harness.candidates[0], "help");
}

#[test]
fn incognito_disables_learning() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.incognito = true;
    harness.type_text("help help ");
    harness.type_text("he");
    assert_eq!(harness.candidates[0], "hello");
}

#[test]
fn selecting_raw_word_learns_it_and_suppresses_autocorrection() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);

    // "thw"는 자동교정 대상("the", 거리 1)이지만, 원문 후보를 두 번 선택해 학습시킨다
    for _ in 0..2 {
        harness.type_text("thw");
        let raw_index = harness
            .candidates
            .iter()
            .position(|candidate| candidate == "thw")
            .unwrap();
        harness.send(InputEvent::CandidateSelected(raw_index));
    }
    assert_eq!(harness.committed, "thw thw ");

    // 학습 후에는 separator에서도 교정되지 않는다
    harness.type_text("thw ");
    assert_eq!(harness.committed, "thw thw thw ");

    // 개인화 어휘가 접두 완성으로도 제안된다
    harness.type_text("th");
    assert_eq!(harness.candidates[0], "thw");
}

#[test]
fn personalization_snapshot_persists_learning() {
    use taza_engine::lang::latin::LatinComposer;
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.type_text("help help ");
    let state = harness.engine.personalization_snapshot();

    let mut restored = Harness::new(&bytes);
    restored.engine = Engine::with_composer(LanguageDescriptor::builtin("en").unwrap(), Box::new(LatinComposer::new()));
    restored.engine.load_pack(Arc::new(bytes.clone())).unwrap();
    restored.engine.restore_personalization(state);
    restored.type_text("he");
    assert_eq!(restored.candidates[0], "help");
}
