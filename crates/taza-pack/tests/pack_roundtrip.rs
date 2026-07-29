use taza_engine::pack::connection::DEFAULT_CONNECTION_COST;
use taza_engine::pack::{Pack, PackError, SectionKind};
use taza_engine::suggest::{Dictionary, Entry, KeyEncoding, Query};
use taza_pack::PackWriter;
use taza_pack::section::conversion::{
    ConnectionBuilder, ConversionBuilder, Entry as ConversionEntry,
};
use taza_pack::section::lexicon::LexiconBuilder;

/// 진행 중인 낱말의 완성 조회 — 뒤에 남는 글자에 비용을 물리지 않는다.
fn completion_query(key: &str) -> Query<'_> {
    Query {
        key,
        max_cost: 0,
        touches: &[],
        encoding: KeyEncoding::Utf8,
        extending: true,
    }
}

fn build_pack(language: &str, words: &[(&str, u32)]) -> Vec<u8> {
    let mut lexicon = LexiconBuilder::new();
    for (word, frequency) in words {
        lexicon.insert(word, *frequency);
    }
    let mut writer = PackWriter::new(language);
    writer.add_section(SectionKind::Lexicon, lexicon.build());
    writer.finish()
}

#[test]
fn exact_lookup() {
    let bytes = build_pack("en", &[("the", 100), ("theme", 40), ("apple", 60)]);
    let pack = Pack::open(&bytes).unwrap();
    assert_eq!(pack.language(), "en");
    let lexicon = pack.lexicon().unwrap();
    assert_eq!(lexicon.frequency("the"), Some(100));
    assert_eq!(lexicon.frequency("apple"), Some(60));
    assert_eq!(lexicon.frequency("th"), None);
    assert!(!lexicon.contains("them"));
}

#[test]
fn prefix_completion_orders_by_frequency() {
    let bytes = build_pack(
        "en",
        &[
            ("the", 100),
            ("theme", 40),
            ("then", 70),
            ("they", 70),
            ("apple", 60),
        ],
    );
    let pack = Pack::open(&bytes).unwrap();
    let lexicon = pack.lexicon().unwrap();
    assert_eq!(
        lexicon.search(&completion_query("the"), 3),
        vec![
            Entry {
                key: "the".to_string(),
                frequency: 100,
                cost: 0
            },
            Entry {
                key: "then".to_string(),
                frequency: 70,
                cost: 0
            },
            Entry {
                key: "they".to_string(),
                frequency: 70,
                cost: 0
            },
        ]
    );
    assert!(lexicon.search(&completion_query("z"), 3).is_empty());
}

#[test]
fn multibyte_words_roundtrip() {
    let bytes = build_pack("ko", &[("안녕", 90), ("안녕하세요", 80), ("안내", 50)]);
    let pack = Pack::open(&bytes).unwrap();
    let lexicon = pack.lexicon().unwrap();
    assert_eq!(lexicon.frequency("안녕"), Some(90));
    let completions = lexicon.search(&completion_query("안"), 10);
    assert_eq!(
        completions
            .iter()
            .map(|entry| entry.key.as_str())
            .collect::<Vec<_>>(),
        vec!["안녕", "안녕하세요", "안내"]
    );
}

#[test]
fn duplicate_insert_keeps_higher_frequency() {
    let bytes = build_pack("en", &[("hi", 10), ("hi", 30)]);
    let pack = Pack::open(&bytes).unwrap();
    assert_eq!(pack.lexicon().unwrap().frequency("hi"), Some(30));
}

#[test]
fn ngram_model_roundtrip() {
    use taza_pack::section::ngram::NgramModelBuilder;
    let mut ngram = NgramModelBuilder::new();
    ngram.insert_bigram("the", "quick", 30);
    ngram.insert_bigram("the", "best", 50);
    ngram.insert_bigram("the", "quick", 20); // 누적 → 50
    ngram.insert_bigram("안녕", "하세요", 10);
    let mut writer = PackWriter::new("en");
    writer.add_section(SectionKind::NgramModel, ngram.build());
    let bytes = writer.finish();

    let pack = Pack::open(&bytes).unwrap();
    assert!(pack.lexicon().is_none());
    let language_model = pack.language_model().unwrap();

    let predictions: Vec<(String, u32)> = language_model
        .predict_next("the", 10)
        .into_iter()
        .map(|prediction| (prediction.word, prediction.weight))
        .collect();
    assert_eq!(
        predictions,
        vec![("best".to_string(), 50), ("quick".to_string(), 50)]
    );
    assert_eq!(language_model.predict_next("the", 1).len(), 1);
    assert_eq!(language_model.predict_next("안녕", 10)[0].word, "하세요");
    assert!(language_model.predict_next("unknown", 10).is_empty());
}

