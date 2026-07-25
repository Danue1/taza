//! 레이아웃 섹션의 텍스트 DSL 파서와 직렬화.
//!
//! 레이아웃 문법: 한 줄 = 한 행, `---` 줄 = 레이어 구분(0=문자, 1=심볼1, 2=심볼2).
//! 공백 구분 토큰 `표기[:시프트표기][[변형문자들]][*폭비율]`. 대괄호 안 글자들은
//! 길게 눌러 고르는 변형이다. 제어 키는 이름으로: `shift`, `backspace`, `space`,
//! `enter`, `language`(다음 언어), `layer0`/`layer1`/`layer2`(레이어 전환).
//! 기본 폭 0.1. 행 맨 앞의 `*<수>` 토큰은 그 행의 높이(표준 행 대비 배수, 기본 1.0)다.
//! ```text
//! ㅂ:ㅃ ㅈ:ㅉ ㄷ:ㄸ ㄱ:ㄲ ㅅ:ㅆ ㅛ ㅕ ㅑ ㅐ:ㅒ ㅔ:ㅖ
//! shift*0.15 ㅋ ㅌ ㅊ ㅍ ㅠ ㅜ ㅡ backspace*0.15
//! layer1*0.125 language*0.125 space*0.45 enter*0.3
//! ---
//! 1 2 3 4 5 6 7 8 9 0[°]
//! ...
//! ```

use taza_engine::pack::layout::{
    KeyAction, KeyboardLayout, KeyboardLayoutSet, LayoutKey, LayoutRow,
};

const DEFAULT_KEY_WIDTH: f32 = 0.1;
const DEFAULT_ROW_HEIGHT: f32 = 1.0;

pub fn parse(text: &str) -> Result<KeyboardLayoutSet, String> {
    let mut layers = Vec::new();
    let mut rows = Vec::new();
    for (line_number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "---" {
            if rows.is_empty() {
                return Err(format!("{}행: 빈 레이어", line_number + 1));
            }
            layers.push(KeyboardLayout {
                rows: std::mem::take(&mut rows),
            });
            continue;
        }
        let mut keys = Vec::new();
        // 행 맨 앞의 `*<수>`는 키가 아니라 그 행의 높이다
        let mut tokens = line.split_whitespace().peekable();
        let mut height_ratio = DEFAULT_ROW_HEIGHT;
        if let Some(token) = tokens.peek()
            && let Some(height) = token.strip_prefix('*').and_then(|s| s.parse::<f32>().ok())
        {
            height_ratio = height;
            tokens.next();
        }
        for token in tokens {
            // `*`·`:`는 기호 키로도 쓰이므로, 양쪽이 온전할 때만 구분자로 해석한다
            let (specification, width_ratio) = match token.split_once('*') {
                Some((specification, width)) if !specification.is_empty() => {
                    match width.parse::<f32>() {
                        Ok(width) => (specification, width),
                        Err(_) => (token, DEFAULT_KEY_WIDTH),
                    }
                }
                _ => (token, DEFAULT_KEY_WIDTH),
            };
            // 변형 문자는 `[…]`로 붙인다. 대괄호 자체가 키인 경우와 겹치지 않도록
            // 앞부분이 비어 있지 않을 때만 구분자로 해석한다.
            let (specification, alternates) = match specification.split_once('[') {
                Some((base, rest)) if !base.is_empty() && rest.ends_with(']') => {
                    (base, rest.trim_end_matches(']').chars().collect())
                }
                _ => (specification, Vec::new()),
            };
            let action = match specification {
                "shift" => KeyAction::Shift,
                "backspace" => KeyAction::Backspace,
                "space" => KeyAction::Space,
                "enter" => KeyAction::Enter,
                "language" => KeyAction::LanguageSwitch,
                "layer0" => KeyAction::LayerSwitch { target: 0 },
                "layer1" => KeyAction::LayerSwitch { target: 1 },
                "layer2" => KeyAction::LayerSwitch { target: 2 },
                characters => {
                    let (base, shifted) = match characters.split_once(':') {
                        Some((base, shifted)) if !base.is_empty() && !shifted.is_empty() => {
                            (base, shifted)
                        }
                        _ => (characters, characters),
                    };
                    let single = |part: &str| -> Result<char, String> {
                        let mut iterator = part.chars();
                        match (iterator.next(), iterator.next()) {
                            (Some(character), None) => Ok(character),
                            _ => Err(format!(
                                "{}행: 키 표기는 1글자여야 함: {part:?}",
                                line_number + 1
                            )),
                        }
                    };
                    KeyAction::Character {
                        base: single(base)?,
                        shifted: single(shifted)?,
                    }
                }
            };
            keys.push(LayoutKey {
                action,
                width_ratio,
                alternates,
            });
        }
        rows.push(LayoutRow { keys, height_ratio });
    }
    if !rows.is_empty() {
        layers.push(KeyboardLayout { rows });
    }
    if layers.is_empty() {
        return Err("레이아웃에 행이 없음".to_string());
    }
    Ok(KeyboardLayoutSet { layers })
}

/// 섹션 바이트 레이아웃은 `taza_engine::pack::layout` 참조.
pub fn serialize(layout_set: &KeyboardLayoutSet) -> Vec<u8> {
    assert!(layout_set.layers.len() <= u8::MAX as usize);
    let mut output = vec![layout_set.layers.len() as u8];
    for layer in &layout_set.layers {
        assert!(layer.rows.len() <= u8::MAX as usize);
        output.push(layer.rows.len() as u8);
        for row in &layer.rows {
            let height_per_mille = (row.height_ratio * 1000.0).round() as u16;
            output.extend_from_slice(&height_per_mille.to_le_bytes());
            assert!(row.keys.len() <= u8::MAX as usize);
            output.push(row.keys.len() as u8);
            for key in &row.keys {
                let (kind, base, shifted) = match key.action {
                    KeyAction::Character { base, shifted } => (1u8, base as u32, shifted as u32),
                    KeyAction::Shift => (2, 0, 0),
                    KeyAction::Backspace => (3, 0, 0),
                    KeyAction::Space => (4, 0, 0),
                    KeyAction::Enter => (5, 0, 0),
                    KeyAction::LayerSwitch { target } => (6, target as u32, 0),
                    KeyAction::LanguageSwitch => (7, 0, 0),
                };
                output.push(kind);
                let width_per_mille = (key.width_ratio * 1000.0).round() as u16;
                output.extend_from_slice(&width_per_mille.to_le_bytes());
                output.extend_from_slice(&base.to_le_bytes());
                output.extend_from_slice(&shifted.to_le_bytes());
                assert!(key.alternates.len() <= u8::MAX as usize);
                output.push(key.alternates.len() as u8);
                for alternate in &key.alternates {
                    output.extend_from_slice(&(*alternate as u32).to_le_bytes());
                }
            }
        }
    }
    output
}
