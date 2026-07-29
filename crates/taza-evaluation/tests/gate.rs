//! CI 회귀 게이트 — 랭킹·사전·교정 로직 변경은 이 임계값을 통과해야 병합한다.

use std::sync::Arc;
use taza_engine::engine::PackBytes;
use taza_engine::lang::jamo::{decompose_word, encode_jamo_ascii};
use taza_engine::lang::{self, InputMethod, LanguageDescriptor};
use taza_engine::pack::SectionKind;
use taza_evaluation::synthesis::{TypedSequence, TypoSynthesizer, synthesize_cases};
use taza_evaluation::{
    CompletionTask, EvaluationCase, evaluate_completions, evaluate_corrections,
    evaluate_false_corrections,
};
use taza_pack::PackWriter;
use taza_pack::section::lexicon::LexiconBuilder;

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
    let layout = lang::latin::LATIN.default_layouts();
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
    let layout = lang::latin::LATIN.default_layouts();
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
    let cases = synthesize_cases(&lang::latin::LATIN.default_layouts(), &word_list(), 42, 5);
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
    let synthesizer = TypoSynthesizer::new(&lang::latin::LATIN.default_layouts(), 42);
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
    // 기준선 실측: 0.636
    assert_eq!(report.word_count, 12);
    assert!(
        report.keystroke_savings >= 0.55,
        "keystroke savings 회귀: {report:?}"
    );
}

/// 사전에 없지만 올바른 낱말 — 사전의 표제어에서 **편집 하나 거리**에 있다.
/// 12낱말 픽스처에서 이 낱말들은 전부 교정된다. 실팩에서는 이런 낱말이 대개 표제어로
/// 들어가 있으므로 이것은 제품의 오교정률이 아니라 마진이 걸린 자리를 비추는 눈금이다.
const ABSENT_NEAR_WORDS: [&str; 6] = ["them", "than", "helps", "worlds", "themes", "languages"];

/// 사전의 어느 표제어와도 편집거리가 멀어, 예산이 온전한 한 교정 후보가 아예 닿지 못하는
/// 낱말. 사용자의 고유명사·신조어가 서는 자리이며 여기가 무너지면 친 말이 망가진다.
const ABSENT_DISTANT_WORDS: [&str; 6] = [
    "banana", "computer", "morning", "silver", "garden", "window",
];

fn false_correction_rate(words: &[&str]) -> f64 {
    let pack: Arc<dyn PackBytes> = Arc::new(english_pack_bytes());
    let synthesizer = TypoSynthesizer::new(&lang::latin::LATIN.default_layouts(), 42);
    let cases: Vec<EvaluationCase> = words
        .iter()
        .map(|word| EvaluationCase {
            typed: TypedSequence {
                text: word.to_string(),
                touches: synthesizer.touches_for(word).unwrap(),
            },
            intended: word.to_string(),
        })
        .collect();
    let report =
        evaluate_false_corrections(&pack, &LanguageDescriptor::builtin("en").unwrap(), &cases);
    println!("[gate] english false correction {report:?}");
    report.false_correction_rate
}

/// 오교정 게이트 — 제대로 친 미등재 낱말을 자동교정이 건드리는가. 교정 정확도만 재면
/// "무엇이든 교정할수록 좋다"는 방향으로 튜닝하게 되므로 마진(`AUTOCORRECT_MARGIN`)과
/// 편집 예산을 만지는 변경은 반드시 이 값과 함께 봐야 한다.
///
/// **이 게이트가 잡는 것**: 편집 예산이 넓어져 이웃 없는 낱말까지 교정 사정권에 드는 회귀.
/// **잡지 못하는 것**: 마진을 조금 푸는 변화 — 가까운 쪽은 12낱말 픽스처에서 이미 다
/// 교정되고 있어 더 나빠질 자리가 없다. 제품의 오교정률은 실팩을 읽는 `pack_report`
/// 예제가 재고(실측 0.074), 그 표본이 CI에 들어올 수 없어(팩은 빌드 산출물이다)
/// 여기서는 방향이 분명한 쪽만 못 박는다.
#[test]
fn distant_words_are_never_false_corrected() {
    assert_eq!(false_correction_rate(&ABSENT_DISTANT_WORDS), 0.0);
}

