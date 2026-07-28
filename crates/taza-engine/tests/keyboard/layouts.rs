//! 배열은 코드다 — 배열을 고르는 일이 팩(사전)을 받았는지와 무관하고, 입력 방식마다
//! 여러 배열이 코드에 실려 있다.

use crate::support::*;

/// 팩을 하나도 싣지 않은 세션이 그대로 배열을 그리고 누름을 받는다 — 배열이 코드에
/// 있으므로 사전 설치가 선행 조건이 아니다.
#[test]
fn code_layouts_drive_the_keyboard_without_a_pack() {
    let mut keyboard = Keyboard::new(
        lang::hangul::HANGUL.default_layouts(),
        LanguageDescriptor::builtin("ko").unwrap(),
    );
    let frame = keyboard.frame();
    let (x, y) = key_center(&frame, "ㄱ");
    assert_eq!(pressed(&mut keyboard, x, y), Some('ㄱ'));
}

/// 한 방식이 배열을 여러 벌 갖고, 엔진이 이름으로 그 사이를 오간다 — 전부 팩 없이.
#[test]
fn the_engine_switches_between_code_layouts_with_no_pack() {
    let mut engine = Engine::new(LanguageDescriptor::builtin("en").unwrap());

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

/// 방식은 태그로 찾는다 — 팩 메타데이터와 설정이 방식을 가리키는 유일한 이름이고,
/// 이 빌드에 없는 이름은 조용히 None이 되어 그 언어가 비활성 처리된다.
#[test]
fn input_methods_are_found_by_tag() {
    for tag in ["direct", "latin", "hangul", "hangul-cheonjiin"] {
        let method = lang::input_method(tag).expect("기본 빌드에 실린 방식");
        assert_eq!(method.tag(), tag);
    }
    assert!(lang::input_method("naratgeul").is_none());
}

/// 방식 하나가 자기 배열과 자기 합성기를 함께 갖는다 — 배열만 있고 합성기가 없거나
/// 배열이 이 빌드에 없는 방식을 가리키는 상태가 생기지 않아야 한다.
#[test]
fn every_method_carries_its_own_layouts() {
    for tag in ["direct", "latin", "hangul", "hangul-cheonjiin"] {
        let method = lang::input_method(tag).unwrap();
        let layouts = method.layouts();
        assert!(!layouts.is_empty(), "{tag}: 배열이 없다");
        for entry in layouts {
            // 배열이 밝힌 방식은 반드시 레지스트리에 있다 — `&'static dyn`이므로
            // 링크되지 않은 방식을 가리킬 길이 애초에 없고, 태그로 되찾을 수 있다
            if let Some(declared) = entry.method {
                assert_eq!(
                    lang::input_method(declared.tag()),
                    Some(declared),
                    "{}: 레지스트리에 없는 방식을 가리킨다",
                    entry.name
                );
            }
        }
    }
}

/// 같은 스크립트를 치는 방식들은 같은 배열 목록을 낸다 — 천지인으로 들어온 사람이
/// 두벌식으로 갈아탈 길이 막히면 안 된다.
#[test]
fn methods_of_one_script_share_the_layout_list() {
    let names = |tag: &str| -> Vec<&'static str> {
        lang::input_method(tag)
            .unwrap()
            .layouts()
            .iter()
            .map(|entry| entry.name)
            .collect()
    };
    assert_eq!(names("hangul"), names("hangul-cheonjiin"));
}

/// 천지인은 조합 규칙이 두벌식과 달라 자기 방식을 밝힌다 — 배열을 고르면 합성기가 함께
/// 갈리므로, 고른 뒤 그 배열의 글자가 프레임에 선다.
#[test]
fn hangul_ships_several_layouts_including_cheonjiin() {
    let mut engine = Engine::new(LanguageDescriptor::builtin("ko").unwrap());

    let names = engine.available_layouts();
    assert_eq!(names.first().map(String::as_str), Some("두벌식"));
    assert!(names.iter().any(|name| name == "세벌식 최종"));
    assert!(names.iter().any(|name| name == "천지인"));

    assert!(engine.select_layout("천지인"));
    assert_eq!(engine.layout_name(), "천지인");
    // 천지인은 하늘(ㆍ)·땅(ㅡ)·사람(ㅣ)이 맨 위에 선다
    key_center(&engine.frame(), "ㆍ");
}
