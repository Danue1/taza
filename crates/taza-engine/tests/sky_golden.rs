use taza_engine::contract::{Composer, ComposerEnvironment, ComposerEvent, EditorContext, Effect};
use taza_engine::engine::Engine;
use taza_engine::lang::LanguageDescriptor;
use taza_engine::lang::sky::SkyComposer;

/// 이벤트 문자열: 자모는 Key, '<'는 Backspace, '_'는 공백. 같은 키를 이어 누르는 멀티탭
/// (ㄱ→ㅋ→ㄲ, ㅏ→ㅑ)은 키보드가 판정하므로 여기서는 갈아 끼운 결과를 그대로 친다.
///
/// **문맥을 실어 보낸다**: 모음을 갈아 끼우는 중에는 지운 글자가 아직 문서에 남아 있어
/// 합성 재개(채택)가 그것을 도로 주워 올 수 있다. 문맥이 빈 하네스는 그 길을 열지 못한다.
fn run(events: &str) -> (String, Option<String>) {
    let mut composer = SkyComposer::new();
    let mut committed = String::new();
    let mut composing: Option<String> = None;
    for character in events.chars() {
        let event = match character {
            '<' => ComposerEvent::Backspace,
            '_' => ComposerEvent::Separator(' '),
            _ => ComposerEvent::Key(character),
        };
        let context = EditorContext {
            text_before_cursor: Some(format!("{committed}{}", composing.as_deref().unwrap_or(""))),
            ..EditorContext::unavailable()
        };
        let output = composer.feed(event, &ComposerEnvironment::new(&context));
        for _ in 0..output.delete_before_commit {
            committed.pop();
        }
        if let Some(commit) = output.commit {
            committed.push_str(&commit.surface);
        }
        composing = output.composing.map(|text| text.text);
    }
    (committed, composing)
}

#[track_caller]
fn assert_text(events: &str, expected: &str) {
    let (committed, composing) = run(events);
    assert_eq!(
        format!("{committed}{}", composing.unwrap_or_default()),
        expected,
        "for {events:?}"
    );
}

/// 프레임에서 라벨로 키를 찾아 그 한가운데를 짚는다.
#[track_caller]
fn key_center(engine: &Engine, label: &str) -> (f32, f32) {
    engine
        .frame()
        .rows
        .iter()
        .flatten()
        .find(|key| key.label == label)
        .map(|key| {
            (
                key.bounds.x + key.bounds.width / 2.0,
                key.bounds.y + key.bounds.height / 2.0,
            )
        })
        .unwrap_or_else(|| panic!("{label} 키가 없다"))
}

/// 모음을 이어 치면 합쳐진다 — 두벌식이라면 ㅏ 다음 ㅣ는 새 글자다.
#[test]
fn joins_consecutive_vowels() {
    assert_text("ㅇㅏㅣ", "애");
    assert_text("ㅇㅑㅣ", "얘");
    assert_text("ㅇㅓㅣ", "에");
    assert_text("ㅇㅕㅣ", "예");
    assert_text("ㅇㅗㅏ", "와");
    assert_text("ㅇㅗㅏㅣ", "왜");
    assert_text("ㅇㅗㅣ", "외");
    assert_text("ㅇㅜㅓ", "워");
    assert_text("ㅇㅜㅓㅣ", "웨");
    assert_text("ㅇㅜㅣ", "위");
    assert_text("ㅇㅡㅣ", "의");
}

/// 단모음은 그대로 선다 — 키에 있는 모음 열은 이어 치지 않아도 제 글자다.
#[test]
fn base_vowels_stand_on_their_own() {
    for (tap, expected) in [
        ("ㅏ", "아"),
        ("ㅑ", "야"),
        ("ㅓ", "어"),
        ("ㅕ", "여"),
        ("ㅗ", "오"),
        ("ㅛ", "요"),
        ("ㅜ", "우"),
        ("ㅠ", "유"),
        ("ㅡ", "으"),
        ("ㅣ", "이"),
    ] {
        assert_text(&format!("ㅇ{tap}"), expected);
    }
}

/// 합쳐질 수 없는 모음은 새 자모로 선다 — 이중모음은 겹모음의 앞자리가 아니다.
#[test]
fn a_vowel_that_cannot_join_stands_on_its_own() {
    assert_text("ㅇㅏㅗ", "아ㅗ");
    assert_text("ㅇㅛㅏ", "요ㅏ");
    assert_text("ㅇㅑㅏ", "야ㅏ");
}

