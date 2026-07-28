//! 판정 — 누름이 상태를 어떻게 움직이는가. shift 일회성·고정, 레이어 전환,
//! 멀티탭 주기, 커서 드래그, 길게 눌러 고르는 변형 문자.

use crate::support::*;

#[test]
fn shift_is_one_shot() {
    let mut keyboard = Keyboard::new(
        layouts::default_for(ComposerSkeleton::Latin),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    let frame = keyboard.frame();
    let (shift_x, shift_y) = key_center(&frame, "⇧");
    let (q_x, q_y) = key_center(&frame, "q");

    let outcome = keyboard.press_at(shift_x, shift_y);
    assert_eq!(outcome.event, None);
    assert!(outcome.layout_changed);
    assert_eq!(key_center(&keyboard.frame(), "Q"), (q_x, q_y));

    let outcome = keyboard.press_at(q_x, q_y);
    assert_eq!(
        outcome.event,
        Some(InputEvent::Key(KeySignal::certain('Q')))
    );
    assert!(outcome.layout_changed);

    // 자동 해제 — 다음 입력은 소문자
    assert_eq!(pressed(&mut keyboard, q_x, q_y), Some('q'));
}

#[test]
fn shift_toggles_off_when_pressed_twice() {
    let mut keyboard = Keyboard::new(
        layouts::default_for(ComposerSkeleton::Latin),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    let frame = keyboard.frame();
    let (shift_x, shift_y) = key_center(&frame, "⇧");
    keyboard.press_at(shift_x, shift_y);
    keyboard.press_at(shift_x, shift_y);
    let (q_x, q_y) = key_center(&frame, "q");
    assert_eq!(pressed(&mut keyboard, q_x, q_y), Some('q'));
}

#[test]
fn shift_lock_survives_typing_and_only_applies_to_cased_layouts() {
    let mut keyboard = Keyboard::new(
        layouts::default_for(ComposerSkeleton::Latin),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    assert!(keyboard.toggle_shift_lock());
    let frame = keyboard.frame();
    assert_eq!(frame.rows[0][0].label, "Q");
    let (x, y) = key_center(&frame, "Q");
    keyboard.press_at(x, y);
    // 고정된 shift는 글자를 넣어도 풀리지 않는다
    assert_eq!(keyboard.frame().rows[0][0].label, "Q");
    assert!(keyboard.toggle_shift_lock());
    assert_eq!(keyboard.frame().rows[0][0].label, "q");

    // 한글은 shift가 대문자가 아니라 다른 자모라 고정할 것이 없다
    let mut hangul = Keyboard::new(
        layouts::default_for(ComposerSkeleton::Hangul),
        LanguageDescriptor::builtin("ko").unwrap(),
    );
    assert!(!hangul.toggle_shift_lock());
    assert_eq!(hangul.frame().rows[0][0].label, "ㅂ");
}

#[test]
fn dubeolsik_shift_produces_tense_consonants() {
    let mut keyboard = Keyboard::new(
        layouts::default_for(ComposerSkeleton::Hangul),
        LanguageDescriptor::builtin("ko").unwrap(),
    );
    let frame = keyboard.frame();
    let (shift_x, shift_y) = key_center(&frame, "⇧");
    let (giyeok_x, giyeok_y) = key_center(&frame, "ㄱ");

    keyboard.press_at(shift_x, shift_y);
    assert_eq!(pressed(&mut keyboard, giyeok_x, giyeok_y), Some('ㄲ'));
}

#[test]
fn layer_switch_cycles_symbol_layers() {
    let mut keyboard = Keyboard::new(
        layouts::default_for(ComposerSkeleton::Latin),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    let frame = keyboard.frame();

    // 문자면 하단의 123 → 심볼 1면
    let (x, y) = key_center(&frame, "123");
    let outcome = keyboard.press_at(x, y);
    assert_eq!(outcome.event, None);
    assert!(outcome.layout_changed);
    let symbols = keyboard.frame();
    let (one_x, one_y) = key_center(&symbols, "1");
    assert_eq!(pressed(&mut keyboard, one_x, one_y), Some('1'));

    // 심볼 1면의 #+= → 심볼 2면, ABC → 문자면 복귀
    let (x, y) = key_center(&keyboard.frame(), "#+=");
    keyboard.press_at(x, y);
    let (bracket_x, bracket_y) = key_center(&keyboard.frame(), "[");
    assert_eq!(pressed(&mut keyboard, bracket_x, bracket_y), Some('['));
    let (x, y) = key_center(&keyboard.frame(), "ABC");
    keyboard.press_at(x, y);
    let (q_x, q_y) = key_center(&keyboard.frame(), "q");
    assert_eq!(pressed(&mut keyboard, q_x, q_y), Some('q'));
}

#[test]
fn language_key_asks_shell_to_switch() {
    let mut keyboard = Keyboard::new(
        layouts::default_for(ComposerSkeleton::Hangul),
        LanguageDescriptor::builtin("ko").unwrap(),
    );
    let frame = keyboard.frame();
    let (x, y) = key_center(&frame, "한");
    let outcome = keyboard.press_at(x, y);
    assert_eq!(outcome.event, None);
    assert!(!outcome.layout_changed);
    assert_eq!(outcome.request, Some(ShellRequest::NextLanguage));
}

#[test]
fn alternates_reach_the_shell_and_come_back_as_input() {
    let mut keyboard = Keyboard::new(
        layouts::default_for(ComposerSkeleton::Latin),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    let frame = keyboard.frame();
    let (x, y) = key_center(&frame, "e");
    let key = keyboard.key_at(x, y);
    assert_eq!(key.role, KeyRole::Character);
    // 누르고 있는 글자가 맨 앞, 배열이 밝힌 변형이 뒤따른다
    assert_eq!(key.alternates.first().map(String::as_str), Some("e"));
    assert_eq!(key.alternates.get(1).map(String::as_str), Some("è"));

    assert_eq!(
        keyboard.select_alternate("é").event,
        Some(InputEvent::Key(KeySignal::certain('é')))
    );
    // 변형이 없는 키는 빈 목록 — 셸은 롱프레스 팝업을 띄우지 않는다
    let (x, y) = key_center(&frame, "g");
    assert!(keyboard.key_at(x, y).alternates.is_empty());
}

/// shift가 올라가 있으면 변형도 대문자로 나온다. 그러지 않으면 É·Ü·Ç에 닿을 길이
/// 아예 없어지는데, QWERTZ·AZERTY를 고르는 이유가 바로 그 글자들이다.
#[test]
fn alternates_follow_shift_into_uppercase() {
    let mut keyboard = Keyboard::new(
        layouts::default_for(ComposerSkeleton::Latin),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    assert!(keyboard.toggle_shift_lock());
    let frame = keyboard.frame();
    let (x, y) = key_center(&frame, "E");
    assert_eq!(
        keyboard.key_at(x, y).alternates,
        ["E", "È", "É", "Ê", "Ë", "Ē", "Ė", "Ę"]
    );
    // 대문자가 두 글자가 되는 글자(ß→SS)는 팝업 한 칸에 담기지 않으므로 그대로 둔다
    let (x, y) = key_center(&frame, "S");
    assert_eq!(keyboard.key_at(x, y).alternates, ["S", "ß", "Ś", "Š"]);

    assert!(keyboard.toggle_shift_lock());
    let frame = keyboard.frame();
    let (x, y) = key_center(&frame, "e");
    assert_eq!(
        keyboard.key_at(x, y).alternates,
        ["e", "è", "é", "ê", "ë", "ē", "ė", "ę"]
    );
}

#[test]
fn hangul_letters_offer_their_shifted_pair_as_alternate() {
    let keyboard = Keyboard::new(
        layouts::default_for(ComposerSkeleton::Hangul),
        LanguageDescriptor::builtin("ko").unwrap(),
    );
    let letters = &keyboard.frame().rows[0];
    // ㄱ은 길게 눌러 ㄲ에 닿고, 짝이 없는 ㅛ에는 변형이 없다
    let alternates = |label: &str| {
        letters
            .iter()
            .find(|key| key.label == label)
            .map(|key| key.alternates.clone())
            .unwrap()
    };
    assert_eq!(alternates("ㄱ"), vec!["ㄱ".to_string(), "ㄲ".to_string()]);
    assert_eq!(alternates("ㅐ"), vec!["ㅐ".to_string(), "ㅒ".to_string()]);
    assert!(alternates("ㅛ").is_empty());
}

#[test]
fn cursor_drag_emits_steps_once_per_threshold() {
    let mut keyboard = Keyboard::new(
        layouts::default_for(ComposerSkeleton::Latin),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    keyboard.begin_cursor_drag(0.5);
    // 임계 이하 이동은 0칸
    assert_eq!(keyboard.update_cursor_drag(0.51), 0);
    // 여러 칸을 한 번에 지나면 이미 낸 칸수를 뺀 나머지만 나온다
    assert_eq!(keyboard.update_cursor_drag(0.56), 4);
    assert_eq!(keyboard.update_cursor_drag(0.56), 0);
    // 되돌아오면 반대 방향
    assert_eq!(keyboard.update_cursor_drag(0.5), -4);

    keyboard.end_cursor_drag();
    assert_eq!(keyboard.update_cursor_drag(0.9), 0);
}

#[test]
fn cursor_drag_sensitivity_is_physical() {
    let mut narrow = Keyboard::new(
        layouts::default_for(ComposerSkeleton::Latin),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    narrow.set_metrics(KeyboardMetrics {
        form_factor: FormFactor::PhonePortrait,
        width_points: 400.0,
        text_scale: 1.0,
    });
    let mut wide = Keyboard::new(
        layouts::default_for(ComposerSkeleton::Latin),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    wide.set_metrics(KeyboardMetrics {
        form_factor: FormFactor::Tablet,
        width_points: 800.0,
        text_scale: 1.0,
    });

    narrow.begin_cursor_drag(0.0);
    wide.begin_cursor_drag(0.0);
    // 손가락이 같은 거리(80pt)를 지나면 화면 폭과 무관하게 같은 칸수가 나온다
    assert_eq!(narrow.update_cursor_drag(80.0 / 400.0), 16);
    assert_eq!(wide.update_cursor_drag(80.0 / 800.0), 16);
}

/// 멀티탭은 주기를 한 바퀴 돌아 첫 글자로 되돌아와도 여전히 **갈아 끼우기**다. 그것을
/// 새 입력으로 내면 넷째 누름에서 글자가 하나 더 붙는다(ㄱ→ㅋ→ㄲ→ㄲㄱ). 주기가 끊긴
/// 뒤에야 새 입력이다.
#[test]
fn multitap_replaces_even_when_the_cycle_wraps() {
    let multitap = |cycle: &str| LayoutKey {
        action: KeyAction::Multitap(cycle.chars().collect()),
        width_ratio: 0.5,
        row_span: 1,
        alternates: Vec::new(),
    };
    let layout_set = KeyboardLayoutSet {
        layers: vec![KeyboardLayout {
            panel_rows: 0.0,
            rows: vec![LayoutRow {
                keys: vec![multitap("ㄱㅋㄲ"), multitap("ㄴㄹ")],
                height_ratio: 1.0,
            }],
        }],
    };
    let mut keyboard = Keyboard::new(layout_set, LanguageDescriptor::builtin("ko").unwrap());

    let mut press = |x: f32| {
        let outcome = keyboard.press_at(x, 0.5);
        // 이어 누르는 동안에는 시한이 매번 새로 시작한다
        assert!(outcome.timer.is_some());
        match outcome.event {
            Some(InputEvent::Key(signal)) => format!("새로 {}", signal.character()),
            Some(InputEvent::Retap(character)) => format!("갈아 {character}"),
            other => panic!("글자가 나오지 않음: {other:?}"),
        }
    };

    let typed: Vec<String> = (0..4).map(|_| press(0.25)).collect();
    assert_eq!(
        typed,
        ["새로 ㄱ", "갈아 ㅋ", "갈아 ㄲ", "갈아 ㄱ"],
        "주기가 한 바퀴 돈 뒤 글자가 덧붙었다"
    );

    // 손이 다른 키로 옮겨 가면 주기가 그 자리에서 끝난다
    assert_eq!(press(0.75), "새로 ㄴ");
    assert_eq!(press(0.25), "새로 ㄱ");
}

#[test]
fn touch_sequence_types_hangul_through_session() {
    let mut keyboard = Keyboard::new(
        layouts::default_for(ComposerSkeleton::Hangul),
        LanguageDescriptor::builtin("ko").unwrap(),
    );
    let mut engine = Engine::new(LanguageDescriptor::builtin("ko").unwrap()).unwrap();
    let frame = keyboard.frame();
    let context = EditorContext::unavailable();

    let mut composing = String::new();
    for label in ["ㄱ", "ㅏ", "ㅂ", "ㅏ"] {
        let (x, y) = key_center(&frame, label);
        let event = keyboard.press_at(x, y).event.unwrap();
        for effect in engine.handle(event, &context) {
            if let Effect::SetComposing(text) = effect {
                composing = text.text;
            }
        }
    }
    assert_eq!(composing, "가바");
}

#[test]
fn text_keys_commit_after_finalizing_the_composition() {
    let mut engine = Engine::new(LanguageDescriptor::builtin("ko").unwrap()).unwrap();
    let context = EditorContext::unavailable();
    engine.set_field(FieldTraits::of(FieldKind::Url));
    let frame = engine.frame();

    // 조합 중이던 한글을 먼저 확정하고 나서 `.com`이 들어간다
    engine.handle(InputEvent::Key(KeySignal::certain('ㄷ')), &context);
    let (x, y) = key_center(&frame, ".com");
    let effects = engine.press_at(x, y, &context).effects;
    let committed: Vec<&String> = effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::CommitText(text) => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(committed.last().map(|text| text.as_str()), Some(".com"));
}

/// `→`는 커서를 오른쪽으로 한 칸 옮긴다. 옮기기 전에 조합이 확정되므로(`CursorDrag`)
/// 같은 자음으로 시작하는 글자를 잇달아 칠 때 멀티탭 시한을 기다리지 않아도 된다.
#[test]
fn the_cursor_right_key_moves_one_step() {
    let mut keyboard = Keyboard::new(
        KeyboardLayoutSet {
            layers: vec![KeyboardLayout {
                panel_rows: 0.0,
                rows: vec![LayoutRow {
                    keys: vec![LayoutKey {
                        action: KeyAction::CursorRight,
                        width_ratio: 1.0,
                        row_span: 1,
                        alternates: Vec::new(),
                    }],
                    height_ratio: 1.0,
                }],
            }],
        },
        LanguageDescriptor::builtin("ko").unwrap(),
    );
    let outcome = keyboard.press_at(0.5, 0.5);
    assert_eq!(outcome.event, Some(InputEvent::CursorDrag(1)));
    assert!(!outcome.layout_changed);
    assert_eq!(keyboard.frame().rows[0][0].role, KeyRole::CursorRight);
}

/// 조합 중에 `→`를 치면 그 글자가 먼저 확정된다 — 커서가 빠져나간 자리에 조합 중
/// 텍스트를 남기지 않는다.
#[test]
fn cursor_right_finalizes_the_composition() {
    let mut engine = Engine::new(LanguageDescriptor::builtin("ko").unwrap()).unwrap();
    let context = EditorContext::unavailable();
    engine.handle(InputEvent::Key(KeySignal::certain('ㄱ')), &context);
    let effects = engine.handle(InputEvent::CursorDrag(1), &context);
    assert!(
        effects.contains(&Effect::CommitText("ㄱ".to_string())),
        "조합을 확정하지 않음: {effects:?}"
    );
    assert_eq!(effects.last(), Some(&Effect::MoveCursor(1)));
}
