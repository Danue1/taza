use taza_core::composer::hangul::{HangulComposer, decompose_word, encode_jamo_ascii};
use taza_core::composer::EditorContext;
use taza_core::session::{Effect, InputEvent, Session};
use taza_pack::lexicon::LexiconBuilder;
use taza_pack::{Pack, PackWriter, SectionKind};

fn korean_pack() -> Vec<u8> {
    let mut lexicon = LexiconBuilder::new();
    for (word, frequency) in [("안녕", 90), ("안녕하세요", 80), ("안내", 50)] {
        let encoded = encode_jamo_ascii(&decompose_word(word).unwrap()).unwrap();
        lexicon.insert(&encoded, frequency);
    }
    let mut writer = PackWriter::new("ko");
    writer.add_section(SectionKind::Lexicon, lexicon.build());
    writer.finish()
}

struct Harness<'bytes> {
    session: Session,
    pack: Pack<'bytes>,
    committed: String,
    composing: Option<String>,
    candidates: Vec<String>,
}

impl<'bytes> Harness<'bytes> {
    fn new(pack_bytes: &'bytes [u8]) -> Self {
        Harness {
            session: Session::new(Box::new(HangulComposer::new())),
            pack: Pack::open(pack_bytes).unwrap(),
            committed: String::new(),
            composing: None,
            candidates: Vec::new(),
        }
    }

    fn send(&mut self, event: InputEvent) {
        let context = EditorContext {
            text_before_cursor: Some(format!(
                "{}{}",
                self.committed,
                self.composing.as_deref().unwrap_or("")
            )),
            incognito: false,
        };
        let pack = &self.pack;
        for effect in self.session.handle(event, &context, Some(pack)) {
            match effect {
                Effect::CommitText(text) => {
                    self.committed.push_str(&text);
                    self.composing = None;
                }
                Effect::SetComposing(text) => self.composing = Some(text.text),
                Effect::ClearComposing => self.composing = None,
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
            }
        }
    }

    fn type_jamo(&mut self, jamo_text: &str) {
        for character in jamo_text.chars() {
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
fn suggests_completions_at_jamo_level() {
    let bytes = korean_pack();
    let mut harness = Harness::new(&bytes);
    harness.type_jamo("ㅇㅏㄴㄴ");
    assert_eq!(harness.composing.as_deref(), Some("안ㄴ"));
    assert_eq!(harness.candidates, vec!["안녕", "안녕하세요", "안내"]);
}

#[test]
fn corrects_jamo_level_typo() {
    let bytes = korean_pack();
    let mut harness = Harness::new(&bytes);
    // 안녕의 마지막 ㅇ을 인접 오타 ㄷ으로 — 자모 편집거리 1
    harness.type_jamo("ㅇㅏㄴㄴㅕㄷ");
    assert!(harness.candidates.contains(&"안녕".to_string()));
}

#[test]
fn selecting_candidate_replaces_whole_word() {
    let bytes = korean_pack();
    let mut harness = Harness::new(&bytes);
    // 안녕하세 — "안녕"은 이미 창 밖으로 확정, composing은 "하세"
    harness.type_jamo("ㅇㅏㄴㄴㅕㅇㅎㅏㅅㅔ");
    assert_eq!(harness.committed, "안녕");
    assert_eq!(harness.composing.as_deref(), Some("하세"));

    let index = harness
        .candidates
        .iter()
        .position(|candidate| candidate == "안녕하세요")
        .unwrap();
    harness.send(InputEvent::CandidateSelected(index));
    assert_eq!(harness.committed, "안녕하세요 ");
    assert_eq!(harness.composing, None);

    // 선택 뒤 타이핑은 새 시퀀스
    harness.type_jamo("ㄱㅏ");
    assert_eq!(harness.committed, "안녕하세요 ");
    assert_eq!(harness.composing.as_deref(), Some("가"));
}

#[test]
fn personalization_boosts_frequent_word() {
    let bytes = korean_pack();
    let mut harness = Harness::new(&bytes);
    // 안내(50) < 안녕(90)이 기본 — 안내를 두 번 확정해 학습시키면 역전
    harness.type_jamo("ㅇㅏㄴㄴㅐ ㅇㅏㄴㄴㅐ ");
    harness.type_jamo("ㅇㅏㄴ");
    assert_eq!(harness.candidates[0], "안내");
}

#[test]
fn works_without_pack() {
    let mut session = Session::new(Box::new(HangulComposer::new()));
    let context = EditorContext::unavailable();
    let effects = session.handle(InputEvent::Key('ㄱ'), &context, None);
    assert!(effects.iter().any(|effect| matches!(
        effect,
        Effect::SetComposing(text) if text.text == "ㄱ"
    )));
}
