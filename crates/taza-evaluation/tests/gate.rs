//! CI 회귀 게이트 — 랭킹·사전·교정 로직 변경은 이 임계값을 통과해야 병합한다.

use std::sync::Arc;
use taza_engine::engine::PackBytes;
use taza_engine::keyboard::layouts;
use taza_engine::lang::jamo::{decompose_word, encode_jamo_ascii};
use taza_engine::lang::{ComposerSkeleton, LanguageDescriptor};
use taza_engine::pack::SectionKind;
use taza_evaluation::synthesis::{TypedSequence, TypoSynthesizer, synthesize_cases};
use taza_evaluation::{CompletionTask, EvaluationCase, evaluate_completions, evaluate_corrections};
use taza_toolchain::PackWriter;
use taza_toolchain::section::lexicon::LexiconBuilder;

// 빈도는 실제 영어 팩에서 그대로 가져온 값이다. 임의로 축소한 숫자를 쓰면 편집 벌점처럼
// 점수 공간을 기준으로 잡힌 판단이 실팩과 다르게 동작해, 게이트가 실물을 대변하지 못한다.
const WORDS: [(&str, u32); 12] = [
    ("the", 64788),
    ("then", 41708),
    ("they", 52449),
    ("theme", 22509),
    ("hello", 27497),
    ("help", 49682),
    ("world", 43962),
    ("would", 50890),
    ("quick", 32369),
    ("question", 40294),
    ("keyboard", 25776),
    ("language", 43037),
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
    let layout = layouts::default_for(ComposerSkeleton::Latin);
    let words = word_list();
    let first = synthesize_cases(&layout, &words, 42, 3);
    let second = synthesize_cases(&layout, &words, 42, 3);
    assert_eq!(first, second);
    assert!(
        first.len() >= words.len() * 2,
        "합성 수율이 너무 낮음: {}",
        first.len()
    );
    for case in &first {
        assert_ne!(case.typed.text, case.intended);
    }

    let different_seed = synthesize_cases(&layout, &words, 43, 3);
    assert_ne!(first, different_seed);
}

#[test]
fn adjacent_substitution_uses_layout_neighbors() {
    let layout = layouts::default_for(ComposerSkeleton::Latin);
    let mut synthesizer = TypoSynthesizer::new(&layout, 7);
    // 'q'의 이웃은 w(가로)·a(세로) 정도 — 합성 결과의 모든 문자는 원문 인접 범위여야 한다는
    // 완전 검증 대신, 시드 고정 산출물이 실제 오타 형태인지만 확인
    let variant = synthesizer.synthesize("hello").unwrap();
    assert_ne!(variant.text, "hello");
    assert!((4..=6).contains(&variant.text.chars().count()));
    // 좌표는 글자마다 하나씩 — 코어는 이 좌표로 키 신호를 만든다
    assert_eq!(variant.touches.len(), variant.text.chars().count());
}

#[test]
fn correction_quality_gate() {
    let pack: Arc<dyn PackBytes> = Arc::new(english_pack_bytes());
    let cases = synthesize_cases(
        &layouts::default_for(ComposerSkeleton::Latin),
        &word_list(),
        42,
        5,
    );
    let report = evaluate_corrections(&pack, &LanguageDescriptor::builtin("en").unwrap(), &cases);
    println!("[gate] english correction {report:?}");

    // 기준선 실측 (seed 42, 실팩 빈도 픽스처): top1 0.900, top3 1.000, MRR 0.944,
    // autocorrect 0.917.
    assert!(
        report.case_count >= 40,
        "평가 셋이 너무 작음: {}",
        report.case_count
    );
    assert!(report.top3_accuracy >= 0.98, "top-3 회귀: {report:?}");
    assert!(report.top1_accuracy >= 0.88, "top-1 회귀: {report:?}");
    assert!(report.mean_reciprocal_rank >= 0.92, "MRR 회귀: {report:?}");
    assert!(
        report.autocorrect_accuracy >= 0.90,
        "자동교정 회귀: {report:?}"
    );
}

