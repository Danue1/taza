use taza_pack::lexicon::{Completion, LexiconBuilder};
use taza_pack::{Pack, PackError, PackWriter, SectionKind};

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
        completions.iter().map(|c| c.word.as_str()).collect::<Vec<_>>(),
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
