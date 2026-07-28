//! 필드 성격이 배열에 하는 일 — 순정은 숫자 계열만 배열을 통째로 갈고
//! 나머지는 하단 행과 리턴키만 바꾼다(`docs/inputmode.md`).

use crate::support::*;

/// 필드 성격이 화면을 바꾸는 규칙 — 실측 근거는 docs/inputmode.md.
#[test]
fn number_fields_open_a_number_pad() {
    let mut keyboard = Keyboard::new(
        lang::latin::LATIN.default_layouts(),
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
        lang::latin::LATIN.default_layouts(),
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
        lang::latin::LATIN.default_layouts(),
        LanguageDescriptor::builtin("en").unwrap(),
    );
    let plain_space = key_width(&keyboard.frame(), "␣");
    keyboard.set_field(FieldTraits::of(FieldKind::Email));
    let frame = keyboard.frame();

    key_center(&frame, "@");
    key_center(&frame, ".");
    assert!(key_width(&frame, "␣") < plain_space);
    assert_eq!(frame.metrics.candidate_bar_height, 0.0);
    // 스페이스가 줄어든 만큼만 나눠 가졌으므로 행 폭은 그대로다
    let bottom: f32 = frame.rows[3].iter().map(|key| key.bounds.width).sum();
    assert!((bottom - 1.0).abs() < 1e-6);
}

#[test]
fn url_field_replaces_space_with_dot_slash_and_domain() {
    let mut keyboard = Keyboard::new(
        lang::latin::LATIN.default_layouts(),
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
        lang::latin::LATIN.default_layouts(),
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
        lang::latin::LATIN.default_layouts(),
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
