use taza_engine::contract::{
    Composer, ComposerEvent, EditorContext, Effect, InputEvent, SuggestionRequest, UserPreferences,
};
use taza_engine::engine::Engine;
use taza_engine::keyboard::KeySignal;
use taza_engine::lang::LanguageDescriptor;
use taza_engine::lang::hangul::HangulComposer;

/// 이벤트 문자열: 자모/문자는 Key, '<'는 Backspace, '_'는 Separator(' '), '|'는 CursorMoved.
/// 문서 상태(committed + composing)를 유지하며 매 이벤트마다 커서 앞 문맥으로 전달한다.
fn run(events: &str) -> (String, Option<String>) {
    let mut engine = Engine::new(LanguageDescriptor::builtin("ko").unwrap());
    let mut committed = String::new();
    let mut composing: Option<String> = None;
    for character in events.chars() {
        let input = match character {
            '<' => InputEvent::Backspace,
            '_' => InputEvent::Separator(' '),
            '|' => InputEvent::CursorMoved,
            _ => InputEvent::Key(KeySignal::certain(character)),
        };
        let context = EditorContext {
            text_before_cursor: Some(format!("{committed}{}", composing.as_deref().unwrap_or(""))),
            incognito: false,
            field: taza_engine::contract::FieldKind::Text,
        };
        for effect in engine.handle(input, &context) {
            match effect {
                Effect::CommitText(text) => {
                    committed.push_str(&text);
                    composing = None;
                }
                Effect::SetComposing(text) => composing = Some(text.text),
                Effect::ClearComposing => composing = None,
                Effect::MoveCursor(_) => panic!("이 하네스는 커서를 옮기지 않는다"),
                Effect::DeleteBackward(count) => {
                    for _ in 0..count {
                        committed.pop();
                    }
                }
                Effect::UpdateCandidates(_) | Effect::SetTimer(_) => {}
            }
        }
    }
    (committed, composing)
}

#[track_caller]
fn assert_text(events: &str, expected_committed: &str, expected_composing: Option<&str>) {
    let (committed, composing) = run(events);
    assert_eq!(committed, expected_committed, "committed for {events:?}");
    assert_eq!(
        composing.as_deref(),
        expected_composing,
        "composing for {events:?}"
    );
}

#[test]
fn composes_basic_syllables() {
    assert_text("ㅇㅏㄴ", "", Some("안"));
    assert_text("ㅇㅏㄴㄴㅕㅇ", "", Some("안녕"));
}

#[test]
fn commits_oldest_syllable_when_window_overflows() {
    assert_text("ㅇㅏㄴㄴㅕㅇㅎㅏ", "안", Some("녕하"));
    assert_text("ㅇㅏㄴㄴㅕㅇㅎㅏㅅㅔㅇㅛ", "안녕하", Some("세요"));
}

#[test]
fn dokkaebibul_moves_final_consonant_to_next_syllable() {
    assert_text("ㄱㅏㅂ", "", Some("갑"));
    assert_text("ㄱㅏㅂㅏ", "", Some("가바"));
    assert_text("ㄱㅏㅂㅅㅣ", "", Some("갑시"));
    assert_text("ㄱㅏㅆㅏ", "", Some("가싸"));
}

#[test]
fn explicit_ieung_prevents_dokkaebibul() {
    assert_text("ㄱㅏㅂㅅㅇㅣ", "", Some("값이"));
}

#[test]
fn backspace_reverses_dokkaebibul() {
    assert_text("ㄱㅏㅂㅏ<", "", Some("갑"));
    assert_text("ㄱㅏㅂㅏ<<", "", Some("가"));
    assert_text("ㄱㅏㅂㅏ<<<", "", Some("ㄱ"));
    assert_text("ㄱㅏㅂㅏ<<<<", "", None);
}

#[test]
fn backspace_on_non_hangul_deletes_committed_text() {
    assert_text("ㄱㅏ_<", "가", None);
}

#[test]
fn combines_compound_vowels() {
    assert_text("ㄱㅗㅏㄴ", "", Some("관"));
    assert_text("ㅎㅗㅣㅅㅏ", "", Some("회사"));
}

#[test]
fn vowel_does_not_combine_after_jongseong() {
    assert_text("ㄱㅏㄹㅏ", "", Some("가라"));
}

#[test]
fn standalone_jamo_do_not_merge() {
    assert_text("ㅏㄱ", "", Some("ㅏㄱ"));
    assert_text("ㄱㄷ", "", Some("ㄱㄷ"));
    assert_text("ㅁㅏㅣ", "", Some("마ㅣ"));
}

