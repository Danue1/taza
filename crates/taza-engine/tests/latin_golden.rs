use std::sync::Arc;
use taza_engine::contract::{
    CandidateKind, EditorContext, Effect, FieldKind, InputEvent, UserPreferences,
};
use taza_engine::engine::Engine;
use taza_engine::keyboard::KeySignal;
use taza_engine::lang::LanguageDescriptor;
use taza_engine::pack::{Pack, SectionKind};
use taza_pack::PackWriter;
use taza_pack::section::lexicon::LexiconBuilder;
use taza_pack::section::ngram::NgramModelBuilder;

/// 빈도는 실제 영어 팩에서 가져온 정규화 점수다 — 축소한 숫자로는 편집 벌점 같은
/// 점수 공간 기준의 판단이 실팩과 다르게 동작한다.
fn english_pack() -> Vec<u8> {
    let mut lexicon = LexiconBuilder::new();
    for (word, frequency) in [
        ("the", 64788),
        ("then", 41708),
        ("they", 52449),
        ("theme", 22509),
        ("hello", 27497),
        ("help", 49682),
        ("quick", 32369),
        ("best", 30000),
        ("say", 28000),
    ] {
        lexicon.insert(word, frequency);
    }
    let mut ngram = NgramModelBuilder::new();
    for (left, right, weight) in [
        ("the", "quick", 32768),
        ("the", "best", 19660),
        ("quick", "help", 6553),
        // hello(80) > help(50)를 뒤집을 만한 문맥 — 재랭킹 검증용
        ("say", "hello", 32768),
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
    /// 후보 바에 실제로 나가는 목록 — 원문 슬롯을 포함한다
    shown: Vec<String>,
    incognito: bool,
    field: FieldKind,
}

impl Harness {
    fn new(pack_bytes: &[u8]) -> Self {
        let mut engine = Engine::new(LanguageDescriptor::builtin("en").unwrap());
        engine.load_pack(Arc::new(pack_bytes.to_vec())).unwrap();
        Harness {
            engine,
            committed: String::new(),
            candidates: Vec::new(),
            shown: Vec::new(),
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
                    // 셸이 보내는 선택 인덱스는 원문 슬롯을 포함한 목록 기준이므로
                    // 그대로 두고, 순위 검증용으로 원문을 뺀 목록을 따로 둔다 —
                    // 그 슬롯이 첫 자리를 지킨다는 계약은 engine.rs가 검증한다.
                    self.shown = candidates.iter().map(|c| c.text.clone()).collect();
                    self.candidates = candidates
                        .into_iter()
                        .filter(|candidate| candidate.kind != CandidateKind::Typed)
                        .map(|candidate| candidate.text)
                        .collect();
                }
                Effect::MoveCursor(_) => panic!("이 하네스는 커서를 옮기지 않는다"),
                // 라틴 배열에는 멀티탭 키가 없다
                Effect::SetTimer(_) => {}
                Effect::SetComposing(_) | Effect::ClearComposing => {
                    panic!("라틴 방식은 composing을 쓰지 않는다")
                }
            }
        }
    }

    /// 커서가 다른 자리로 옮겨 갔다 — 셸이 그것을 알아채지 못한 경우까지 재현하려고
    /// 코어에는 알리지 않고 커서 앞 텍스트만 바꾼다.
    fn move_caret(&mut self, text_before_cursor: &str) {
        self.committed = text_before_cursor.to_string();
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
    // 원문("th")은 첫 자리를 지키고 랭킹 후보가 그 뒤에 온다
    assert_eq!(harness.shown, vec!["th", "the", "they", "then"]);
}

/// 자동 대문자화가 문장 첫 자리마다 shift를 올리므로, 대문자를 접지 않으면 문장을
/// 여는 낱말마다 예측이 끊기고 대문자 하나가 오타로 계산돼 엉뚱한 낱말이 올라온다.
#[test]
fn sentence_initial_capital_still_predicts_and_keeps_its_case() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.type_text("Th");
    assert_eq!(harness.shown, vec!["Th", "The", "They", "Then"]);
}

/// 전부 대문자로 치면 후보도 전부 대문자다 — 되씌우는 꼴이 친 꼴을 따른다.
#[test]
fn all_caps_input_yields_all_caps_candidates() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.type_text("TH");
    assert_eq!(harness.shown, vec!["TH", "THE", "THEY", "THEN"]);
}

