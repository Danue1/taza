use taza_engine::contract::{
    EditorContext, Effect, FieldKind, FieldTraits, InputEvent, UserPreferences,
};
use taza_engine::engine::Engine;
use taza_engine::keyboard::{
    FormFactor, KeyLegend, KeyRole, KeySignal, Keyboard, KeyboardFrame, KeyboardMetrics,
    ShellRequest, layouts,
};
use taza_engine::lang::LanguageDescriptor;

/// 터치는 이웃 키 확률까지 담은 신호를 만든다 — 여기서는 실제로 입력된 글자만 본다.
fn pressed(keyboard: &mut Keyboard, x: f32, y: f32) -> Option<char> {
    match keyboard.press_at(x, y).event {
        Some(InputEvent::Key(signal)) => Some(signal.character()),
        _ => None,
    }
}

fn key_width(frame: &KeyboardFrame, label: &str) -> f32 {
    for row in &frame.rows {
        for key in row {
            if key.label == label {
                return key.bounds.width;
            }
        }
    }
    panic!("키 {label:?} 없음");
}

fn key_center(frame: &KeyboardFrame, label: &str) -> (f32, f32) {
    for row in &frame.rows {
        for key in row {
            if key.label == label {
                return (
                    key.bounds.x + key.bounds.width / 2.0,
                    key.bounds.y + key.bounds.height / 2.0,
                );
            }
        }
    }
    panic!("키 {label:?} 없음");
}