/// 자음은 이 방식이 볼 것이 없다 — 멀티탭이 이미 갈아 끼운 자모가 두벌식 오토마타로
/// 그대로 흘러간다.
#[test]
fn consonants_flow_through_untouched() {
    assert_text("ㄱㅏㄴ", "간");
    assert_text("ㄲㅗㅊ", "꽃");
    assert_text("ㅎㅏㄱㄱㅛ", "학교");
}

/// 모음 타건을 하나 무르면 쌓던 모음이 앞 상태로 돌아간다.
#[test]
fn backspace_undoes_one_tap() {
    assert_text("ㅇㅏㅣ<", "아");
    assert_text("ㅇㅗㅏㅣ<", "와");
    assert_text("ㅇㅜㅓㅣ<", "워");
    // 첫 타건까지 무르면 그 모음이 통째로 사라진다
    assert_text("ㅇㅏ<", "ㅇ");
}

/// 어절이 끊기면 쌓던 타건도 끊긴다 — 공백 너머의 ㅣ가 앞 어절의 ㅏ를 ㅐ로 만들지 않는다.
#[test]
fn a_word_boundary_clears_the_taps() {
    assert_text("ㅇㅏ_ㅣ", "아ㅣ");
}

/// 낱말 하나를 통째로 — 위키가 드는 보기 그대로다(ㄴ ㅏ ㅁ ㅜ ㅇ ㅜ ㅣ ㅋ ㅣ).
#[test]
fn types_whole_words() {
    assert_text("ㄴㅏㅁㅜㅇㅜㅣㅋㅣ", "나무위키");
    assert_text("ㅇㅏㄴㄴㅕㅇ", "안녕");
    // 자음이 모음 타건을 끊으므로 ㅔ 다음의 ㅣ는 앞 모음에 붙지 않는다
    assert_text("ㅋㅓㅣㅇㅣㅋㅡ", "케이크");
}

/// 조회 키는 배열과 무관하다 — 베가로 친 어절도 두벌식 타건 순서로 사전을 찾는다.
#[test]
fn the_lookup_key_stays_dubeolsik() {
    let mut composer = SkyComposer::new();
    let context = EditorContext::unavailable();
    let mut request = Default::default();
    // "학"의 조회 키는 두벌식 타건 그대로 g(ㅎ) k(ㅏ) r(ㄱ)
    for tap in ['ㅎ', 'ㅏ', 'ㄱ'] {
        request = composer
            .feed(ComposerEvent::Key(tap), &ComposerEnvironment::new(&context))
            .suggest;
    }
    assert_eq!(
        request,
        taza_engine::contract::SuggestionRequest::Word {
            key: "gkr".to_string()
        }
    );
}

/// 된소리는 키캡에 없어도 세 번째 타건으로 닿는다 — 시프트가 없는 판이므로 이 길이
/// 막히면 된소리를 칠 수 없다.
#[test]
fn a_third_tap_reaches_the_tense_consonant() {
    let mut engine = Engine::new(LanguageDescriptor::builtin("ko").unwrap());
    assert!(engine.select_layout("베가"));

    let context = EditorContext::unavailable();
    let (x, y) = key_center(&engine, "ㄱㅋ");
    let mut composing = String::new();
    for _ in 0..3 {
        for effect in engine.press_at(x, y, &context).effects {
            if let Effect::SetComposing(text) = effect {
                composing = text.text;
            }
        }
    }
    assert_eq!(composing, "ㄲ");
}

/// 배열을 고르면 합성기가 함께 갈린다 — 베가 판에서 ㅏ 다음 ㅣ는 ㅐ이고, 그 결과가
/// 조합 창에 선다. 키캡은 순정처럼 된소리를 적지 않으므로 라벨은 "ㄱㅋ"이다.
#[test]
fn selecting_the_layout_switches_the_composer() {
    let mut engine = Engine::new(LanguageDescriptor::builtin("ko").unwrap());
    assert!(engine.select_layout("베가"));

    let context = EditorContext::unavailable();
    let (x, y) = key_center(&engine, "ㄱㅋ");
    engine.press_at(x, y, &context);
    let (x, y) = key_center(&engine, "ㅏㅑ");
    engine.press_at(x, y, &context);
    let (x, y) = key_center(&engine, "ㅣㅡ");
    let result = engine.press_at(x, y, &context);

    assert!(
        result.effects.iter().any(
            |effect| matches!(effect, Effect::SetComposing(composing) if composing.text == "개")
        ),
        "{:?}",
        result.effects
    );
}
