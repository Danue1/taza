//! 한국어 실팩 동작 점검 도구 — 자모 시퀀스를 단계별로 넣어 composing·후보를 출력한다.
//! 실행: cargo run -p taza-engine --example korean_probe <팩경로> <자모열>

use std::sync::Arc;
use taza_engine::contract::{EditorContext, Effect, InputEvent};
use taza_engine::engine::Engine;
use taza_engine::lang::Language;

fn main() {
    let arguments: Vec<String> = std::env::args().collect();
    let [_, pack_path, jamo_text] = arguments.as_slice() else {
        eprintln!("사용법: korean_probe <팩경로> <자모열>");
        std::process::exit(1);
    };
    let bytes = std::fs::read(pack_path).unwrap();
    let mut engine = Engine::new(Language::Korean).unwrap();
    engine.load_pack(Arc::new(bytes)).unwrap();

    let mut committed = String::new();
    let mut composing = String::new();
    for jamo in jamo_text.chars() {
        let context = EditorContext {
            text_before_cursor: Some(format!("{committed}{composing}")),
            incognito: false,
            field: taza_engine::contract::FieldKind::Text,
        };
        let event = if jamo == '<' {
            InputEvent::Backspace
        } else {
            InputEvent::Key(jamo)
        };
        let mut candidates = Vec::new();
        for effect in engine.handle(event, &context) {
            match effect {
                Effect::CommitText(text) => {
                    committed.push_str(&text);
                    composing.clear();
                }
                Effect::SetComposing(text) => composing = text.text,
                Effect::ClearComposing => composing.clear(),
                Effect::MoveCursor(_) => {}
                Effect::DeleteBackward(count) => {
                    for _ in 0..count {
                        committed.pop();
                    }
                }
                Effect::UpdateCandidates(updated) => {
                    candidates = updated
                        .into_iter()
                        .map(|candidate| candidate.text)
                        .collect();
                }
            }
        }
        println!("{jamo}: 확정={committed:?} composing={composing:?} 후보={candidates:?}");
    }
}