/// 사전에 있는 낱말은 문장 첫 자리에서도 교정 대상이 아니다.
#[test]
fn capitalized_known_word_is_not_autocorrected() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.type_text("The ");
    assert_eq!(harness.committed, "The ");
}

/// 교정은 대문자 꼴을 지킨 채 일어난다 — 문장 첫 낱말의 오타를 고치면서 소문자로
/// 되돌려 버리면 자동 대문자화가 방금 한 일이 없던 것이 된다.
#[test]
fn autocorrection_keeps_the_typed_capitalization() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.type_text("Teh ");
    assert_eq!(harness.committed, "The ");
}

/// 짝맞춤 부호는 기본으로 켜져 있고 아포스트로피를 U+2019로 바꾼다. 그 글자가 어절을
/// 끊으면 영어 축약형이 통째로 사전에 닿지 못한다 — 사전은 축약형을 싣고 있는데도.
#[test]
fn smart_apostrophe_keeps_the_word_together() {
    let mut lexicon = LexiconBuilder::new();
    lexicon.insert("don't", 40000);
    lexicon.insert("does", 30000);
    let mut writer = PackWriter::new("en");
    writer.add_section(SectionKind::Lexicon, lexicon.build());
    let bytes = writer.finish();

    let mut harness = Harness::new(&bytes);
    harness.type_text("don't");
    // 화면에는 짝맞춤 아포스트로피가 들어가고
    assert_eq!(harness.committed, "don\u{2019}t");
    // 후보는 그 꼴 그대로 나온다 — 고른 것과 친 것이 화면에서 달라 보이면 안 된다
    assert_eq!(harness.shown, vec!["don\u{2019}t"]);

    // 어절이 이어지므로 완성도 받는다
    let mut harness = Harness::new(&bytes);
    harness.type_text("don'");
    assert_eq!(harness.shown, vec!["don\u{2019}", "don\u{2019}t"]);
}

/// 곧은 따옴표를 쓰는 앱(코드 입력란 등)에서는 접을 것이 없으므로 그대로 조회한다.
#[test]
fn straight_apostrophe_still_reaches_the_same_entry() {
    let mut lexicon = LexiconBuilder::new();
    lexicon.insert("don't", 40000);
    let mut writer = PackWriter::new("en");
    writer.add_section(SectionKind::Lexicon, lexicon.build());
    let bytes = writer.finish();

    let mut harness = Harness::new(&bytes);
    harness.engine.set_preferences(UserPreferences {
        smart_punctuation: false,
        ..UserPreferences::default()
    });
    harness.type_text("don'");
    assert_eq!(harness.committed, "don'");
    assert_eq!(harness.shown, vec!["don'", "don't"]);
}

#[test]
fn exact_word_ranks_first() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.type_text("the");
    assert_eq!(harness.shown[0], "the");
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

/// 자동교정 직후의 Backspace는 지우는 것이 아니라 사용자가 친 원문을 되살린다 —
/// 교정이 틀렸을 때 빠져나오는 길이며 순정 키보드 관습이다.
#[test]
fn backspace_right_after_autocorrection_restores_the_typed_word() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.type_text("teh ");
    assert_eq!(harness.committed, "the ");

    harness.send(InputEvent::Backspace);
    assert_eq!(harness.committed, "teh");

    // 되돌린 뒤 이어 치면 그 어절이 그대로 이어진다
    harness.type_text("m");
    assert_eq!(harness.committed, "tehm");
}

/// 되돌리기는 교정 **바로 뒤**에만 유효하다. 한 번이라도 다른 입력이 지나가면
/// Backspace는 평범한 글자 삭제로 돌아간다.
#[test]
fn revert_expires_after_the_next_input() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.type_text("teh a");
    harness.send(InputEvent::Backspace);
    assert_eq!(harness.committed, "the ");
}

