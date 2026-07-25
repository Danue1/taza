use taza_engine::contract::{
    Composer, ComposerEvent, EditorContext, Effect, InputEvent,
};
use taza_engine::engine::Engine;
use taza_engine::lang::LanguageDescriptor;
use taza_engine::lang::hangul::HangulComposer;

/// 이벤트 문자열: 자모/문자는 Key, '<'는 Backspace, '_'는 Separator(' '), '|'는 CursorMoved.
/// 문서 상태(committed + composing)를 유지하며 매 이벤트마다 커서 앞 문맥으로 전달한다.
fn run(events: &str) -> (String, Option<String>) {
    let mut engine = Engine::new(LanguageDescriptor::builtin("ko").unwrap()).unwrap();
    let mut committed = String::new();
    let mut composing: Option<String> = None;
    for character in events.chars() {
        let input = match character {
            '<' => InputEvent::Backspace,
            '_' => InputEvent::Separator(' '),
            '|' => InputEvent::CursorMoved,
            _ => InputEvent::Key(character),
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
                Effect::UpdateCandidates(_) => {}
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

#[test]
fn separator_commits_window() {
    assert_text("ㄱㅏ_", "가 ", None);
}

#[test]
fn double_space_inserts_period() {
    assert_text("ㄱㅏ__", "가. ", None);
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
    let mut engine = Engine::new(LanguageDescriptor::builtin("ko").unwrap()).unwrap();
    let effects = engine.handle(InputEvent::Backspace, &EditorContext::unavailable());
    assert_eq!(effects, vec![Effect::DeleteBackward(1)]);
}

#[test]
fn cursor_move_finalizes_composing() {
    let mut engine = Engine::new(LanguageDescriptor::builtin("ko").unwrap()).unwrap();
    let context = EditorContext::unavailable();
    engine.handle(InputEvent::Key('ㄱ'), &context);
    engine.handle(InputEvent::Key('ㅏ'), &context);
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