#[test]
fn tense_consonants_cannot_be_jongseong() {
    assert_text("ㄱㅏㄸ", "", Some("가ㄸ"));
}

#[test]
fn jongseong_clusters() {
    assert_text("ㄷㅏㄹㄱ", "", Some("닭"));
    assert_text("ㄱㅏㅂㅅ", "", Some("값"));
}

/// 세벌식이 내는 타건 — 초성·중성·종성을 제 키로 치므로 완성형 음절의 정준 분해가 곧
/// 타건 순서다. 조합용 자모를 소스에 그대로 적으면 눈에는 완성형과 구별되지 않아
/// 읽는 사람이 무엇을 시험하는지 알 수 없다.
fn sebeolsik(text: &str) -> String {
    let mut events = String::new();
    for syllable in text.chars() {
        let offset = syllable as u32 - 0xAC00;
        events.push(char::from_u32(0x1100 + offset / 588).unwrap());
        events.push(char::from_u32(0x1161 + (offset / 28) % 21).unwrap());
        if !offset.is_multiple_of(28) {
            events.push(char::from_u32(0x11A7 + offset % 28).unwrap());
        }
    }
    events
}

/// 세벌식은 자리를 밝힌 자모를 내므로 합성기가 앞뒤를 추론하지 않는다 — 두벌식에서
/// "가바"가 되는 타건이 세벌식에서는 종성을 제자리에 남긴다.
#[test]
fn sebeolsik_has_no_dokkaebibul() {
    // 갑 + 중성 ㅏ
    assert_text(&format!("{}\u{1161}", sebeolsik("갑")), "", Some("갑ㅏ"));
    assert_text(&sebeolsik("각각"), "", Some("각각"));
    assert_text(&sebeolsik("각각각"), "각", Some("각각"));
}

/// 겹받침을 한 키로 내는 자리가 있으므로 자모 하나가 곧 한 타건이다 — Backspace도
/// 그 단위로 되돌린다.
#[test]
fn sebeolsik_types_clusters_with_one_key() {
    assert_text(&sebeolsik("값"), "", Some("값"));
    assert_text(&format!("{}<", sebeolsik("값")), "", Some("가"));
}

/// 복합 모음 키가 없는 자리(세벌식 최종의 ㅘ)는 중성 둘을 이어 쳐서 만든다.
#[test]
fn sebeolsik_combines_compound_vowels_from_two_keys() {
    assert_text("\u{1100}\u{1169}\u{1161}\u{11AB}", "", Some("관"));
}

/// 사전은 두벌식 타건 순서로 저장되므로, 어느 배열로 쳤든 조회 키는 같아야 한다.
#[test]
fn sebeolsik_and_dubeolsik_look_up_the_same_key() {
    let context = EditorContext::unavailable();
    let lookup = |events: &str| {
        let mut composer = HangulComposer::new();
        let mut request = SuggestionRequest::None;
        for character in events.chars() {
            request = composer
                .feed(ComposerEvent::Key(character), &context)
                .suggest;
        }
        request
    };
    assert_eq!(lookup(&sebeolsik("값")), lookup("ㄱㅏㅂㅅ"));
    assert_eq!(lookup(&sebeolsik("관")), lookup("ㄱㅗㅏㄴ"));
}

#[test]
fn separator_commits_window() {
    assert_text("ㄱㅏ_", "가 ", None);
}

#[test]
fn double_space_inserts_period() {
    assert_text("ㄱㅏ__", "가. ", None);
}

/// 더블 스페이스 마침표는 언어와 무관한 규칙이라 합성기가 아니라 Engine이 본다 —
/// 설정을 끄면 한글에서도 공백 두 개가 그대로 남는다.
#[test]
fn double_space_period_setting_reaches_hangul() {
    let mut engine = Engine::new(LanguageDescriptor::builtin("ko").unwrap());
    engine.set_preferences(UserPreferences {
        double_space_period: false,
        ..UserPreferences::default()
    });
    let mut committed = String::new();
    for input in [
        InputEvent::Key(KeySignal::certain('ㄱ')),
        InputEvent::Key(KeySignal::certain('ㅏ')),
        InputEvent::Separator(' '),
        InputEvent::Separator(' '),
    ] {
        let context = EditorContext {
            text_before_cursor: Some(committed.clone()),
            ..EditorContext::unavailable()
        };
        for effect in engine.handle(input, &context) {
            match effect {
                Effect::CommitText(text) => committed.push_str(&text),
                Effect::DeleteBackward(count) => {
                    for _ in 0..count {
                        committed.pop();
                    }
                }
                _ => {}
            }
        }
    }
    assert_eq!(committed, "가  ");
}

