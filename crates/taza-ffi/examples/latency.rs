//! FFI 경계(Mutex + 변환 + 팩 재개방 포함) 이벤트 왕복 지연 실측.
//! 데스크톱 기준선용 — 실기기 스파이크 전 회귀 감지에 쓴다.
//! 실행: cargo run --release -p taza-ffi --example latency [팩경로]
//! 팩 경로를 주면 그 팩(실데이터)으로, 없으면 내장 소형 팩으로 측정한다.

use taza_engine::pack::SectionKind;
use taza_ffi::{FfiEditorContext, FfiFieldKind, FfiInputEvent, FfiLanguage, KeyboardSession};
use taza_toolchain::PackWriter;
use taza_toolchain::lexicon::LexiconBuilder;

fn main() {
    let path = match std::env::args().nth(1) {
        Some(path) => path.into(),
        None => {
            let mut lexicon = LexiconBuilder::new();
            for (word, frequency) in [
                ("the", 1000u32),
                ("hello", 500),
                ("help", 300),
                ("keyboard", 120),
                ("language", 110),
            ] {
                lexicon.insert(word, frequency);
            }
            let mut writer = PackWriter::new("en");
            writer.add_section(SectionKind::Lexicon, lexicon.build());
            let path = std::env::temp_dir().join("taza-latency.tazapack");
            std::fs::write(&path, writer.finish()).unwrap();
            path
        }
    };

    let session = KeyboardSession::new(FfiLanguage::English).unwrap();
    session
        .load_pack(path.to_string_lossy().to_string())
        .unwrap();

    let word = "hello ";
    let iterations = 2000usize;
    let mut durations = Vec::with_capacity(iterations * word.len());
    let mut committed = String::new();
    for _ in 0..iterations {
        for character in word.chars() {
            let event = if character == ' ' {
                FfiInputEvent::Separator {
                    character: " ".to_string(),
                }
            } else {
                FfiInputEvent::Key {
                    character: character.to_string(),
                }
            };
            let context = FfiEditorContext {
                text_before_cursor: Some(committed.clone()),
                incognito: false,
                field: FfiFieldKind::Text,
            };
            let start = std::time::Instant::now();
            let effects = session.handle_event(event, context);
            durations.push(start.elapsed());
            for effect in effects {
                if let taza_ffi::FfiEffect::CommitText { text } = effect {
                    committed.push_str(&text);
                }
            }
        }
        committed.clear();
    }
    durations.sort();
    let percentile = |fraction: f64| durations[(durations.len() as f64 * fraction) as usize];
    println!(
        "events={} p50={:?} p99={:?} max={:?}",
        durations.len(),
        percentile(0.50),
        percentile(0.99),
        durations[durations.len() - 1],
    );
}