#[test]
fn rejects_invalid_input() {
    assert_eq!(Pack::open(b"NOPE").unwrap_err(), PackError::InvalidMagic);
    assert_eq!(Pack::open(b"TA").unwrap_err(), PackError::Truncated);

    let mut bytes = build_pack("en", &[("the", 100)]);
    bytes[4] = 0xFF;
    bytes[5] = 0xFF;
    assert_eq!(
        Pack::open(&bytes).unwrap_err(),
        PackError::UnsupportedVersion(0xFFFF)
    );

    let bytes = build_pack("en", &[("the", 100)]);
    assert_eq!(
        Pack::open(&bytes[..bytes.len() - 1]).unwrap_err(),
        PackError::Truncated
    );
}

/// 곁들일 것은 낱말 후보 뒤에 갈래 순서대로 붙는다 — 낱말이 설 자리를 가져가지 않는다.
#[test]
fn annotations_accompany_word_suggestions() {
    use taza_engine::contract::CandidateGroup;
    use taza_engine::suggest::{KeyEncoding, Suggester, SuggestionPolicy, SuggestionSources};
    use taza_pack::section::annotation::AnnotationBuilder;

    let encoding = KeyEncoding::HangulJamoDubeolsik;
    // 조회 키는 낱말에서 뽑는다 — 손으로 적으면 자모 순서를 틀리기 쉽다
    let key = |word: &str| encoding.encode(word).unwrap();

    let mut lexicon = LexiconBuilder::new();
    lexicon.insert(&key("웃음"), 60000);
    lexicon.insert(&key("웃음소리"), 30000);
    let mut annotations = AnnotationBuilder::new();
    annotations.insert(&key("웃음"), CandidateGroup::Emoji, "😀");
    annotations.insert(&key("웃음"), CandidateGroup::Emoticon, "(^_^)");

    let mut writer = PackWriter::new("ko");
    writer.add_section(SectionKind::Lexicon, lexicon.build());
    writer.add_section(SectionKind::Annotation, annotations.build());
    let bytes = writer.finish();
    let pack = Pack::open(&bytes).unwrap();

    let suggester = Suggester::new(SuggestionPolicy {
        encoding,
        autocorrect: false,
        limit: 3,
        annotation_limit: 1,
    });
    let sources = SuggestionSources {
        pack: Some(&pack),
        personalization: None,
        previous_word: None,
        touches: &[],
    };

    let suggestions = suggester.suggest(&key("웃음"), &sources);
    let accompanying: Vec<(CandidateGroup, &str)> = suggestions
        .iter()
        .filter(|suggestion| suggestion.group != CandidateGroup::Word)
        .map(|suggestion| (suggestion.group, suggestion.text.as_str()))
        .collect();
    // 갈래 순서대로 — 이모지가 먼저, 얼굴 문자가 뒤
    assert_eq!(
        accompanying,
        vec![
            (CandidateGroup::Emoji, "😀"),
            (CandidateGroup::Emoticon, "(^_^)"),
        ]
    );
    // 낱말이 먼저다 — 곁들이는 것은 뒤에 붙는다
    assert_eq!(suggestions[0].group, CandidateGroup::Word);
    assert!(
        suggestions
            .iter()
            .any(|suggestion| suggestion.text == "웃음")
    );

    // 어절이 완성되기 전에는 튀어나오지 않는다
    let partial = suggester.suggest(&key("웃"), &sources);
    assert!(
        partial
            .iter()
            .all(|suggestion| suggestion.group == CandidateGroup::Word)
    );
}

