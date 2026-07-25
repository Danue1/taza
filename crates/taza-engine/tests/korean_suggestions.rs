use taza_engine::keyboard::KeySignal;
use std::sync::Arc;
use taza_engine::contract::{EditorContext, Effect, InputEvent};
use taza_engine::engine::Engine;
use taza_engine::lang::LanguageDescriptor;
use taza_engine::lang::jamo::{decompose_word, encode_jamo_ascii};
use taza_engine::pack::SectionKind;
use taza_toolchain::PackWriter;
use taza_toolchain::lexicon::LexiconBuilder;

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

struct Harness {
    engine: Engine,
    committed: String,
    composing: Option<String>,
    candidates: Vec<String>,
}

impl Harness {
    fn new(pack_bytes: &[u8]) -> Self {
        let mut engine = Engine::new(LanguageDescriptor::builtin("ko").unwrap()).unwrap();
        engine.load_pack(Arc::new(pack_bytes.to_vec())).unwrap();
        Harness {
            engine,
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
            field: taza_engine::contract::FieldKind::Text,
        };
        for effect in self.engine.handle(event, &context) {
            match effect {
                Effect::CommitText(text) => {
                    self.committed.push_str(&text);
                    self.composing = None;
                }
                Effect::SetComposing(text) => self.composing = Some(text.text),
                Effect::ClearComposing => self.composing = None,
                Effect::MoveCursor(_) => panic!("이 하네스는 커서를 옮기지 않는다"),
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
                InputEvent::Key(KeySignal::certain(character))
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

/// 사전에 없는 사용자 어휘가 한국어에서도 접두 완성으로 제안된다. 개인화 스토어가
/// 표시 형태가 아니라 자모 키 공간에 담기므로 성립한다 — "안다"의 접두는 "안ㄷ"이지
/// "안"이 아니기 때문에, 표시 공간에서는 접두 검색 자체가 어긋난다.
#[test]
fn personalized_word_absent_from_lexicon_is_completed() {
    let bytes = korean_pack();
    let mut harness = Harness::new(&bytes);
    harness.type_jamo("ㅇㅏㄴㄷㅏ ㅇㅏㄴㄷㅏ ");
    harness.type_jamo("ㅇㅏㄴ");
    assert_eq!(harness.candidates[0], "안다");
}

#[test]
fn works_without_pack() {
    let mut engine = Engine::new(LanguageDescriptor::builtin("ko").unwrap()).unwrap();
    let context = EditorContext::unavailable();
    let effects = engine.handle(InputEvent::Key(KeySignal::certain('ㄱ')), &context);
    assert!(effects.iter().any(|effect| matches!(
        effect,
        Effect::SetComposing(text) if text.text == "ㄱ"
    )));
}
