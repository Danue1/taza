//! 실팩 품질 보고 — 팩을 갈아치울 때의 A/B 판단 자료를 만든다.
//! 합성 사전으로 도는 CI 게이트(tests/gate.rs)는 로직 회귀를 잡고, 이 도구는 데이터
//! 파이프라인이 만든 실팩의 랭킹 품질을 잰다.
//!
//! ```text
//! cargo run --release -p taza-evaluation --example pack_report -- \
//!     data/packs/english.tazapack data/build/english-words.tsv en [표본수]
//! ```
//! 표본은 점수표 상위 N개 — 실제로 자주 쓰는 어휘에서 재는 것이 목적이다.

use std::sync::Arc;
use taza_engine::contract::Pack;
use taza_engine::engine::PackBytes;
use taza_engine::keyboard::layouts;
use taza_engine::lang::Language;
use taza_engine::lang::jamo::decompose_word;
use taza_evaluation::synthesis::TypoSynthesizer;
use taza_evaluation::{CompletionTask, EvaluationCase, evaluate_completions, evaluate_corrections};

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
        layouts::dubeolsik()
    } else {
        layouts::qwerty()
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
        for _ in 0..TYPOS_PER_WORD {
            if let Some(variant) = synthesizer.synthesize(&typed) {
                cases.push(EvaluationCase {
                    typed: variant,
                    intended: word.clone(),
                });
            }
        }
        tasks.push(CompletionTask {
            typed,
            intended: word.clone(),
        });
    }

    let evaluated_language = if korean {
        Language::Korean
    } else {
        Language::English
    };
    let corrections = evaluate_corrections(&pack, evaluated_language, &cases);
    let completions = evaluate_completions(&pack, evaluated_language, &tasks);

    let metadata = opened.metadata();
    println!("팩: {pack_path} ({} 언어)", opened.language());
    if let Some(metadata) = &metadata {
        for key in ["pack_version", "word_count", "lexicon_encoding", "sources"] {
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
}