#[test]
fn frame_geometry_is_normalized_and_centered() {
    let keyboard = Keyboard::new(
        layouts::qwerty(),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    let frame = keyboard.frame();
    assert_eq!(frame.rows.len(), 4);

    // 첫 행(10키 × 0.1)은 전체 폭, 둘째 행(9키)은 가운데 정렬
    let first_row_first = &frame.rows[0][0];
    assert!(first_row_first.bounds.x.abs() < 1e-6);
    let second_row_first = &frame.rows[1][0];
    assert!((second_row_first.bounds.x - 0.05).abs() < 1e-6);

    for (row_index, row) in frame.rows.iter().enumerate() {
        for key in row {
            assert!((key.bounds.y - row_index as f32 * 0.25).abs() < 1e-6);
            assert!(key.bounds.x >= 0.0 && key.bounds.x + key.bounds.width <= 1.0 + 1e-6);
        }
    }
}

#[test]
fn hit_test_maps_coordinates_to_keys() {
    let mut keyboard = Keyboard::new(
        layouts::qwerty(),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    let frame = keyboard.frame();

    let (x, y) = key_center(&frame, "q");
    assert_eq!(pressed(&mut keyboard, x, y), Some('q'));

    let (x, y) = key_center(&frame, "English");
    assert_eq!(
        keyboard.press_at(x, y).event,
        Some(InputEvent::Separator(' '))
    );

    let (x, y) = key_center(&frame, "⌫");
    assert_eq!(keyboard.press_at(x, y).event, Some(InputEvent::Backspace));
}

#[test]
fn coordinates_outside_snap_to_nearest_key() {
    let mut keyboard = Keyboard::new(
        layouts::qwerty(),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    // 화면 왼쪽 위 바깥 → 첫 행 첫 키
    assert_eq!(pressed(&mut keyboard, -0.1, -0.5), Some('q'));
    // 둘째 행 왼쪽 여백(가운데 정렬로 생긴 빈 공간) → 'a'
    assert_eq!(pressed(&mut keyboard, 0.01, 0.3), Some('a'));
}

#[test]
fn shift_is_one_shot() {
    let mut keyboard = Keyboard::new(
        layouts::qwerty(),
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
        layouts::qwerty(),
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
fn control_keys_carry_a_role_the_shell_can_name() {
    let keyboard = Keyboard::new(
        layouts::qwerty(),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    let frame = keyboard.frame();
    // 접근성 문구는 셸이 화면 언어로 짓는다 — 코어는 어느 키인지(역할)만 밝힌다
    let roles: Vec<KeyRole> = frame.rows.iter().flatten().map(|key| key.role).collect();
    assert!(roles.contains(&KeyRole::Shift));
    assert!(roles.contains(&KeyRole::Backspace));
    assert!(roles.contains(&KeyRole::Space));
    assert!(roles.contains(&KeyRole::Enter));
}

#[test]
fn dubeolsik_shift_produces_tense_consonants() {
    let mut keyboard = Keyboard::new(
        layouts::dubeolsik(),
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
        layouts::qwerty(),
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
fn korean_layer_switch_label_is_hangul() {
    let mut keyboard = Keyboard::new(
        layouts::dubeolsik(),
        LanguageDescriptor::builtin("ko").unwrap(),
    );
    let frame = keyboard.frame();
    let (x, y) = key_center(&frame, "123");
    keyboard.press_at(x, y);
    // 심볼면에서 문자면 복귀 키는 "한글"
    key_center(&keyboard.frame(), "한글");
}

#[test]
fn layout_from_pack_roundtrip_drives_keyboard() {
    use taza_engine::pack::{Pack, SectionKind};
    use taza_toolchain::PackWriter;
    // 높이가 다른 행도 팩 데이터로 실려 간다 — 폼팩터별 배열 확장의 통로
    let mut source = layouts::dubeolsik();
    source.layers[0].rows[0].height_ratio = 0.8;
    let named = vec![taza_engine::pack::layout::NamedLayoutSet {
        skeleton: None,
        name: "두벌식".to_string(),
        layouts: source.clone(),
    }];
    let mut writer = PackWriter::new("ko");
    writer.add_section(
        SectionKind::Layout,
        taza_toolchain::layout::serialize(&named),
    );
    let bytes = writer.finish();

    let loaded = Pack::open(&bytes).unwrap().layouts().unwrap();
    assert_eq!(loaded, named);

    let mut keyboard = Keyboard::new(
        loaded[0].layouts.clone(),
        LanguageDescriptor::builtin("ko").unwrap(),
    );
    let frame = keyboard.frame();
    let (x, y) = key_center(&frame, "ㄱ");
    assert_eq!(pressed(&mut keyboard, x, y), Some('ㄱ'));
}

#[test]
fn bottom_row_order_is_symbols_emoji_language_space_enter() {
    let keyboard = Keyboard::new(
        layouts::qwerty(),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    let frame = keyboard.frame();
    let bottom = frame.rows.last().unwrap();
    let roles: Vec<KeyRole> = bottom.iter().map(|key| key.role).collect();
    assert_eq!(
        roles,
        vec![
            KeyRole::LayerSwitch,
            KeyRole::LayerSwitch,
            KeyRole::LanguageSwitch,
            KeyRole::Space,
            KeyRole::Enter
        ]
    );
    // 심볼 다음이 통합 검색면 진입 — 순정 이모지 키와 같은 웃는 얼굴
    assert_eq!(bottom[0].label, "123");
    assert_eq!(bottom[1].label, "☺");
    assert_eq!(bottom[1].role, KeyRole::LayerSwitch);
    // 스페이스바는 순정 관례대로 현재 언어를 표기한다
    assert_eq!(bottom[3].label, "English");
    assert_eq!(bottom[2].label, "A");
    assert_eq!(bottom[2].role, KeyRole::LanguageSwitch);
}

/// 통합 검색면은 키 대신 패널이 자리를 갖는다 — 하단 행만 키로 남고, 키보드 전체 높이는
/// 문자면과 같아야 레이어를 넘나들 때 키보드가 커지거나 작아지지 않는다.
#[test]
fn the_annotation_panel_layer_keeps_the_keyboard_height() {
    let mut keyboard = Keyboard::new(
        layouts::qwerty(),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    let letters = keyboard.frame();
    assert_eq!(letters.panel_height_ratio, 0.0);

    let bottom = letters.rows.last().unwrap();
    let emoji_key = &bottom[1];
    let (x, y) = (emoji_key.bounds.x + 0.01, emoji_key.bounds.y + 0.01);
    assert!(keyboard.press_at(x, y).layout_changed);

    let panel = keyboard.frame();
    assert_eq!(panel.metrics.grid_height, letters.metrics.grid_height);
    // 패널이 네 행 가운데 셋을 차지하고 하단 행만 키로 남는다
    assert!((panel.panel_height_ratio - 0.75).abs() < 0.001);
    assert_eq!(panel.rows.len(), 1);
    let roles: Vec<KeyRole> = panel.rows[0].iter().map(|key| key.role).collect();
    // 고르는 일만 하는 면이라 낱말을 치는 키(스페이스·엔터)는 두지 않는다
    assert_eq!(
        roles,
        vec![KeyRole::LayerSwitch, KeyRole::Blank, KeyRole::Backspace]
    );
    // 문자면 복귀 키는 순정 관례대로 스크립트에 맞는 라벨을 쓴다
    assert_eq!(panel.rows[0][0].label, "ABC");
    // 키 행은 패널 아래에서 시작한다
    assert!((panel.rows[0][0].bounds.y - 0.75).abs() < 0.001);
}

#[test]
fn language_key_asks_shell_to_switch() {
    let mut keyboard = Keyboard::new(
        layouts::dubeolsik(),
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
        layouts::qwerty(),
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
        keyboard.select_alternate("é"),
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
        layouts::qwerty(),
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

/// 이웃 후보는 조회 키가 될 수 있는 같은 갈래여야 한다. 숫자 행을 켜면 숫자 키가 글자
/// 바로 위에 서는데, 사전에 숫자 표제어가 없으므로 그것이 후보 자리와 확률 몫을
/// 가져가면 진짜 이웃 글자의 몫만 깎인다.
#[test]
fn the_number_row_does_not_take_probability_from_letters() {
    let signal_for = |number_row: bool| {
        let mut keyboard = Keyboard::new(
            layouts::qwerty(),
            LanguageDescriptor::builtin("en").unwrap(),
        );
        keyboard.set_preferences(UserPreferences {
            number_row,
            ..UserPreferences::default()
        });
        let frame = keyboard.frame();
        let key = frame
            .rows
            .iter()
            .flatten()
            .find(|key| key.label == "e")
            .unwrap();
        // 숫자 행과 맞닿은 위쪽 가장자리 — 숫자가 끼어든다면 여기서 끼어든다
        let (x, y) = (
            key.bounds.x + key.bounds.width / 2.0,
            key.bounds.y + key.bounds.height * 0.15,
        );
        match keyboard.press_at(x, y).event {
            Some(InputEvent::Key(signal)) => signal,
            other => panic!("글자 키가 아님: {other:?}"),
        }
    };

    let without = signal_for(false);
    let with = signal_for(true);
    assert_eq!(with.character(), 'e');
    assert!(
        with.candidates()
            .iter()
            .all(|key| key.character.is_alphabetic()),
        "숫자가 이웃 후보에 들어옴: {:?}",
        with.candidates()
    );
    // 숫자 행이 있으나 없으나 글자끼리의 확률은 같다
    for candidate in without.candidates() {
        assert!(
            (with.probability_of(candidate.character) - candidate.probability).abs() < 1e-5,
            "{}의 확률이 숫자 행 때문에 달라짐",
            candidate.character
        );
    }
}

#[test]
fn hangul_letters_offer_their_shifted_pair_as_alternate() {
    let keyboard = Keyboard::new(
        layouts::dubeolsik(),
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
fn shift_lock_survives_typing_and_only_applies_to_cased_layouts() {
    let mut keyboard = Keyboard::new(
        layouts::qwerty(),
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
        layouts::dubeolsik(),
        LanguageDescriptor::builtin("ko").unwrap(),
    );
    assert!(!hangul.toggle_shift_lock());
    assert_eq!(hangul.frame().rows[0][0].label, "ㅂ");
}

#[test]
fn cursor_drag_emits_steps_once_per_threshold() {
    let mut keyboard = Keyboard::new(
        layouts::qwerty(),
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
fn touch_sequence_types_hangul_through_session() {
    let mut keyboard = Keyboard::new(
        layouts::dubeolsik(),
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
fn form_factor_drives_measured_sizes() {
    let mut keyboard = Keyboard::new(
        layouts::qwerty(),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    let portrait = keyboard.frame().metrics;
    assert!(
        (portrait.total_height() - (portrait.grid_height + portrait.candidate_bar_height)).abs()
            < 1e-6
    );

    keyboard.set_metrics(KeyboardMetrics {
        form_factor: FormFactor::PhoneLandscape,
        width_points: 844.0,
        text_scale: 1.0,
    });
    let landscape = keyboard.frame();
    // 가로에서는 행 높이만 줄고 배열은 그대로다 (순정 관례)
    assert!(landscape.metrics.grid_height < portrait.grid_height);
    assert_eq!(landscape.rows.len(), 4);
    assert_eq!(landscape.rows[0].len(), 10);

    keyboard.set_metrics(KeyboardMetrics {
        form_factor: FormFactor::Tablet,
        width_points: 1024.0,
        text_scale: 1.0,
    });
    let tablet = keyboard.frame().metrics;
    assert!(tablet.grid_height > portrait.grid_height);
    assert!(tablet.letter_font_size > portrait.letter_font_size);
}

#[test]
fn row_height_comes_from_layout_data() {
    let mut layout_set = layouts::qwerty();
    // 낮은 숫자행을 얹는 경우 — 배치도 히트 테스트도 이 값을 따라야 한다
    layout_set.layers[0].rows[0].height_ratio = 0.5;
    let mut keyboard = Keyboard::new(layout_set, LanguageDescriptor::builtin("en").unwrap());
    let frame = keyboard.frame();

    let total = 0.5 + 3.0;
    assert!((frame.rows[0][0].bounds.height - 0.5 / total).abs() < 1e-6);
    assert!((frame.rows[1][0].bounds.y - 0.5 / total).abs() < 1e-6);
    // 그리드 높이는 행 수가 아니라 높이 합을 따른다
    assert!((frame.metrics.grid_height - 54.0 * total).abs() < 1e-3);

    let (x, y) = key_center(&frame, "a");
    assert_eq!(pressed(&mut keyboard, x, y), Some('a'));
    // 낮아진 첫 행의 아래쪽 경계 바로 밑은 이미 둘째 행이다
    assert_eq!(pressed(&mut keyboard, 0.05, 0.5 / total + 1e-3), Some('a'));
}

#[test]
fn cursor_drag_sensitivity_is_physical() {
    let mut narrow = Keyboard::new(
        layouts::qwerty(),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    narrow.set_metrics(KeyboardMetrics {
        form_factor: FormFactor::PhonePortrait,
        width_points: 400.0,
        text_scale: 1.0,
    });
    let mut wide = Keyboard::new(
        layouts::qwerty(),
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

#[test]
fn symbol_rows_span_the_full_width() {
    let mut keyboard = Keyboard::new(
        layouts::qwerty(),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    let (x, y) = key_center(&keyboard.frame(), "123");
    keyboard.press_at(x, y);

    // 순정처럼 #+= 는 왼쪽 끝, ⌫ 는 오른쪽 끝에 붙는다
    for row in &keyboard.frame().rows {
        let first = row.first().unwrap();
        let last = row.last().unwrap();
        assert!(first.bounds.x.abs() < 1e-6);
        assert!((last.bounds.x + last.bounds.width - 1.0).abs() < 1e-6);
    }
}

/// 필드 성격이 화면을 바꾸는 규칙 — 실측 근거는 docs/inputmode.md.
#[test]
fn number_fields_open_a_number_pad() {
    let mut keyboard = Keyboard::new(
        layouts::qwerty(),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    keyboard.set_field(FieldTraits::of(FieldKind::Number));
    let frame = keyboard.frame();

    // 순정처럼 3열 4행이고, 좌하단은 눌리지 않는 빈 자리다
    assert_eq!(frame.rows.len(), 4);
    assert!(frame.rows.iter().all(|row| row.len() == 3));
    assert_eq!(frame.rows[3][0].role, KeyRole::Blank);
    assert_eq!(frame.rows[3][1].label, "0");
    assert_eq!(frame.rows[3][2].role, KeyRole::Backspace);
    // 예측을 내지 않는 필드에서는 후보 바 자리가 사라져 키보드가 낮아진다
    assert_eq!(frame.metrics.candidate_bar_height, 0.0);

    let (x, y) = key_center(&frame, "");
    assert_eq!(keyboard.press_at(x, y).event, None);
}

#[test]
fn decimal_and_phone_fields_differ_only_in_the_corner_key() {
    let mut keyboard = Keyboard::new(
        layouts::qwerty(),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    keyboard.set_field(FieldTraits::of(FieldKind::Decimal));
    assert_eq!(keyboard.frame().rows[3][0].label, ".");

    keyboard.set_field(FieldTraits::of(FieldKind::Phone));
    let frame = keyboard.frame();
    assert_eq!(frame.rows[3][0].label, "+*#");
    assert_eq!(frame.rows[0][1].label, "2");
}

#[test]
fn email_field_puts_at_and_dot_beside_a_shorter_space() {
    let mut keyboard = Keyboard::new(
        layouts::qwerty(),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    let plain_space = key_width(&keyboard.frame(), "English");
    keyboard.set_field(FieldTraits::of(FieldKind::Email));
    let frame = keyboard.frame();

    key_center(&frame, "@");
    key_center(&frame, ".");
    assert!(key_width(&frame, "English") < plain_space);
    assert_eq!(frame.metrics.candidate_bar_height, 0.0);
    // 스페이스가 줄어든 만큼만 나눠 가졌으므로 행 폭은 그대로다
    let bottom: f32 = frame.rows[3].iter().map(|key| key.bounds.width).sum();
    assert!((bottom - 1.0).abs() < 1e-6);
}

#[test]
fn url_field_replaces_space_with_dot_slash_and_domain() {
    let mut keyboard = Keyboard::new(
        layouts::qwerty(),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    keyboard.set_field(FieldTraits::of(FieldKind::Url));
    let frame = keyboard.frame();
    assert!(frame.rows[3].iter().all(|key| key.role != KeyRole::Space));

    // `.com`은 한 번에 여러 글자를 넣는 키다
    let (x, y) = key_center(&frame, ".com");
    assert_eq!(
        keyboard.press_at(x, y).event,
        Some(InputEvent::Text(".com".to_string()))
    );
}

#[test]
fn search_field_only_changes_the_return_key() {
    let mut keyboard = Keyboard::new(
        layouts::qwerty(),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    let plain = keyboard.frame();
    keyboard.set_field(FieldTraits::of(FieldKind::Search));
    let frame = keyboard.frame();

    assert_eq!(frame.rows[0].len(), plain.rows[0].len());
    let enter = frame.rows[3].last().unwrap();
    // 리턴키가 무슨 낱말로 적힐지는 갈래로만 알린다 — 낱말 자체는 셸의 몫이다
    assert_eq!(enter.legend, Some(KeyLegend::Search));
    assert!(enter.emphasized);
    // 검색어야말로 예측이 쓸모 있는 자리라 후보 바는 그대로 둔다
    assert!(frame.metrics.candidate_bar_height > 0.0);
}

#[test]
fn password_field_strips_emoji_and_language_keys() {
    let mut keyboard = Keyboard::new(
        layouts::qwerty(),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    keyboard.set_field(FieldTraits::of(FieldKind::Password));
    let frame = keyboard.frame();

    let bottom = &frame.rows[3];
    assert!(bottom.iter().all(|key| key.role != KeyRole::LanguageSwitch));
    assert!(bottom.iter().all(|key| key.label != "☺"));
    assert_eq!(bottom[0].label, ".?123");
    let width: f32 = bottom.iter().map(|key| key.bounds.width).sum();
    assert!((width - 1.0).abs() < 1e-6);
    // 순정은 이 자리를 암호 관리자에 내주므로 바를 없애지 않는다
    assert!(frame.metrics.candidate_bar_height > 0.0);
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

#[test]
fn pack_can_carry_several_layouts_and_the_engine_switches_between_them() {
    use std::sync::Arc;
    use taza_engine::engine::PackBytes;
    use taza_engine::pack::SectionKind;
    use taza_toolchain::PackWriter;
    use taza_toolchain::metadata::MetadataBuilder;

    let text = "\
=== QWERTY
q w e
layer1*0.15 space*0.55 enter*0.3
---
1 2 3
layer0*0.15 space*0.55 enter*0.3
=== Dvorak
p y f
layer1*0.15 space*0.55 enter*0.3
";
    let mut metadata = MetadataBuilder::new();
    metadata.set("display_name", "English");
    metadata.set("keycap_label", "A");
    metadata.set("layout_name", "QWERTY");
    metadata.set("composer_skeleton", "latin");
    metadata.set("lexicon_encoding", "utf8");

    let mut writer = PackWriter::new("en");
    writer.add_section(
        SectionKind::Layout,
        taza_toolchain::layout::serialize(&taza_toolchain::layout::parse(text).unwrap()),
    );
    writer.add_section(SectionKind::Metadata, metadata.build());
    let bytes = writer.finish();

    let mut engine = Engine::new(LanguageDescriptor::builtin("en").unwrap()).unwrap();
    engine
        .load_pack(Arc::new(bytes) as Arc<dyn PackBytes>)
        .unwrap();
    assert_eq!(engine.available_layouts(), vec!["QWERTY", "Dvorak"]);
    assert_eq!(engine.layout_name(), "QWERTY");
    key_center(&engine.frame(), "q");

    assert!(engine.select_layout("Dvorak"));
    assert_eq!(engine.layout_name(), "Dvorak");
    key_center(&engine.frame(), "p");
    // 심볼면은 첫 배열에서 물려받는다 — 배열마다 다시 싣지 않는다
    let frame = engine.frame();
    let (x, y) = key_center(&frame, "123");
    engine.press_at(x, y, &EditorContext::unavailable());
    key_center(&engine.frame(), "1");

    // 없는 이름은 조용히 무시된다 — 팩 갱신으로 사라진 배열이 설정에 남아 있을 수 있다
    assert!(!engine.select_layout("Colemak"));
    assert_eq!(engine.layout_name(), "Dvorak");
}

/// 세벌식은 새 합성기가 아니라 배열 데이터다 — 키가 자리를 밝힌 자모(초성 U+1100 등)를
/// 멀티탭은 주기를 한 바퀴 돌아 첫 글자로 되돌아와도 여전히 **갈아 끼우기**다. 그것을
/// 새 입력으로 내면 넷째 누름에서 글자가 하나 더 붙는다(ㄱ→ㅋ→ㄲ→ㄲㄱ). 주기가 끊긴
/// 뒤에야 새 입력이다.
#[test]
fn multitap_replaces_even_when_the_cycle_wraps() {
    use taza_engine::pack::layout::{
        KeyAction, KeyboardLayout, KeyboardLayoutSet, LayoutKey, LayoutRow,
    };

    let multitap = |cycle: &str| LayoutKey {
        action: KeyAction::Multitap(cycle.chars().collect()),
        width_ratio: 0.5,
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

/// 내고, 키캡에는 사람이 읽는 호환 자모가 찍힌다.
#[test]
fn sebeolsik_keys_carry_their_place_and_show_compatibility_jamo() {
    use std::sync::Arc;
    use taza_engine::engine::PackBytes;
    use taza_engine::pack::SectionKind;
    use taza_toolchain::PackWriter;

    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/korean-layout.txt"
    ))
    .unwrap();
    let mut writer = PackWriter::new("ko");
    writer.add_section(
        SectionKind::Layout,
        taza_toolchain::layout::serialize(&taza_toolchain::layout::parse(&text).unwrap()),
    );
    let bytes = writer.finish();

    let mut engine = Engine::new(LanguageDescriptor::builtin("ko").unwrap()).unwrap();
    engine
        .load_pack(Arc::new(bytes) as Arc<dyn PackBytes>)
        .unwrap();
    assert_eq!(
        engine.available_layouts(),
        vec!["두벌식", "세벌식 최종", "천지인"]
    );
    assert!(engine.select_layout("세벌식 최종"));

    let frame = engine.frame();
    let labels: Vec<&str> = frame.rows[2].iter().map(|key| key.label.as_str()).collect();
    assert_eq!(
        labels,
        ["ㅇ", "ㄴ", "ㅣ", "ㅏ", "ㅡ", "ㄴ", "ㅇ", "ㄱ", "ㅈ", "ㅂ"]
    );

    // 초성 ㄱ · 중성 ㅏ · 종성 ㄱ — 자리가 자모에 실려 있으므로 도깨비불이 없다
    let context = EditorContext::unavailable();
    let mut composing = None;
    for (row, index) in [(2, 7), (2, 3), (3, 2)] {
        let bounds = engine.frame().rows[row][index].bounds;
        let result = engine.press_at(
            bounds.x + bounds.width / 2.0,
            bounds.y + bounds.height / 2.0,
            &context,
        );
        for effect in result.effects {
            if let Effect::SetComposing(text) = effect {
                composing = Some(text.text);
            }
        }
    }
    assert_eq!(composing.as_deref(), Some("각"));
}
