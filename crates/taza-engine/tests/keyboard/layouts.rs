//! 배열은 코드다 — 배열을 고르는 일이 팩(사전)을 받았는지와 무관하고, 골격마다 여러
//! 배열이 코드에 실려 있다.

use crate::support::*;

/// 팩을 하나도 싣지 않은 세션이 그대로 배열을 그리고 누름을 받는다 — 배열이 코드에
/// 있으므로 사전 설치가 선행 조건이 아니다.
#[test]
fn code_layouts_drive_the_keyboard_without_a_pack() {
    let mut keyboard = Keyboard::new(
        layouts::default_for(ComposerSkeleton::Hangul),
        LanguageDescriptor::builtin("ko").unwrap(),
    );
    let frame = keyboard.frame();
    let (x, y) = key_center(&frame, "ㄱ");
    assert_eq!(pressed(&mut keyboard, x, y), Some('ㄱ'));
}

/// 한 골격이 배열을 여러 벌 갖고, 엔진이 이름으로 그 사이를 오간다 — 전부 팩 없이.
#[test]
fn the_engine_switches_between_code_layouts_with_no_pack() {
    let mut engine = Engine::new(LanguageDescriptor::builtin("en").unwrap()).unwrap();

    assert_eq!(
        engine.available_layouts(),
        vec!["QWERTY", "QWERTZ", "AZERTY", "Colemak"]
    );
    assert_eq!(engine.layout_name(), "QWERTY");
    assert_eq!(engine.frame().rows[0][0].label, "q");

    assert!(engine.select_layout("AZERTY"));
    assert_eq!(engine.layout_name(), "AZERTY");
    // 프레임이 실제로 다시 서면 맨 앞 글자가 바뀐다
    assert_eq!(engine.frame().rows[0][0].label, "a");

    // 심볼면은 공용 부품에서 오므로 배열을 갈아도 그대로 닿는다 — 배열마다 싣지 않는다
    let frame = engine.frame();
    let (x, y) = key_center(&frame, "123");
    engine.press_at(x, y, &EditorContext::unavailable());
    key_center(&engine.frame(), "1");

    // 판올림으로 사라진 이름이 설정에 남아 있어도 조용히 지금 배열에 머문다
    assert!(!engine.select_layout("Dvorak"));
    assert_eq!(engine.layout_name(), "AZERTY");
}

/// 천지인은 조합 규칙이 두벌식과 달라 자기 골격을 밝힌다 — 배열을 고르면 합성기가 함께
/// 갈리므로, 고른 뒤 그 배열의 글자가 프레임에 선다.
#[test]
fn hangul_ships_several_layouts_including_cheonjiin() {
    let mut engine = Engine::new(LanguageDescriptor::builtin("ko").unwrap()).unwrap();

    let names = engine.available_layouts();
    assert_eq!(names.first().map(String::as_str), Some("두벌식"));
    assert!(names.iter().any(|name| name == "세벌식 최종"));
    assert!(names.iter().any(|name| name == "천지인"));

    assert!(engine.select_layout("천지인"));
    assert_eq!(engine.layout_name(), "천지인");
    // 천지인은 하늘(ㆍ)·땅(ㅡ)·사람(ㅣ)이 맨 위에 선다
    key_center(&engine.frame(), "ㆍ");
}
