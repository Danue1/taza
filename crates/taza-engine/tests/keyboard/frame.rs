//! 표현 — 키에 무엇이 적히고 어떤 역할로 나가는가, 그리고 폼팩터가 정하는 치수.

use crate::support::*;

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
    assert_eq!(bottom[3].label, "␣");
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
        taza_toolchain::section::layout::serialize(
            &taza_toolchain::section::layout::parse(&text).unwrap(),
        ),
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
