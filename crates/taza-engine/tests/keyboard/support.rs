//! 갈래마다 되풀이되는 조회 — 프레임에서 라벨로 키를 찾고, 누름에서 실제 글자를 뽑는다.

pub use taza_engine::contract::{
    EditorContext, Effect, FieldKind, FieldTraits, InputEvent, UserPreferences,
};
pub use taza_engine::engine::Engine;
pub use taza_engine::keyboard::{
    FormFactor, KeyAction, KeyLegend, KeyRole, KeySignal, Keyboard, KeyboardFrame, KeyboardLayout,
    KeyboardLayoutSet, KeyboardMetrics, LayoutKey, LayoutRow, ShellRequest, layouts,
};
pub use taza_engine::lang::LanguageDescriptor;

/// 터치는 이웃 키 확률까지 담은 신호를 만든다 — 여기서는 실제로 입력된 글자만 본다.
pub fn pressed(keyboard: &mut Keyboard, x: f32, y: f32) -> Option<char> {
    match keyboard.press_at(x, y).event {
        Some(InputEvent::Key(signal)) => Some(signal.character()),
        _ => None,
    }
}

pub fn key_width(frame: &KeyboardFrame, label: &str) -> f32 {
    for row in &frame.rows {
        for key in row {
            if key.label == label {
                return key.bounds.width;
            }
        }
    }
    panic!("키 {label:?} 없음");
}

pub fn key_center(frame: &KeyboardFrame, label: &str) -> (f32, f32) {
    for row in &frame.rows {
        for key in row {
            if key.label == label {
                return (
                    key.bounds.x + key.bounds.width / 2.0,
                    key.bounds.y + key.bounds.height / 2.0,
                );
            }
        }
    }
    panic!("키 {label:?} 없음");
}
