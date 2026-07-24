use taza_ffi::{
    FfiEditorContext, FfiEffect, FfiFieldKind, FfiInputEvent, FfiLanguage, KeyboardSession,
};

fn context(text: &str) -> FfiEditorContext {
    FfiEditorContext {
        text_before_cursor: Some(text.to_string()),
        incognito: false,
        field: FfiFieldKind::Text,
    }
}

#[test]
fn korean_typing_over_ffi() {
    let session = KeyboardSession::new(FfiLanguage::Korean);
    let mut composing = String::new();
    for jamo in ["ㄱ", "ㅏ", "ㅂ", "ㅏ"] {
        let effects = session.handle_event(
            FfiInputEvent::Key {
                character: jamo.to_string(),
            },
            context(&composing),
        );
        for effect in effects {
            if let FfiEffect::SetComposing { text, .. } = effect {
                composing = text;
            }
        }
    }
    assert_eq!(composing, "가바");
}

#[test]
fn frame_and_press_over_ffi() {
    let session = KeyboardSession::new(FfiLanguage::English);
    let frame = session.keyboard_frame();
    assert_eq!(frame.rows.len(), 4);
    let q_key = frame.rows[0]
        .iter()
        .find(|key| key.label == "q")
        .unwrap();
    let result = session.press_at(
        q_key.bounds.x + q_key.bounds.width / 2.0,
        q_key.bounds.y + q_key.bounds.height / 2.0,
        context(""),
    );
    assert!(result
        .effects
        .iter()
        .any(|effect| matches!(effect, FfiEffect::CommitText { text } if text == "q")));
}

#[test]
fn pack_loading_and_suggestions_over_ffi() {
    use taza_pack::lexicon::LexiconBuilder;
    use taza_pack::{PackWriter, SectionKind};

    let mut lexicon = LexiconBuilder::new();
    lexicon.insert("hello", 80);
    lexicon.insert("help", 50);
    let mut writer = PackWriter::new("en");
    writer.add_section(SectionKind::Lexicon, lexicon.build());
    let path = std::env::temp_dir().join("taza-ffi-test.tazapack");
    std::fs::write(&path, writer.finish()).unwrap();

    let session = KeyboardSession::new(FfiLanguage::English);
    session.load_pack(path.to_string_lossy().to_string()).unwrap();

    let mut candidates = Vec::new();
    for (index, character) in ["h", "e"].iter().enumerate() {
        let typed: String = "he"[..index].to_string();
        let effects = session.handle_event(
            FfiInputEvent::Key {
                character: character.to_string(),
            },
            context(&typed),
        );
        for effect in effects {
            if let FfiEffect::UpdateCandidates {
                candidates: updated,
            } = effect
            {
                candidates = updated.into_iter().map(|candidate| candidate.text).collect();
            }
        }
    }
    assert_eq!(candidates[0], "hello");
    assert!(session.load_pack("/nonexistent/path.tazapack".to_string()).is_err());
}

#[test]
fn personalization_snapshot_over_ffi() {
    let session = KeyboardSession::new(FfiLanguage::English);
    for character in ["h", "i"] {
        session.handle_event(
            FfiInputEvent::Key {
                character: character.to_string(),
            },
            context(""),
        );
    }
    session.handle_event(
        FfiInputEvent::Separator {
            character: " ".to_string(),
        },
        context("hi"),
    );
    let snapshot = session.personalization_snapshot();
    assert!(snapshot.iter().any(|line| line.starts_with("hi\t")));

    let restored = KeyboardSession::new(FfiLanguage::English);
    restored.restore_personalization(snapshot.clone());
    assert_eq!(restored.personalization_snapshot(), snapshot);
}