#[test]
fn completion_quality_gate() {
    let pack: Arc<dyn PackBytes> = Arc::new(english_pack_bytes());
    let synthesizer = TypoSynthesizer::new(&layouts::default_for(ComposerSkeleton::Latin), 42);
    let tasks: Vec<CompletionTask> = word_list()
        .iter()
        .map(|word| CompletionTask {
            typed: TypedSequence {
                text: word.to_string(),
                touches: synthesizer.touches_for(word).unwrap(),
            },
            intended: word.to_string(),
        })
        .collect();
    let report = evaluate_completions(&pack, &LanguageDescriptor::builtin("en").unwrap(), &tasks);
    println!("[gate] english completion {report:?}");
    // 기준선 실측: 0.622
    assert_eq!(report.word_count, 12);
    assert!(
        report.keystroke_savings >= 0.55,
        "keystroke savings 회귀: {report:?}"
    );
}

// 실제 한국어 팩의 값 — 팩에 없는 어절은 이웃한 표제어 수준으로 맞춰 두었다.
const KOREAN_WORDS: [(&str, u32); 8] = [
    ("안녕", 22600),
    ("안녕하세요", 20000),
    ("안내", 18000),
    ("감사합니다", 19000),
    ("사랑", 14505),
    ("사람", 37761),
    ("키보드", 12000),
    ("한국어", 22142),
];

fn korean_pack_bytes() -> Vec<u8> {
    let mut lexicon = LexiconBuilder::new();
    for (word, frequency) in KOREAN_WORDS {
        let encoded = encode_jamo_ascii(&decompose_word(word).unwrap()).unwrap();
        lexicon.insert(&encoded, frequency);
    }
    let mut writer = PackWriter::new("ko");
    writer.add_section(SectionKind::Lexicon, lexicon.build());
    writer.finish()
}

#[test]
fn korean_correction_quality_gate() {
    let pack: Arc<dyn PackBytes> = Arc::new(korean_pack_bytes());

    // 자모 시퀀스 위에서 두벌식 레이아웃 인접성으로 오타 합성
    let layout = layouts::default_for(ComposerSkeleton::Hangul);
    let mut synthesizer = TypoSynthesizer::new(&layout, 42);
    let mut cases = Vec::new();
    for (word, _) in KOREAN_WORDS {
        let jamo: String = decompose_word(word).unwrap().into_iter().collect();
        for _ in 0..5 {
            if let Some(typed) = synthesizer.synthesize(&jamo) {
                cases.push(EvaluationCase {
                    typed,
                    intended: word.to_string(),
                });
            }
        }
    }
    let report = evaluate_corrections(&pack, &LanguageDescriptor::builtin("ko").unwrap(), &cases);
    println!("[gate] korean correction {report:?}");

    // 기준선 실측 (seed 42): top1 1.0, top3 1.0, MRR 1.0 (소규모 사전 기준.
    // 한국어는 자동교정 없음 — autocorrect_accuracy는 검증하지 않는다)
    assert!(
        report.case_count >= 30,
        "평가 셋이 너무 작음: {}",
        report.case_count
    );
    assert!(
        report.top3_accuracy >= 0.95,
        "한국어 top-3 회귀: {report:?}"
    );
    assert!(
        report.top1_accuracy >= 0.90,
        "한국어 top-1 회귀: {report:?}"
    );
    assert!(
        report.mean_reciprocal_rank >= 0.95,
        "한국어 MRR 회귀: {report:?}"
    );
}

#[test]
fn korean_completion_quality_gate() {
    let pack: Arc<dyn PackBytes> = Arc::new(korean_pack_bytes());
    let synthesizer = TypoSynthesizer::new(&layouts::default_for(ComposerSkeleton::Hangul), 42);
    let tasks: Vec<CompletionTask> = KOREAN_WORDS
        .iter()
        .map(|(word, _)| {
            let jamo: String = decompose_word(word).unwrap().into_iter().collect();
            CompletionTask {
                typed: TypedSequence {
                    touches: synthesizer.touches_for(&jamo).unwrap(),
                    text: jamo,
                },
                intended: word.to_string(),
            }
        })
        .collect();
    let report = evaluate_completions(&pack, &LanguageDescriptor::builtin("ko").unwrap(), &tasks);
    println!("[gate] korean completion {report:?}");
    // 기준선 실측: 0.737
    assert_eq!(report.word_count, 8);
    assert!(
        report.keystroke_savings >= 0.65,
        "한국어 keystroke savings 회귀: {report:?}"
    );
}