/// 교정 후보가 원문을 앞서지 못하면 원문을 그대로 둔다 — 교정은 사용자가 친 것을
/// 지우는 일이라 점수로 판단한다.
#[test]
fn correction_that_does_not_outscore_the_typed_word_is_skipped() {
    let mut lexicon = LexiconBuilder::new();
    // 편집 하나의 벌점(점수 공간의 1/4)을 못 넘는 희귀어
    lexicon.insert("thew", 1000);
    let mut writer = PackWriter::new("en");
    writer.add_section(SectionKind::Lexicon, lexicon.build());
    let bytes = writer.finish();

    let mut harness = Harness::new(&bytes);
    harness.type_text("thex ");
    assert_eq!(harness.committed, "thex ");
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
    let selected = harness.shown[1].clone();
    assert_eq!(selected, "the");
    harness.send(InputEvent::CandidateSelected(1));
    assert_eq!(harness.committed, "the ");
    assert_eq!(harness.candidates, vec!["quick", "best"]);

    // 띄어쓰기 없이 이어 타이핑해도 새 단어로 인지. 방금 확정한 "the"는 최근 사용
    // 보너스를 받아 앞에 선다 — 개인화 가중치가 팩 빈도와 같은 점수 공간에 있기 때문이다.
    harness.type_text("he");
    assert_eq!(harness.committed, "the he");
    assert_eq!(harness.shown, vec!["he", "the", "help", "hello"]);
}

#[test]
fn previous_word_reranks_current_suggestions() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    // 문맥이 없으면 사전 빈도대로 help(49682) > hello(27497)
    harness.type_text("hel");
    assert_eq!(harness.candidates[0], "help");

    // "say" 뒤에서는 언어모델이 hello를 끌어올린다
    let mut harness = Harness::new(&bytes);
    harness.type_text("say hel");
    assert_eq!(harness.candidates[0], "hello");
}

#[test]
fn backspace_shrinks_word_and_updates_suggestions() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.type_text("thex");
    harness.send(InputEvent::Backspace);
    assert_eq!(harness.committed, "the");
    assert_eq!(harness.shown[0], "the");
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
    assert_eq!(harness.shown[0], "theme");
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
fn auto_correction_off_keeps_the_typed_word_but_still_predicts() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.engine.set_preferences(UserPreferences {
        auto_correction: false,
        ..UserPreferences::default()
    });
    harness.type_text("teh ");
    assert_eq!(harness.committed, "teh ");

    // 교정만 꺼진 것이므로 후보 제안은 그대로 나온다
    harness.type_text("th");
    assert_eq!(harness.candidates, vec!["the", "they", "then"]);
}

#[test]
fn predictions_off_empties_the_candidate_bar_but_still_corrects() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.engine.set_preferences(UserPreferences {
        predictions: false,
        ..UserPreferences::default()
    });
    harness.type_text("teh ");
    assert_eq!(harness.committed, "the ");
    assert!(harness.shown.is_empty());
}

#[test]
fn double_space_period_off_leaves_two_spaces() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.engine.set_preferences(UserPreferences {
        double_space_period: false,
        ..UserPreferences::default()
    });
    harness.type_text("the  ");
    assert_eq!(harness.committed, "the  ");
}

#[test]
fn personalized_learning_off_keeps_the_static_ranking() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.engine.set_preferences(UserPreferences {
        personalized_learning: false,
        ..UserPreferences::default()
    });
    harness.type_text("hello hello ");
    harness.type_text("he");
    assert_eq!(harness.candidates[0], "help");
}

/// 학습을 끄면 앞으로 배우지 않을 뿐 아니라 이미 배운 것도 쓰지 않는다 — 껐는데
/// 예전에 배운 말이 계속 순위를 흔들면 설정이 한 일이 눈에 보이지 않는다.
#[test]
fn personalized_learning_off_also_stops_using_what_was_learned() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.type_text("hello hello ");
    harness.type_text("he");
    assert_eq!(harness.candidates[0], "hello");

    harness.engine.set_preferences(UserPreferences {
        personalized_learning: false,
        ..UserPreferences::default()
    });
    harness.type_text(" he");
    assert_eq!(harness.candidates[0], "help");
}

