//! 배치와 히트 테스트 — 언어·상태와 무관한 순수 기하가 그려진 자리와
//! 눌린 자리를 같게 만드는가.

use crate::support::*;

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