#[test]
fn non_jamo_key_commits_window_then_inserts() {
    assert_text("ㄱㅏ1", "가1", None);
}

#[test]
fn resumes_composition_after_cursor_move() {
    assert_text("ㄱㅏㅂ|ㅏ", "", Some("가바"));
    assert_text("ㄱㅏ|ㅂ", "", Some("갑"));
}

#[test]
fn decomposes_committed_syllable_on_backspace_after_cursor_move() {
    assert_text("ㅇㅏㄴㄴㅕㅇ|<", "안", Some("녀"));
    assert_text("ㅇㅏㄴㄴㅕㅇ|<<", "안", Some("ㄴ"));
    assert_text("ㅇㅏㄴㄴㅕㅇ|<<<", "안", None);
    assert_text("ㅇㅏㄴㄴㅕㅇ|<<<<", "", Some("아"));
}

#[test]
fn decomposes_compound_vowel_when_resuming() {
    assert_text("ㄱㅗㅏㄴ|<", "", Some("과"));
    assert_text("ㄱㅗㅏㄴ|<<", "", Some("고"));
}

#[test]
fn resume_skips_when_context_unavailable() {
    let mut engine = Engine::new(LanguageDescriptor::builtin("ko").unwrap());
    let effects = engine.handle(InputEvent::Backspace, &EditorContext::unavailable());
    assert_eq!(effects, vec![Effect::DeleteBackward(1)]);
}

#[test]
fn cursor_move_finalizes_composing() {
    let mut engine = Engine::new(LanguageDescriptor::builtin("ko").unwrap());
    let context = EditorContext::unavailable();
    engine.handle(InputEvent::Key(KeySignal::certain('ㄱ')), &context);
    engine.handle(InputEvent::Key(KeySignal::certain('ㅏ')), &context);
    let effects = engine.handle(InputEvent::CursorMoved, &context);
    assert_eq!(
        effects,
        vec![Effect::CommitText("가".to_string()), Effect::ClearComposing,]
    );
}

#[test]
fn snapshot_roundtrip_preserves_composing() {
    let context = EditorContext::unavailable();
    let mut composer = HangulComposer::new();
    composer.feed(ComposerEvent::Key('ㄱ'), &context);
    composer.feed(ComposerEvent::Key('ㅏ'), &context);
    composer.feed(ComposerEvent::Key('ㅂ'), &context);
    let state = composer.snapshot();
    assert_eq!(state.text(), Some("ㄱㅏㅂ\tㄱㅏㅂ"));

    let mut restored = HangulComposer::new();
    restored.restore(state);
    let output = restored.feed(ComposerEvent::Key('ㅏ'), &context);
    assert_eq!(output.composing.unwrap().text, "가바");
}

/// 커서가 한글이 아닌 자리로 옮겨 갔으면 어절도 거기서 새로 시작한다. 쥐고 있던 자모를
/// 그대로 두면 남의 자리 어절로 제안하고, 경계에서는 그 길이만큼 지운다.
#[test]
fn the_word_follows_the_caret() {
    let mut composer = HangulComposer::new();
    let typing = EditorContext {
        text_before_cursor: Some(String::new()),
        incognito: false,
        field: taza_engine::contract::FieldKind::Text,
    };
    // 세 음절을 치면 첫 음절은 조합 창 밖으로 확정되고 어절 자모에만 남는다
    for character in "ㄱㅏㄴㅏㄷㅏ".chars() {
        composer.feed(ComposerEvent::Key(character), &typing);
    }
    // 조합 창을 다 물러 비운다 — 어절 자모에는 확정된 앞부분이 남는다
    let unavailable = EditorContext::unavailable();
    for _ in 0..4 {
        composer.feed(ComposerEvent::Backspace, &unavailable);
    }
    let elsewhere = EditorContext {
        text_before_cursor: Some("abc".to_string()),
        ..typing.clone()
    };
    let output = composer.feed(ComposerEvent::Key('ㅁ'), &elsewhere);
    assert_eq!(
        output.suggest,
        SuggestionRequest::Word {
            key: "a".to_string()
        }
    );
    assert_eq!(output.delete_before_commit, 0);

    // 경계에서 지우는 것도 그 자리의 어절뿐이다
    let boundary = composer.feed(ComposerEvent::Separator(' '), &elsewhere);
    assert_eq!(
        boundary.boundary.map(|boundary| boundary.surface),
        Some("ㅁ".to_string())
    );
}
