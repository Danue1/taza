//! 팩이 실어 온 배열 — 배열 추가가 팩 배포로 끝나는가.

use crate::support::*;

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
        taza_toolchain::section::layout::serialize(&named),
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
fn pack_can_carry_several_layouts_and_the_engine_switches_between_them() {
    use std::sync::Arc;
    use taza_engine::engine::PackBytes;
    use taza_engine::pack::SectionKind;
    use taza_toolchain::PackWriter;
    use taza_toolchain::section::metadata::MetadataBuilder;

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
        taza_toolchain::section::layout::serialize(
            &taza_toolchain::section::layout::parse(text).unwrap(),
        ),
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
