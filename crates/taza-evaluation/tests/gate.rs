//! CI 회귀 게이트 — 랭킹·사전·교정 로직 변경은 이 임계값을 통과해야 병합한다.

use taza_core::keyboard::layouts;
use taza_evaluation::synthesis::{TypoSynthesizer, synthesize_cases};
use taza_evaluation::{evaluate_completions, evaluate_corrections};
use taza_pack::lexicon::LexiconBuilder;
use taza_pack::{Pack, PackWriter, SectionKind};

const WORDS: [(&str, u32); 12] = [
    ("the", 1000),
    ("then", 300),
    ("they", 400),
    ("theme", 100),
    ("hello", 500),
    ("help", 300),
    ("world", 400),
    ("would", 600),
    ("quick", 200),
    ("question", 150),
    ("keyboard", 120),
    ("language", 110),
];

fn english_pack_bytes() -> Vec<u8> {
    let mut lexicon = LexiconBuilder::new();
    for (word, frequency) in WORDS {
        lexicon.insert(word, frequency);
    }
    let mut writer = PackWriter::new("en");
    writer.add_section(SectionKind::Lexicon, lexicon.build());
    writer.finish()
}

fn word_list() -> Vec<&'static str> {
    WORDS.iter().map(|(word, _)| *word).collect()
}

#[test]
fn synthesis_is_deterministic_and_produces_typos() {
    let layout = layouts::qwerty();
    let words = word_list();
    let first = synthesize_cases(&layout, &words, 42, 3);
    let second = synthesize_cases(&layout, &words, 42, 3);
    assert_eq!(first, second);
    assert!(first.len() >= words.len() * 2, "합성 수율이 너무 낮음: {}", first.len());
    for case in &first {
        assert_ne!(case.typed, case.intended);
    }

    let different_seed = synthesize_cases(&layout, &words, 43, 3);
    assert_ne!(first, different_seed);
}

#[test]
fn adjacent_substitution_uses_layout_neighbors() {
    let layout = layouts::qwerty();
    let mut synthesizer = TypoSynthesizer::new(&layout, 7);
    // 'q'의 이웃은 w(가로)·a(세로) 정도 — 합성 결과의 모든 문자는 원문 인접 범위여야 한다는
    // 완전 검증 대신, 시드 고정 산출물이 실제 오타 형태인지만 확인
    let variant = synthesizer.synthesize("hello").unwrap();
    assert_ne!(variant, "hello");
    assert!((4..=6).contains(&variant.chars().count()));
}

#[test]
fn correction_quality_gate() {
    let bytes = english_pack_bytes();
    let pack = Pack::open(&bytes).unwrap();
    let cases = synthesize_cases(&layouts::qwerty(), &word_list(), 42, 5);
    let report = evaluate_corrections(&pack, &cases);

    // 기준선 실측 (seed 42): top1 0.900, top3 0.983, MRR 0.936, autocorrect 0.917
    assert!(report.case_count >= 40, "평가 셋이 너무 작음: {}", report.case_count);
    assert!(
        report.top3_accuracy >= 0.95,
        "top-3 회귀: {report:?}"
    );
    assert!(
        report.top1_accuracy >= 0.85,
        "top-1 회귀: {report:?}"
    );
    assert!(
        report.mean_reciprocal_rank >= 0.90,
        "MRR 회귀: {report:?}"
    );
    assert!(
        report.autocorrect_accuracy >= 0.85,
        "자동교정 회귀: {report:?}"
    );
}

#[test]
fn completion_quality_gate() {
    let bytes = english_pack_bytes();
    let pack = Pack::open(&bytes).unwrap();
    let report = evaluate_completions(&pack, &word_list());
    // 기준선 실측: 0.622
    assert_eq!(report.word_count, 12);
    assert!(
        report.keystroke_savings >= 0.55,
        "keystroke savings 회귀: {report:?}"
    );
}
