use taza_core::composer::hangul::HangulComposer;
use taza_core::composer::EditorContext;
use taza_core::keyboard::{Keyboard, KeyboardFrame, layouts};
use taza_core::session::{Effect, InputEvent, Session};

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
    let keyboard = Keyboard::new(layouts::qwerty());
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
    let mut keyboard = Keyboard::new(layouts::qwerty());
    let frame = keyboard.frame();

    let (x, y) = key_center(&frame, "q");
    assert_eq!(
        keyboard.press_at(x, y).event,
        Some(InputEvent::Key('q'))
    );

    let (x, y) = key_center(&frame, "␣");
    assert_eq!(
        keyboard.press_at(x, y).event,
        Some(InputEvent::Separator(' '))
    );

    let (x, y) = key_center(&frame, "⌫");
    assert_eq!(keyboard.press_at(x, y).event, Some(InputEvent::Backspace));
}

#[test]
fn coordinates_outside_snap_to_nearest_key() {
    let mut keyboard = Keyboard::new(layouts::qwerty());
    // 화면 왼쪽 위 바깥 → 첫 행 첫 키
    assert_eq!(
        keyboard.press_at(-0.1, -0.5).event,
        Some(InputEvent::Key('q'))
    );
    // 둘째 행 왼쪽 여백(가운데 정렬로 생긴 빈 공간) → 'a'
    assert_eq!(
        keyboard.press_at(0.01, 0.3).event,
        Some(InputEvent::Key('a'))
    );
}

#[test]
fn shift_is_one_shot() {
    let mut keyboard = Keyboard::new(layouts::qwerty());
    let frame = keyboard.frame();
    let (shift_x, shift_y) = key_center(&frame, "⇧");
    let (q_x, q_y) = key_center(&frame, "q");

    let outcome = keyboard.press_at(shift_x, shift_y);
    assert_eq!(outcome.event, None);
    assert!(outcome.layout_changed);
    assert_eq!(key_center(&keyboard.frame(), "Q"), (q_x, q_y));

    let outcome = keyboard.press_at(q_x, q_y);
    assert_eq!(outcome.event, Some(InputEvent::Key('Q')));
    assert!(outcome.layout_changed);

    // 자동 해제 — 다음 입력은 소문자
    assert_eq!(
        keyboard.press_at(q_x, q_y).event,
        Some(InputEvent::Key('q'))
    );
}

#[test]
fn shift_toggles_off_when_pressed_twice() {
    let mut keyboard = Keyboard::new(layouts::qwerty());
    let frame = keyboard.frame();
    let (shift_x, shift_y) = key_center(&frame, "⇧");
    keyboard.press_at(shift_x, shift_y);
    keyboard.press_at(shift_x, shift_y);
    let (q_x, q_y) = key_center(&frame, "q");
    assert_eq!(
        keyboard.press_at(q_x, q_y).event,
        Some(InputEvent::Key('q'))
    );
}

#[test]
fn accessibility_labels_name_control_keys() {
    let keyboard = Keyboard::new(layouts::qwerty());
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
    let mut keyboard = Keyboard::new(layouts::dubeolsik());
    let frame = keyboard.frame();
    let (shift_x, shift_y) = key_center(&frame, "⇧");
    let (giyeok_x, giyeok_y) = key_center(&frame, "ㄱ");

    keyboard.press_at(shift_x, shift_y);
    assert_eq!(
        keyboard.press_at(giyeok_x, giyeok_y).event,
        Some(InputEvent::Key('ㄲ'))
    );
}

#[test]
fn layout_from_pack_roundtrip_drives_keyboard() {
    use taza_pack::{Pack, PackWriter, SectionKind, layout};
    let mut writer = PackWriter::new("ko");
    writer.add_section(
        SectionKind::Layout,
        layout::serialize(&layouts::dubeolsik()),
    );
    let bytes = writer.finish();

    let loaded = Pack::open(&bytes).unwrap().layout().unwrap();
    assert_eq!(loaded, layouts::dubeolsik());

    let mut keyboard = Keyboard::new(loaded);
    let frame = keyboard.frame();
    let (x, y) = key_center(&frame, "ㄱ");
    assert_eq!(keyboard.press_at(x, y).event, Some(InputEvent::Key('ㄱ')));
}

#[test]
fn touch_sequence_types_hangul_through_session() {
    let mut keyboard = Keyboard::new(layouts::dubeolsik());
    let mut session = Session::new(Box::new(HangulComposer::new()));
    let frame = keyboard.frame();
    let context = EditorContext::unavailable();

    let mut composing = String::new();
    for label in ["ㄱ", "ㅏ", "ㅂ", "ㅏ"] {
        let (x, y) = key_center(&frame, label);
        let event = keyboard.press_at(x, y).event.unwrap();
        for effect in session.handle(event, &context, None) {
            if let Effect::SetComposing(text) = effect {
                composing = text.text;
            }
        }
    }
    assert_eq!(composing, "가바");
}
