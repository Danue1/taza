//! 실팩 동작 점검 도구 — 입력을 한 글자씩 넣어 composing·후보가 어떻게 움직이는지
//! 단계별로 찍는다. 게이트가 잡지 못하는 것(문맥 재랭킹, 실어휘에서의 후보 품질)을
//! 눈으로 확인하는 용도다.
//!
//! ```text
//! cargo run -p taza-engine --example probe -- <팩경로> <en|ko> <입력>
//! ```
//! 입력에서 공백은 어절 경계, `<`는 Backspace다. 한국어 입력은 자모열로 준다.

use std::sync::Arc;
use taza_engine::contract::{EditorContext, Effect, FieldKind, InputEvent};
use taza_engine::engine::Engine;
use taza_engine::lang::Language;

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [pack_path, language, input] = arguments.as_slice() else {
        eprintln!("사용법: probe <팩경로> <en|ko> <입력>");
        std::process::exit(1);
    };
    let language = match language.as_str() {
        "ko" => Language::Korean,
        "en" => Language::English,
        other => {
            eprintln!("모르는 언어: {other}");
            std::process::exit(1);
        }
    };
    let bytes = std::fs::read(pack_path).unwrap();
    let mut engine = Engine::new(language).unwrap();
    engine.load_pack(Arc::new(bytes)).unwrap();

    let mut committed = String::new();
    let mut composing = String::new();
    for character in input.chars() {
        let context = EditorContext {
            text_before_cursor: Some(format!("{committed}{composing}")),
            incognito: false,
            field: FieldKind::Text,
        };
        let event = match character {
            '<' => InputEvent::Backspace,
            ' ' => InputEvent::Separator(' '),
            other => InputEvent::Key(other),
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
        println!("{character}: 확정={committed:?} composing={composing:?} 후보={candidates:?}");
    }
}
