use taza_engine::contract::{EditorContext, Effect, InputEvent};
use taza_engine::engine::Engine;
use taza_engine::keyboard::{
    FormFactor, KeyRole, KeySignal, Keyboard, KeyboardFrame, KeyboardMetrics, ShellRequest, layouts,
};
use taza_engine::lang::LanguageDescriptor;

/// 터치는 이웃 키 확률까지 담은 신호를 만든다 — 여기서는 실제로 입력된 글자만 본다.
fn pressed(keyboard: &mut Keyboard, x: f32, y: f32) -> Option<char> {
    match keyboard.press_at(x, y).event {
        Some(InputEvent::Key(signal)) => Some(signal.character()),
        _ => None,
    }
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
fn accessibility_labels_name_control_keys() {
    let keyboard = Keyboard::new(
        layouts::qwerty(),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    let frame = keyboard.frame();
    let labels: Vec<&str> = frame
        .rows
        .iter()
        .flatten()
        .map(|key| key.accessibility_label.as_str())
        .collect();
    assert!(labels.contains(&"shift"));
    assert!(labels.contains(&"backspace"));
    assert!(labels.contains(&"space"));
    assert!(labels.contains(&"enter"));
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
    let mut writer = PackWriter::new("ko");
    writer.add_section(
        SectionKind::Layout,
        taza_toolchain::layout::serialize(&source),
    );
    let bytes = writer.finish();

    let loaded = Pack::open(&bytes).unwrap().layout().unwrap();
    assert_eq!(loaded, source);

    let mut keyboard = Keyboard::new(loaded, LanguageDescriptor::builtin("ko").unwrap());
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
    assert_eq!(bottom[1].accessibility_label, "emoji");
    // 스페이스바는 순정 관례대로 현재 언어를 표기한다
    assert_eq!(bottom[3].label, "English");
    assert_eq!(bottom[2].label, "A");
    assert_eq!(bottom[2].accessibility_label, "language, English");
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
    assert_eq!(
        roles,
        vec![
            KeyRole::LayerSwitch,
            KeyRole::Space,
            KeyRole::Backspace,
            KeyRole::Enter
        ]
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
    assert_eq!(key.alternates.first().map(String::as_str), Some("è"));

    assert_eq!(
        keyboard.select_alternate("é"),
        Some(InputEvent::Key(KeySignal::certain('é')))
    );
    // 변형이 없는 키는 빈 목록 — 셸은 롱프레스 팝업을 띄우지 않는다
    let (x, y) = key_center(&frame, "q");
    assert!(keyboard.key_at(x, y).alternates.is_empty());
}

#[test]
fn hangul_letters_have_no_alternates() {
    let keyboard = Keyboard::new(
        layouts::dubeolsik(),
        LanguageDescriptor::builtin("ko").unwrap(),
    );
    let letters = &keyboard.frame().rows[0];
    assert!(letters.iter().all(|key| key.alternates.is_empty()));
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
    // 오른쪽으로 두 칸 넘게 이동하면 이미 낸 칸수를 뺀 나머지만 나온다
    assert_eq!(keyboard.update_cursor_drag(0.56), 2);
    assert_eq!(keyboard.update_cursor_drag(0.56), 0);
    // 되돌아오면 반대 방향
    assert_eq!(keyboard.update_cursor_drag(0.5), -2);

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
    });
    let landscape = keyboard.frame();
    // 가로에서는 행 높이만 줄고 배열은 그대로다 (순정 관례)
    assert!(landscape.metrics.grid_height < portrait.grid_height);
    assert_eq!(landscape.rows.len(), 4);
    assert_eq!(landscape.rows[0].len(), 10);

    keyboard.set_metrics(KeyboardMetrics {
        form_factor: FormFactor::Tablet,
        width_points: 1024.0,
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
    });
    let mut wide = Keyboard::new(
        layouts::qwerty(),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    wide.set_metrics(KeyboardMetrics {
        form_factor: FormFactor::Tablet,
        width_points: 800.0,
    });

    narrow.begin_cursor_drag(0.0);
    wide.begin_cursor_drag(0.0);
    // 손가락이 같은 거리(80pt)를 지나면 화면 폭과 무관하게 같은 칸수가 나온다
    assert_eq!(narrow.update_cursor_drag(80.0 / 400.0), 8);
    assert_eq!(wide.update_cursor_drag(80.0 / 800.0), 8);
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