/// 편집 하나 거리의 미등재 낱말이 어떻게 되는지를 기준선으로 남긴다 — 실측 1.0.
/// 값이 내려가면 마진·예산이 조여진 것이고 올라갈 자리는 없다.
#[test]
fn near_words_baseline_is_recorded() {
    assert_eq!(false_correction_rate(&ABSENT_NEAR_WORDS), 1.0);
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
    let layout = lang::hangul::HANGUL.default_layouts();
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
    let synthesizer = TypoSynthesizer::new(&lang::hangul::HANGUL.default_layouts(), 42);
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

/// 공간 모델이 한국어에서도 일하는가.
///
/// 터치 신호에는 배열이 내는 자모(ㄱ)가 담기고 trie에는 조회 키(두벌식 ASCII로 'r')가
/// 담긴다. 둘을 같은 공간에서 견주지 않으면 어떤 이웃도 이웃으로 보이지 않아, 인접 키
/// 오타와 판 반대편 오타가 같은 값이 된다 — 게이트의 작은 사전으로는 편집거리만으로도
/// 만점이 나와 드러나지 않으므로 여기서 따로 못 박는다.
///
/// 빈도를 같게 두고 사전순 tiebreak가 **먼 키** 쪽을 앞세우도록 골랐다. 그래야 이웃이
/// 앞에 선 것이 공간 모델이 실제로 일한 증거가 된다.
#[test]
fn spatial_model_reaches_the_hangul_key_space() {
    let mut lexicon = LexiconBuilder::new();
    // ㅁ(a)의 이웃은 ㄴ(s)이고 ㄱ(r)은 판 반대편이다
    for word in ["나", "가"] {
        let encoded = encode_jamo_ascii(&decompose_word(word).unwrap()).unwrap();
        lexicon.insert(&encoded, 30000);
    }
    let mut writer = PackWriter::new("ko");
    writer.add_section(SectionKind::Lexicon, lexicon.build());
    let pack: Arc<dyn PackBytes> = Arc::new(writer.finish());

    let layout = lang::hangul::HANGUL.default_layouts();
    let synthesizer = TypoSynthesizer::new(&layout, 42);
    let cases = [EvaluationCase {
        typed: TypedSequence {
            text: "ㅁㅏ".to_string(),
            touches: synthesizer.touches_for("ㅁㅏ").unwrap(),
        },
        intended: "나".to_string(),
    }];
    let report = evaluate_corrections(&pack, &LanguageDescriptor::builtin("ko").unwrap(), &cases);
    assert_eq!(
        report.top1_accuracy, 1.0,
        "이웃 키 오타가 먼 키 오타에 밀림 — 공간 모델이 키 공간에 닿지 못한다"
    );
}

/// 변환 회귀 게이트 — 읽기를 표기로 옮긴 결과가 정답과 얼마나 같은가.
///
/// 오타 합성 게이트와 달리 **실팩이 있어야 뜻이 있다**. 변환 품질은 사전이 실어 온 비용과
/// 연접 값이 정하는 것이라, 합성 사전으로 재면 파이프라인이 아니라 격자 코드만 보게 된다.
/// 그래서 팩이 자리에 없으면 건너뛴다 — 손으로 갖다 놓는 원천을 다루는 방식과 같다.
///
/// 문턱은 실측값 바로 아래에 둔다. 랭킹을 손보다 나빠지면 여기서 멈춘다.
mod conversion {
    use taza_engine::convert::Conversion;
    use taza_engine::pack::Pack;
    use taza_evaluation::conversion::{measure, parse_mozc_evaluation};

    /// 실측 0.683 (mozc 2.32.5994.102 · 564문장)
    const SENTENCE_FLOOR: f64 = 0.67;
    /// 실측 0.868
    const CHARACTER_FLOOR: f64 = 0.86;

    #[test]
    fn japanese_conversion_does_not_regress() {
        let pack_path = std::path::Path::new("../../out/packs/japanese.tazapack");
        let Ok(bytes) = std::fs::read(pack_path) else {
            println!("일본어 팩이 없어 건너뜀 — `taza build japanese`로 만든 뒤 다시 돈다");
            return;
        };
        let text = std::fs::read_to_string("../../data/languages/japanese/evaluation.tsv")
            .expect("평가 셋");
        let cases = parse_mozc_evaluation(&text);
        assert!(cases.len() > 500, "평가 셋이 줄었다: {}", cases.len());

        let pack = Pack::open(&bytes).expect("팩 열기");
        let conversion =
            Conversion::new(pack.conversion().expect("변환표"), pack.connection(), None);
        let metrics = measure(&conversion, &cases);
        assert!(
            metrics.sentence >= SENTENCE_FLOOR,
            "문장 정확도 {:.3} < {SENTENCE_FLOOR}",
            metrics.sentence
        );
        assert!(
            metrics.character >= CHARACTER_FLOOR,
            "글자 정확도 {:.3} < {CHARACTER_FLOOR}",
            metrics.character
        );
    }
}
