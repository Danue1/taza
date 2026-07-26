use taza_ffi::{
    FfiCandidateGroup, FfiEditorContext, FfiEffect, FfiFieldKind, FfiFormFactor, FfiInputEvent,
    FfiUserPreferences, KeyboardSession,
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
    let session = KeyboardSession::new("ko".to_string()).unwrap();
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
    let session = KeyboardSession::new("en".to_string()).unwrap();
    let frame = session.keyboard_frame();
    assert_eq!(frame.rows.len(), 4);
    let q_key = frame.rows[0].iter().find(|key| key.label == "q").unwrap();
    let result = session.press_at(
        q_key.bounds.x + q_key.bounds.width / 2.0,
        q_key.bounds.y + q_key.bounds.height / 2.0,
        context(""),
    );
    assert!(
        result
            .effects
            .iter()
            .any(|effect| matches!(effect, FfiEffect::CommitText { text } if text == "q"))
    );
}

#[test]
fn pack_loading_and_suggestions_over_ffi() {
    use taza_engine::pack::SectionKind;
    use taza_toolchain::PackWriter;
    use taza_toolchain::lexicon::LexiconBuilder;

    let mut lexicon = LexiconBuilder::new();
    lexicon.insert("hello", 80);
    lexicon.insert("help", 50);
    let mut writer = PackWriter::new("en");
    writer.add_section(SectionKind::Lexicon, lexicon.build());
    let path = std::env::temp_dir().join("taza-ffi-test.tazapack");
    std::fs::write(&path, writer.finish()).unwrap();

    let session = KeyboardSession::new("en".to_string()).unwrap();
    session
        .load_pack(path.to_string_lossy().to_string())
        .unwrap();

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
                candidates = updated
                    .into_iter()
                    .map(|candidate| candidate.text)
                    .collect();
            }
        }
    }
    // [0]은 원문 슬롯이고 사전이 내놓는 후보는 그 뒤에 온다
    assert_eq!(candidates[0], "he");
    assert_eq!(candidates[1], "hello");
    assert!(
        session
            .load_pack("/nonexistent/path.tazapack".to_string())
            .is_err()
    );
}

#[test]
fn personalization_snapshot_over_ffi() {
    let session = KeyboardSession::new("en".to_string()).unwrap();
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
    // 검색면에서 고른 것도 같은 스냅샷에 담긴다 — 낱말 학습은 w, 곁들임 최근 사용은 a
    session.select_annotation(FfiCandidateGroup::Emoji, "😀".to_string(), context("hi "));
    let snapshot = session.personalization_snapshot();
    assert!(snapshot.iter().any(|line| line.starts_with("w\thi\t")));
    assert!(snapshot.iter().any(|line| line == "a\t1\t😀"));

    let restored = KeyboardSession::new("en".to_string()).unwrap();
    restored.restore_personalization(snapshot.clone());
    assert_eq!(restored.personalization_snapshot(), snapshot);
}

#[test]
fn preferences_injection_reaches_the_core() {
    let session = KeyboardSession::new("en".to_string()).unwrap();
    session.set_preferences(FfiUserPreferences {
        double_space_period: false,
        ..taza_ffi::default_user_preferences()
    });
    let effects = session.handle_event(
        FfiInputEvent::Separator {
            character: " ".to_string(),
        },
        context("hi "),
    );
    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect, FfiEffect::CommitText { text } if text == " "))
    );
}

#[test]
fn metrics_injection_changes_measured_sizes() {
    let session = KeyboardSession::new("en".to_string()).unwrap();
    let portrait = session.keyboard_frame().metrics;
    assert!(
        (portrait.total_height - (portrait.grid_height + portrait.candidate_bar_height)).abs()
            < 1e-6
    );

    session.set_metrics(FfiFormFactor::PhoneLandscape, 844.0);
    let landscape = session.keyboard_frame();
    // 배열은 그대로고 높이만 줄어든다 — 폼팩터 판단은 코어가 한다
    assert_eq!(landscape.rows[0].len(), 10);
    assert!(landscape.metrics.grid_height < portrait.grid_height);
    assert_eq!(
        session.frame_metrics().grid_height,
        landscape.metrics.grid_height
    );
}

#[test]
fn pack_archive_installs_after_verification() {
    use taza_engine::pack::SectionKind;
    use taza_ffi::{install_pack_archive, read_installed_pack};
    use taza_toolchain::PackWriter;
    use taza_toolchain::distribute;
    use taza_toolchain::lexicon::LexiconBuilder;
    use taza_toolchain::metadata::MetadataBuilder;

    let mut lexicon = LexiconBuilder::new();
    lexicon.insert("hello", 80);
    let mut metadata = MetadataBuilder::new();
    metadata.set(taza_engine::pack::metadata::keys::PACK_VERSION, "7");
    metadata.set(taza_engine::pack::metadata::keys::WORD_COUNT, "1");
    metadata.set(
        taza_engine::pack::metadata::keys::SOURCES,
        "테스트 원천\t1.0\tCC-BY 4.0\t테스트 원천 표시\n칸이 모자란 줄",
    );
    let mut writer = PackWriter::new("en");
    writer.add_section(SectionKind::Lexicon, lexicon.build());
    writer.add_section(SectionKind::Metadata, metadata.build());
    let pack = writer.finish();
    let archive = distribute::compress(&pack).unwrap();

    let directory = std::env::temp_dir().join("taza-install-test");
    std::fs::create_dir_all(&directory).unwrap();
    let archive_path = directory.join("english.tazapack.zst");
    std::fs::write(&archive_path, &archive.bytes).unwrap();
    let destination = directory.join("english.tazapack");

    let pack_sha256 = {
        use sha2::{Digest, Sha256};
        Sha256::digest(&pack)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    let installed = install_pack_archive(
        archive_path.to_string_lossy().to_string(),
        destination.to_string_lossy().to_string(),
        archive.sha256.clone(),
        pack_sha256.clone(),
    )
    .unwrap();
    assert_eq!(installed.language, "en");
    assert_eq!(installed.pack_version, 7);
    // 칸이 모자란 줄은 버린다 — 형식이 다른 옛 팩을 반쯤 읽지 않는다
    assert_eq!(installed.sources.len(), 1);
    assert_eq!(installed.sources[0].name, "테스트 원천");
    assert_eq!(installed.sources[0].license, "CC-BY 4.0");
    assert_eq!(installed.sources[0].attribution, "테스트 원천 표시");
    assert_eq!(
        read_installed_pack(destination.to_string_lossy().to_string())
            .unwrap()
            .word_count,
        1
    );
    // 설치된 팩은 세션이 바로 mmap할 수 있다
    let session = KeyboardSession::new("en".to_string()).unwrap();
    session
        .load_pack(destination.to_string_lossy().to_string())
        .unwrap();

    // 해시가 어긋나면 설치하지 않는다 — 손상·중간 개입을 조용히 넘기지 않는다
    assert!(
        install_pack_archive(
            archive_path.to_string_lossy().to_string(),
            destination.to_string_lossy().to_string(),
            "0".repeat(64),
            pack_sha256,
        )
        .is_err()
    );
}