/// 읽기 하나에 표기가 여럿 딸리는 표 — 조회 키를 되돌릴 수 없는 언어가 서는 자리다.
fn build_conversion_pack(readings: &[(&str, &[(&str, u16)])]) -> Vec<u8> {
    let mut conversion = ConversionBuilder::new();
    for (reading, surfaces) in readings {
        for (surface, cost) in *surfaces {
            conversion.insert(
                reading,
                ConversionEntry {
                    surface: surface.to_string(),
                    left_id: 1,
                    right_id: 1,
                    cost: *cost,
                    dependent: false,
                },
            );
        }
    }
    let (trie, store) = conversion.build();
    let mut writer = PackWriter::new("ja");
    writer.add_section(SectionKind::Conversion, trie);
    writer.add_section(SectionKind::ConversionEntry, store);
    writer.finish()
}

#[test]
fn conversion_lookup_returns_every_surface_cheapest_first() {
    let bytes = build_conversion_pack(&[
        ("きしゃ", &[("汽車", 300), ("記者", 100), ("貴社", 500)]),
        ("は", &[("は", 10)]),
    ]);
    let pack = Pack::open(&bytes).unwrap();
    let table = pack.conversion().unwrap();
    let surfaces: Vec<&str> = table
        .lookup("きしゃ")
        .unwrap()
        .iter()
        .map(|entry| entry.surface)
        .collect();
    assert_eq!(surfaces, ["記者", "汽車", "貴社"]);
    assert_eq!(table.lookup("きし"), None);
    assert_eq!(table.lookup("は").unwrap().best().unwrap().surface, "は");
}

/// 라티스가 마디를 세우는 통로 — 한 자리에서 시작하는 표제어를 한 번의 순회로 모은다.
#[test]
fn conversion_prefixes_stop_at_character_boundaries() {
    let bytes = build_conversion_pack(&[
        ("に", &[("に", 10)]),
        ("にわ", &[("庭", 20)]),
        ("にわに", &[("にわに", 900)]),
    ]);
    let pack = Pack::open(&bytes).unwrap();
    let table = pack.conversion().unwrap();
    let reading = "にわにはにわ";
    let found: Vec<usize> = table
        .prefixes(reading, 0)
        .into_iter()
        .map(|(end, _)| end)
        .collect();
    // 가나 하나가 3바이트 — に·にわ·にわに 셋이 선다
    assert_eq!(found, [3, 6, 9]);
    // 넷째 글자(は) 자리에서 다시 세우면 그 자리의 표제어만 나온다
    assert!(table.prefixes(reading, 9).is_empty());
}

#[test]
fn conversion_completions_come_cheapest_first() {
    let bytes = build_conversion_pack(&[
        ("かい", &[("回", 100)]),
        ("かいしゃ", &[("会社", 50)]),
        ("かいだん", &[("階段", 300)]),
        ("さくら", &[("桜", 10)]),
    ]);
    let pack = Pack::open(&bytes).unwrap();
    let table = pack.conversion().unwrap();
    let readings: Vec<String> = table
        .completions("か", 3)
        .into_iter()
        .map(|(reading, _)| reading)
        .collect();
    assert_eq!(readings, ["かいしゃ", "かい", "かいだん"]);
    assert!(table.completions("ま", 3).is_empty());
}

#[test]
fn connection_matrix_falls_back_outside_the_table() {
    let mut connection = ConnectionBuilder::new(2, 2);
    connection.set(0, 1, -400);
    connection.set(1, 0, 700);
    let mut writer = PackWriter::new("ja");
    writer.add_section(SectionKind::Connection, connection.build());
    let bytes = writer.finish();
    let pack = Pack::open(&bytes).unwrap();
    let matrix = pack.connection().unwrap();
    assert_eq!(matrix.cost(0, 1), -400);
    assert_eq!(matrix.cost(1, 0), 700);
    assert_eq!(matrix.cost(0, 0), 0);
    // 표 밖의 자리는 기본값으로 물러난다 — 사전과 표의 판이 어긋나도 변환은 돈다
    assert_eq!(matrix.cost(9, 9), DEFAULT_CONNECTION_COST);
}

/// 변환표가 반쪽만 실린 팩 — trie와 곳간은 짝이라 하나만으로는 표가 서지 않는다.
#[test]
fn conversion_needs_both_sections() {
    let (trie, _) = ConversionBuilder::new().build();
    let mut writer = PackWriter::new("ja");
    writer.add_section(SectionKind::Conversion, trie);
    let bytes = writer.finish();
    assert!(Pack::open(&bytes).unwrap().conversion().is_none());
}