/// 재설정은 배운 것을 지운다 — 학습으로 억제됐던 자동교정이 다시 살아난다.
#[test]
fn resetting_personalization_forgets_learned_words() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    for _ in 0..2 {
        harness.type_text("thw");
        let raw_index = harness
            .shown
            .iter()
            .position(|candidate| candidate == "thw")
            .unwrap();
        harness.send(InputEvent::CandidateSelected(raw_index));
    }
    harness.type_text("thw ");
    assert_eq!(harness.committed, "thw thw thw ");

    harness.engine.reset_personalization();
    harness.type_text("thw ");
    assert_eq!(harness.committed, "thw thw thw the ");
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
    harness.type_text("hello hello ");
    harness.field = FieldKind::Text;
    harness.type_text("he");
    // 비밀번호 입력은 학습되지 않아 기본 순위(help 우선) 유지
    assert_eq!(harness.candidates[0], "help");
}

#[test]
fn works_without_lexicon() {
    let mut engine = Engine::new(LanguageDescriptor::builtin("en").unwrap());
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
            personalization: Some(&personalization),
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
    // help(49682) > hello(27497)가 기본 — hello를 두 번 확정해 학습시키면 역전
    harness.type_text("hello hello ");
    harness.type_text("he");
    assert_eq!(harness.candidates[0], "hello");
}

#[test]
fn incognito_disables_learning() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.incognito = true;
    harness.type_text("hello hello ");
    harness.type_text("he");
    assert_eq!(harness.candidates[0], "help");
}

#[test]
fn selecting_raw_word_learns_it_and_suppresses_autocorrection() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);

    // "thw"는 자동교정 대상("the", 거리 1)이지만, 원문 후보를 두 번 선택해 학습시킨다
    for _ in 0..2 {
        harness.type_text("thw");
        let raw_index = harness
            .shown
            .iter()
            .position(|candidate| candidate == "thw")
            .unwrap();
        harness.send(InputEvent::CandidateSelected(raw_index));
    }
    assert_eq!(harness.committed, "thw thw ");

    // 학습 후에는 separator에서도 교정되지 않는다
    harness.type_text("thw ");
    assert_eq!(harness.committed, "thw thw thw ");
}

#[test]
fn personalization_snapshot_persists_learning() {
    use taza_engine::lang::latin::LatinComposer;
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.type_text("hello hello ");
    let state = harness.engine.personalization_snapshot();

    let mut restored = Harness::new(&bytes);
    restored.engine = Engine::with_composer(
        LanguageDescriptor::builtin("en").unwrap(),
        Box::new(LatinComposer::new()),
    );
    restored.engine.load_pack(Arc::new(bytes.clone())).unwrap();
    restored.engine.restore_personalization(state);
    restored.type_text("he");
    assert_eq!(restored.candidates[0], "hello");
}

/// 어절은 공백에서만 끝나지 않는다 — 순정도 마침표·쉼표에서 교정한다. 부호에서
/// 어절을 그냥 버리면 그 자리의 오타는 영영 고쳐지지 않고 배우지도 못한다.
#[test]
fn punctuation_ends_the_word_and_corrects_it() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.type_text("Teh.");
    assert_eq!(harness.committed, "The.");

    // 부호 뒤는 새 어절이다 — 앞 어절이 남아 그 뒤 교정에 끼어들지 않는다
    let mut harness = Harness::new(&bytes);
    harness.type_text("teh,teh ");
    assert_eq!(harness.committed, "the,the ");
}

/// 커서가 다른 자리로 옮겨 갔으면 지금 어절도 그 자리의 것이다. 쥐고 있던 어절로
/// 교정하면 남의 자리에서 그 길이만큼 글자를 지운다.
#[test]
fn the_word_follows_the_caret() {
    let bytes = english_pack();
    let mut harness = Harness::new(&bytes);
    harness.type_text("teh");
    // 사용자가 다른 문단을 짚었다 — 커서 앞에는 이미 끝난 어절과 공백뿐이다
    harness.move_caret("Say ");
    harness.type_text("he");
    // 후보도 그 자리의 어절을 따른다 — 쥐고 있던 "teh"가 앞에 붙으면 아무것도 못 찾는다
    assert_eq!(harness.shown, vec!["he", "help", "the", "hello"]);
    harness.type_text("llo ");
    assert_eq!(harness.committed, "Say hello ");

    // 커서가 낱말 뒤에 내려앉았으면 그 낱말을 이어 친다
    harness.move_caret("Say te");
    harness.type_text("h ");
    assert_eq!(harness.committed, "Say the ");
}
