//! 실팩 품질 보고 — 팩을 갈아치울 때의 A/B 판단 자료를 만든다.
//! 합성 사전으로 도는 CI 게이트(tests/gate.rs)는 로직 회귀를 잡고, 이 도구는 데이터
//! 파이프라인이 만든 실팩의 랭킹 품질을 잰다.
//!
//! ```text
//! cargo run --release -p taza-evaluation --example pack_report -- \
//!     out/packs/english.tazapack out/build/english-words.tsv en [표본수]
//! ```
//! 같은 자리에 `<이름>-absent.tsv`가 있으면 오교정률도 함께 잰다 — 예산에 밀려 팩에서
//! 빠진 실제 낱말들을 제대로 쳤을 때 자동교정이 건드리는 비율이다.
//! 표본은 점수표 상위 N개 — 실제로 자주 쓰는 어휘에서 재는 것이 목적이다.

use std::sync::Arc;
use taza_engine::contract::Pack;
use taza_engine::engine::PackBytes;
use taza_engine::keyboard::layouts;
use taza_engine::lang::jamo::decompose_word;
use taza_engine::lang::{ComposerSkeleton, LanguageDescriptor};
use taza_evaluation::synthesis::{TypedSequence, TypoSynthesizer};
use taza_evaluation::{
    CompletionTask, EvaluationCase, evaluate_completions, evaluate_corrections,
    evaluate_false_corrections,
};

const TYPOS_PER_WORD: usize = 5;
const DEFAULT_SAMPLE: usize = 500;

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [pack_path, table_path, language, sample @ ..] = arguments.as_slice() else {
        eprintln!("사용법: pack_report <팩경로> <점수표.tsv> <en|ko> [표본수]");
        std::process::exit(1);
    };
    let sample_size = sample
        .first()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_SAMPLE);

    let table = std::fs::read_to_string(table_path).expect("점수표 읽기 실패");
    let words: Vec<String> = table
        .lines()
        .filter_map(|line| line.split('\t').next())
        .filter(|word| word.chars().count() >= 3)
        .take(sample_size)
        .map(str::to_string)
        .collect();

    let bytes = std::fs::read(pack_path).expect("팩 읽기 실패");
    let pack: Arc<dyn PackBytes> = Arc::new(bytes);
    let opened = Pack::open(pack.bytes()).expect("팩 열기 실패");

    let korean = language == "ko";
    let layout = if korean {
        layouts::default_for(ComposerSkeleton::Hangul)
    } else {
        layouts::default_for(ComposerSkeleton::Latin)
    };
    // 한국어는 화면 표기가 아니라 자모 시퀀스가 입력이다
    let typed_form = |word: &str| -> Option<String> {
        if korean {
            decompose_word(word).map(|jamo| jamo.into_iter().collect())
        } else {
            Some(word.to_string())
        }
    };

    let mut synthesizer = TypoSynthesizer::new(&layout, 42);
    let mut cases = Vec::new();
    let mut tasks = Vec::new();
    for word in &words {
        let Some(typed) = typed_form(word) else {
            continue;
        };
        // 이 배열로 칠 수 없는 낱말은 평가 대상이 아니다 (어퍼스트로피 등)
        let Some(touches) = synthesizer.touches_for(&typed) else {
            continue;
        };
        for _ in 0..TYPOS_PER_WORD {
            if let Some(variant) = synthesizer.synthesize(&typed) {
                cases.push(EvaluationCase {
                    typed: variant,
                    intended: word.clone(),
                });
            }
        }
        tasks.push(CompletionTask {
            typed: TypedSequence {
                touches,
                text: typed,
            },
            intended: word.clone(),
        });
    }

    let evaluated_language = LanguageDescriptor::builtin(language).expect("모르는 언어 태그");
    let corrections = evaluate_corrections(&pack, &evaluated_language, &cases);
    let completions = evaluate_completions(&pack, &evaluated_language, &tasks);

    let metadata = opened.metadata();
    println!("팩: {pack_path} ({} 언어)", opened.language());
    if let Some(metadata) = &metadata {
        for key in [
            "pack_version",
            "word_count",
            "bigram_count",
            "lexicon_encoding",
            "sources",
        ] {
            if let Some(value) = metadata.get(key) {
                println!("  {key}: {}", value.replace('\n', "; "));
            }
        }
    }
    println!("표본 {} 낱말 / 오타 {} 건", tasks.len(), cases.len());
    println!(
        "  top1 {:.3} / top3 {:.3} / MRR {:.3} / 자동교정 {:.3}",
        corrections.top1_accuracy,
        corrections.top3_accuracy,
        corrections.mean_reciprocal_rank,
        corrections.autocorrect_accuracy
    );
    println!("  keystroke savings {:.3}", completions.keystroke_savings);

    // 사전에 없지만 올바른 낱말 — 이것을 건드리면 사용자가 친 것을 망친 것이다
    let absent_path = std::path::Path::new(table_path).with_file_name(format!(
        "{}-absent.tsv",
        std::path::Path::new(table_path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(|stem| stem.strip_suffix("-words"))
            .unwrap_or("")
    ));
    if let Ok(text) = std::fs::read_to_string(&absent_path) {
        let absent: Vec<EvaluationCase> = text
            .lines()
            .filter_map(|word| {
                let typed = typed_form(word)?;
                let touches = synthesizer.touches_for(&typed)?;
                Some(EvaluationCase {
                    typed: TypedSequence {
                        text: typed,
                        touches,
                    },
                    intended: word.to_string(),
                })
            })
            .take(sample_size)
            .collect();
        let report = evaluate_false_corrections(&pack, &evaluated_language, &absent);
        println!(
            "미등재 낱말 {} 개 (예산에 밀려 빠진 실제 낱말)",
            report.word_count
        );
        println!("  오교정률 {:.3}", report.false_correction_rate);
    }
}
