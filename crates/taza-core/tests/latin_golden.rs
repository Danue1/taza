use taza_core::composer::latin::LatinComposer;
use taza_core::composer::{CandidateKind, EditorContext};
use taza_core::session::{Effect, InputEvent, Session};
use taza_pack::lexicon::LexiconBuilder;
use taza_pack::ngram::NgramModelBuilder;
use taza_pack::{Pack, PackWriter, SectionKind};

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
    ] {
        lexicon.insert(word, frequency);
    }
    let mut ngram = NgramModelBuilder::new();
    for (left, right, weight) in [
        ("the", "quick", 50),
        ("the", "best", 30),
        ("quick", "help", 10),
    ] {
        ngram.insert_bigram(left, right, weight);
    }
    let mut writer = PackWriter::new("en");
    writer.add_section(SectionKind::Lexicon, lexicon.build());
    writer.add_section(SectionKind::NgramModel, ngram.build());
    writer.finish()
}

struct Harness<'bytes> {
    session: Session,
    pack: Pack<'bytes>,
    committed: String,
    candidates: Vec<String>,
}

impl<'bytes> Harness<'bytes> {
    fn new(pack_bytes: &'bytes [u8]) -> Self {
        Harness {
            session: Session::new(Box::new(LatinComposer::new())),
            pack: Pack::open(pack_bytes).unwrap(),
            committed: String::new(),
            candidates: Vec::new(),
        }
    }

    fn send(&mut self, event: InputEvent) {
        let context = EditorContext {
            text_before_cursor: Some(self.committed.clone()),
        };
        let pack = &self.pack;
        for effect in self.session.handle(event, &context, Some(pack)) {
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
                InputEvent::Key(character)
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
    assert_eq!(harness.candidates, vec!["the", "then", "they"]);
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
    assert_eq!(harness.candidates, vec!["hello", "help", "the"]);
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

    harness.send(InputEvent::Key('m'));
    assert_eq!(harness.committed, "them");
    harness.send(InputEvent::Key('e'));
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
fn works_without_lexicon() {
    let mut session = Session::new(Box::new(LatinComposer::new()));
    let context = EditorContext::unavailable();
    let effects = session.handle(InputEvent::Key('h'), &context, None);
    assert_eq!(effects, vec![Effect::CommitText("h".to_string())]);
}

#[test]
fn suggestion_kinds_distinguish_completion_from_correction() {
    let bytes = english_pack();
    let pack = Pack::open(&bytes).unwrap();
    let mut composer = LatinComposer::new();
    let context = EditorContext::unavailable();
    use taza_core::composer::{Composer, ComposerEvent};
    composer.feed(ComposerEvent::Key('t'), &context, Some(&pack));
    composer.feed(ComposerEvent::Key('e'), &context, Some(&pack));
    let output = composer.feed(ComposerEvent::Key('h'), &context, Some(&pack));
    let correction = output
        .candidates
        .iter()
        .find(|candidate| candidate.text == "the")
        .unwrap();
    assert_eq!(correction.kind, CandidateKind::Correction);
}
