use taza_engine::pack::lexicon::Completion;
use taza_engine::pack::{Pack, PackError, SectionKind};
use taza_toolchain::PackWriter;
use taza_toolchain::lexicon::LexiconBuilder;

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
        lexicon.complete("the", 3),
        vec![
            Completion {
                word: "the".to_string(),
                frequency: 100
            },
            Completion {
                word: "then".to_string(),
                frequency: 70
            },
            Completion {
                word: "they".to_string(),
                frequency: 70
            },
        ]
    );
    assert!(lexicon.complete("z", 3).is_empty());
}

#[test]
fn multibyte_words_roundtrip() {
    let bytes = build_pack("ko", &[("안녕", 90), ("안녕하세요", 80), ("안내", 50)]);
    let pack = Pack::open(&bytes).unwrap();
    let lexicon = pack.lexicon().unwrap();
    assert_eq!(lexicon.frequency("안녕"), Some(90));
    let completions = lexicon.complete("안", 10);
    assert_eq!(
        completions
            .iter()
            .map(|c| c.word.as_str())
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
    use taza_toolchain::ngram::NgramModelBuilder;
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
