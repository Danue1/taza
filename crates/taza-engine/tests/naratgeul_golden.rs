use taza_engine::contract::{
    Composer, ComposerEnvironment, ComposerEvent, EditorContext, Effect, InputEvent,
};
use taza_engine::engine::Engine;
use taza_engine::keyboard::KeySignal;
use taza_engine::lang::LanguageDescriptor;
use taza_engine::lang::naratgeul::{NaratgeulComposer, STROKE, TENSE};

/// 이벤트 문자열: 밑글자는 Key, '+'는 획 추가, '*'는 쌍자음, '<'는 Backspace, '_'는 공백.
/// 모음 키의 멀티탭(ㅏ→ㅓ)은 키보드가 판정하므로 여기서는 갈아 끼운 결과를 그대로 친다.
///
/// **문맥을 실어 보낸다**: 자모를 갈아 끼우는 중에는 지운 글자가 아직 문서에 남아 있어
/// 합성 재개(채택)가 그것을 도로 주워 올 수 있다. 문맥이 빈 하네스는 그 길을 열지 못한다.
fn run(events: &str) -> (String, Option<String>) {
    let mut composer = NaratgeulComposer::new();
    let mut committed = String::new();
    let mut composing: Option<String> = None;
    for character in events.chars() {
        let event = match character {
            '+' => ComposerEvent::Key(STROKE),
            '*' => ComposerEvent::Key(TENSE),
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

/// 획을 더해 자음을 만든다 — 자음 키가 내는 것은 밑글자 여섯뿐이고, 획은 짝이 아니라
/// 차례다(ㄴ에 획을 더하면 ㄷ이고 한 번 더하면 ㅌ이다).
#[test]
fn adds_strokes_to_build_consonants() {
    assert_text("ㄱ+ㅏ", "카");
    assert_text("ㄴ+ㅏ", "다");
    assert_text("ㄴ++ㅏ", "타");
    assert_text("ㅁ+ㅏ", "바");
    assert_text("ㅁ++ㅏ", "파");
    assert_text("ㅅ+ㅏ", "자");
    assert_text("ㅅ++ㅏ", "차");
    assert_text("ㅇ+ㅏ", "하");
    // ㄹ에는 더할 획이 없다 — 아무 일도 일어나지 않는다
    assert_text("ㄹ+ㅏ", "라");
}

/// 된소리는 획과 다른 키이고, 획을 더해 만든 자음에도 걸린다(ㄴ→ㄷ→ㄸ).
#[test]
fn adds_tension_to_build_double_consonants() {
    assert_text("ㄱ*ㅏ", "까");
    assert_text("ㄴ+*ㅏ", "따");
    assert_text("ㅁ+*ㅏ", "빠");
    assert_text("ㅅ*ㅏ", "싸");
    assert_text("ㅅ+*ㅏ", "짜");
    // ㅋ에 된소리는 없다 — 아무 일도 일어나지 않고 그다음 모음이 그대로 붙는다
    assert_text("ㄱ+*ㅏ", "카");
}

/// 고리를 한 바퀴 돌면 밑글자로 되돌아온다 — 잘못 더한 획을 같은 키로 물리는 길이다.
#[test]
fn a_full_turn_of_the_ring_returns_to_the_base() {
    assert_text("ㄱ++ㅏ", "가");
    assert_text("ㄴ+++ㅏ", "나");
    assert_text("ㅅ+++ㅏ", "사");
    assert_text("ㄱ**ㅏ", "가");
    assert_text("ㅏ++", "ㅏ");
}

/// 종성도 같은 규칙으로 만들어진다 — 갈아 끼우기가 자리를 가리지 않는다.
#[test]
fn builds_a_final_consonant_with_the_same_keys() {
    assert_text("ㄱㅏㄱ+", "갘");
    assert_text("ㅇㅏㄱ*", "앆");
    assert_text("ㅇㅏㄴ+", "앋");
}

/// 모음에 획을 더하면 이중모음이 된다.
#[test]
fn adds_a_stroke_to_build_vowels() {
    assert_text("ㅇㅏ+", "야");
    assert_text("ㅇㅓ+", "여");
    assert_text("ㅇㅗ+", "요");
    assert_text("ㅇㅜ+", "유");
    // ㅡ·ㅣ에는 더할 획이 없다
    assert_text("ㅇㅡ+", "으");
    assert_text("ㅇㅣ+", "이");
}

/// 모음을 이어 치면 합쳐진다 — 두벌식이라면 ㅏ 다음 ㅣ는 새 글자다.
#[test]
fn joins_consecutive_vowels() {
    assert_text("ㅇㅏㅣ", "애");
    assert_text("ㅇㅓㅣ", "에");
    assert_text("ㅇㅏ+ㅣ", "얘");
    assert_text("ㅇㅓ+ㅣ", "예");
    assert_text("ㅇㅗㅏ", "와");
    assert_text("ㅇㅗㅏㅣ", "왜");
    assert_text("ㅇㅗㅣ", "외");
    assert_text("ㅇㅜㅓ", "워");
    assert_text("ㅇㅜㅓㅣ", "웨");
    assert_text("ㅇㅜㅣ", "위");
    assert_text("ㅇㅡㅣ", "의");
}

/// 합쳐질 수 없는 모음은 새 자모로 선다 — 초성 없이 선 모음이 자모 그대로인 것은
/// 두벌식과 같다.
#[test]
fn a_vowel_that_cannot_join_stands_on_its_own() {
    assert_text("ㅇㅏㅗ", "아ㅗ");
    // 획을 더한 모음은 더 이상 겹모음의 앞자리가 아니다(ㅗㅏ는 ㅘ지만 ㅛㅏ는 아니다)
    assert_text("ㅇㅗ+ㅏ", "요ㅏ");
}

/// 표식도 누름 하나이므로 무르는 단위도 누름 하나다 — 획을 더하기 전으로 돌아간다.
#[test]
fn backspace_undoes_one_press() {
    assert_text("ㄱ+<ㅏ", "가");
    assert_text("ㄴ++<ㅏ", "다");
    assert_text("ㄱ*<ㅏ", "가");
    assert_text("ㅇㅏㅣ<", "아");
    assert_text("ㅇㅏ+<", "아");
    assert_text("ㅇㅗㅏㅣ<", "와");
    // 밑글자까지 물리면 그 자모가 통째로 사라진다
    assert_text("ㅇㅏ<", "ㅇ");
}

/// 밑글자 없이 눌린 표식은 아무 일도 하지 않는다.
#[test]
fn a_marker_without_a_base_does_nothing() {
    assert_text("+", "");
    assert_text("*+", "");
    // 어절이 끊긴 뒤의 표식도 마찬가지다(경계가 붙이는 공백은 Engine의 몫이라 여기 없다)
    assert_text("ㅇㅏ_+", "아");
}

/// 낱말 하나를 통째로 — 획·된소리·이중모음이 한 어절에서 만난다.
#[test]
fn types_whole_words() {
    assert_text("ㅇ+ㅏㄱㄱㅗ+", "학교");
    assert_text("ㅇㅏㄴㄴㅓ+ㅇ", "안녕");
    assert_text("ㄱ*ㅗㅅ++", "꽃");
    assert_text("ㄱ+ㅓㅣㅇㅣㄱ+ㅡ", "케이크");
}

/// 조회 키는 배열과 무관하다 — 나랏글로 친 어절도 두벌식 타건 순서로 사전을 찾는다.
#[test]
fn the_lookup_key_stays_dubeolsik() {
    let mut composer = NaratgeulComposer::new();
    let context = EditorContext::unavailable();
    let mut request = Default::default();
    // ㅇ에 획을 더하면 ㅎ이다 → "학"의 조회 키는 두벌식 타건 그대로 g(ㅎ) k(ㅏ) r(ㄱ)
    for press in ['ㅇ', STROKE, 'ㅏ', 'ㄱ'] {
        request = composer
            .feed(
                ComposerEvent::Key(press),
                &ComposerEnvironment::new(&context),
            )
            .suggest;
    }
    assert_eq!(
        request,
        taza_engine::contract::SuggestionRequest::Word {
            key: "gkr".to_string()
        }
    );
}

/// 배열을 고르면 합성기가 함께 갈린다 — 나랏글 판의 획 추가 키가 실제로 자음을 갈아
/// 끼우고, 그 결과가 조합 창에 선다.
#[test]
fn selecting_the_layout_switches_the_composer() {
    let mut engine = Engine::new(LanguageDescriptor::builtin("ko").unwrap());
    assert!(engine.select_layout("나랏글"));

    let context = EditorContext::unavailable();
    let key_at = |engine: &Engine, label: &str| {
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
    };

    let (x, y) = key_at(&engine, "ㄱ");
    engine.press_at(x, y, &context);
    let (x, y) = key_at(&engine, "획추가");
    let result = engine.press_at(x, y, &context);

    // 획 추가는 자기 글자를 내지 않는다 — 방금 넣은 ㄱ을 지우고 ㅋ으로 갈아 끼운다
    assert!(
        result.effects.iter().any(
            |effect| matches!(effect, Effect::SetComposing(composing) if composing.text == "ㅋ")
        ),
        "{:?}",
        result.effects
    );
}

/// 표식은 조합 중이 아닐 때도 문서에 새어 나가지 않는다 — 사설 영역의 글자가 그대로
/// 들어가면 앱에는 두부 상자가 남는다.
#[test]
fn the_marker_never_reaches_the_document() {
    let mut engine = Engine::new(LanguageDescriptor::builtin("ko").unwrap());
    assert!(engine.select_layout("나랏글"));
    let effects = engine.handle(
        InputEvent::Key(KeySignal::certain(STROKE)),
        &EditorContext::unavailable(),
    );
    assert!(
        !effects
            .iter()
            .any(|effect| matches!(effect, Effect::CommitText(text) if text.contains(STROKE))),
        "{effects:?}"
    );
}
