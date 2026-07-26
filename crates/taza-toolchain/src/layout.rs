//! 레이아웃 섹션의 텍스트 DSL 파서와 직렬화.
//!
//! 레이아웃 문법: 한 줄 = 한 행, `---` 줄 = 레이어 구분(0=문자, 1=심볼1, 2=심볼2,
//! 3=통합 검색면). `panel *<수>` 줄은 그 레이어의 키 위에 놓이는 패널 높이(표준 행 대비
//! 배수)를 선언한다 — 통합 검색면이 이 줄로 자기 자리를 잡는다.
//! 공백 구분 토큰 `표기[:시프트표기][[변형문자들]][*폭비율]`. 대괄호 안 글자들은
//! 길게 눌러 고르는 변형이다. 제어 키는 이름으로: `shift`, `backspace`, `space`,
//! `enter`, `language`(다음 언어), `layer0`~`layer3`(레이어 전환), `blank`(빈 자리).
//! 시프트 표기 없이 여러 글자를 적으면 한 번에 넣는 키다 (`.com`).
//! `ㄱ|ㅋ|ㄲ`처럼 세로줄로 이으면 이어 누를 때마다 갈리는 멀티탭 키다(천지인).
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
    KeyAction, KeyboardLayout, KeyboardLayoutSet, LayoutKey, LayoutRow, NamedLayoutSet,
};

const DEFAULT_KEY_WIDTH: f32 = 0.1;
const DEFAULT_ROW_HEIGHT: f32 = 1.0;

/// 한 파일에 배열을 여러 벌 적을 수 있다 — `=== <이름>` 줄이 새 배열을 연다. 이름 줄이
/// 하나도 없으면 파일 전체가 이름 없는 배열 한 벌이고, 이름은 레시피가 댄다.
/// `=== <이름> : <골격>`으로 적으면 그 배열이 쓸 조합 골격을 밝힌다 — 같은 언어 안에서
/// 조합 규칙이 다른 배열(천지인)만 필요한 표기이고, 없으면 언어의 골격을 쓴다.
///
/// 뒤에 오는 배열이 레이어를 덜 적으면 첫 배열의 나머지 레이어를 그대로 물려받는다.
/// 배열끼리 다른 것은 대개 문자면뿐이라(QWERTY와 Dvorak의 심볼면은 같다) 이 규칙이
/// 없으면 심볼면 세 벌이 배열마다 복사된다.
pub fn parse(text: &str) -> Result<Vec<NamedLayoutSet>, String> {
    let mut blocks: Vec<(String, Option<String>, String)> = Vec::new();
    for line in text.lines() {
        match line.trim().strip_prefix("===") {
            Some(heading) => {
                let (name, skeleton) = match heading.split_once(':') {
                    Some((name, skeleton)) => (name.trim(), Some(skeleton.trim().to_string())),
                    None => (heading.trim(), None),
                };
                blocks.push((name.to_string(), skeleton, String::new()))
            }
            None => match blocks.last_mut() {
                Some((_, _, body)) => {
                    body.push_str(line);
                    body.push('\n');
                }
                // 이름 줄보다 앞에 있는 내용은 이름 없는 첫 배열이다
                None => blocks.push((String::new(), None, format!("{line}\n"))),
            },
        }
    }
    // 첫 이름 줄보다 앞에 머리말만 있는 파일에서는 이름 없는 빈 블록이 생긴다
    blocks.retain(|(name, _, body)| !name.is_empty() || has_content(body));
    let mut named: Vec<NamedLayoutSet> = Vec::new();
    for (name, skeleton, body) in blocks {
        // 이름만 있고 내용이 없는 블록은 오타이므로 그냥 지나가지 않는다
        let mut layouts = parse_set(&body)?;
        if let Some(first) = named.first()
            && layouts.layers.len() < first.layouts.layers.len()
        {
            layouts
                .layers
                .extend_from_slice(&first.layouts.layers[layouts.layers.len()..]);
        }
        named.push(NamedLayoutSet {
            name,
            skeleton,
            layouts,
        });
    }
    if named.is_empty() {
        return Err("레이아웃에 행이 없음".to_string());
    }
    Ok(named)
}

/// 한 글자짜리 표기의 대문자 — 여러 글자를 한 번에 넣는 키(`.com`)는 대문자가 없다.
/// 글자 하나를 올리는 규칙 자체는 코어와 같아야 하므로 코어 것을 쓴다.
fn uppercase(characters: &str) -> String {
    let mut iterator = characters.chars();
    let (Some(character), None) = (iterator.next(), iterator.next()) else {
        return characters.to_string();
    };
    taza_engine::pack::layout::uppercase(character).to_string()
}

fn has_content(text: &str) -> bool {
    text.lines()
        .any(|line| !line.trim().is_empty() && !line.trim().starts_with('#'))
}

fn parse_set(text: &str) -> Result<KeyboardLayoutSet, String> {
    let mut layers = Vec::new();
    let mut rows = Vec::new();
    let mut panel_rows = 0.0f32;
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
                panel_rows: std::mem::take(&mut panel_rows),
            });
            continue;
        }
        if let Some(height) = line.strip_prefix("panel ") {
            panel_rows = height
                .trim()
                .strip_prefix('*')
                .and_then(|value| value.parse::<f32>().ok())
                .ok_or_else(|| format!("{}행: 패널 높이를 읽지 못했음", line_number + 1))?;
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
                    let width = width.parse::<f32>().map_err(|_| {
                        format!("{}행: 폭을 읽지 못했음: {token:?}", line_number + 1)
                    })?;
                    (specification, width)
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
                "layer3" => KeyAction::LayerSwitch { target: 3 },
                "blank" => KeyAction::Blank,
                // 세로줄로 이은 글자들은 이어 누를 때마다 갈린다 — 세로줄 자체가 키인
                // 경우와 겹치지 않도록 양쪽이 온전할 때만 구분자로 읽는다
                characters
                    if characters
                        .split('|')
                        .filter(|part| !part.is_empty())
                        .count()
                        > 1
                        && !characters.starts_with('|')
                        && !characters.ends_with('|') =>
                {
                    KeyAction::Multitap(
                        characters
                            .split('|')
                            .map(|part| {
                                let mut iterator = part.chars();
                                match (iterator.next(), iterator.next()) {
                                    (Some(character), None) => Ok(character),
                                    _ => Err(format!(
                                        "{}행: 멀티탭 글자는 1글자여야 함: {part:?}",
                                        line_number + 1
                                    )),
                                }
                            })
                            .collect::<Result<Vec<char>, String>>()?,
                    )
                }
                characters => {
                    let (base, shifted) = match characters.split_once(':') {
                        Some((base, shifted)) if !base.is_empty() && !shifted.is_empty() => {
                            (base.to_string(), shifted.to_string())
                        }
                        // 시프트 표기를 적지 않은 글자는 대문자로 올라간다. 대소문자가
                        // 없는 스크립트(한글 자모·숫자·기호)에서는 같은 글자 그대로다.
                        _ => (characters.to_string(), uppercase(characters)),
                    };
                    let (base, shifted) = (base.as_str(), shifted.as_str());
                    // 시프트 표기 없이 여러 글자면 한 번에 넣는 키다 (`.com`)
                    if base == shifted && base.chars().count() > 1 {
                        keys.push(LayoutKey {
                            action: KeyAction::Text(base.to_string()),
                            width_ratio,
                            alternates,
                        });
                        continue;
                    }
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
        layers.push(KeyboardLayout { rows, panel_rows });
    }
    if layers.is_empty() {
        return Err("레이아웃에 행이 없음".to_string());
    }
    Ok(KeyboardLayoutSet { layers })
}

/// 섹션 바이트 레이아웃은 `taza_engine::pack::layout` 참조.
pub fn serialize(named: &[NamedLayoutSet]) -> Vec<u8> {
    assert!(named.len() <= u8::MAX as usize);
    let mut output = vec![1u8, named.len() as u8];
    for entry in named {
        write_text(&entry.name, &mut output);
        write_text(entry.skeleton.as_deref().unwrap_or_default(), &mut output);
        serialize_set(&entry.layouts, &mut output);
    }
    output
}

fn write_text(text: &str, output: &mut Vec<u8>) {
    assert!(text.len() <= u8::MAX as usize);
    output.push(text.len() as u8);
    output.extend_from_slice(text.as_bytes());
}

fn serialize_set(layout_set: &KeyboardLayoutSet, output: &mut Vec<u8>) {
    assert!(layout_set.layers.len() <= u8::MAX as usize);
    output.push(layout_set.layers.len() as u8);
    for layer in &layout_set.layers {
        let panel_per_mille = (layer.panel_rows * 1000.0).round() as u16;
        output.extend_from_slice(&panel_per_mille.to_le_bytes());
        assert!(layer.rows.len() <= u8::MAX as usize);
        output.push(layer.rows.len() as u8);
        for row in &layer.rows {
            let height_per_mille = (row.height_ratio * 1000.0).round() as u16;
            output.extend_from_slice(&height_per_mille.to_le_bytes());
            assert!(row.keys.len() <= u8::MAX as usize);
            output.push(row.keys.len() as u8);
            for key in &row.keys {
                let (kind, base, shifted) = match &key.action {
                    KeyAction::Character { base, shifted } => (1u8, *base as u32, *shifted as u32),
                    KeyAction::Shift => (2, 0, 0),
                    KeyAction::Backspace => (3, 0, 0),
                    KeyAction::Space => (4, 0, 0),
                    KeyAction::Enter => (5, 0, 0),
                    KeyAction::LayerSwitch { target } => (6, *target as u32, 0),
                    KeyAction::LanguageSwitch => (7, 0, 0),
                    KeyAction::Text(_) => (8, 0, 0),
                    KeyAction::Blank => (9, 0, 0),
                    KeyAction::Multitap(_) => (10, 0, 0),
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
                match &key.action {
                    KeyAction::Text(text) => write_text(text, output),
                    KeyAction::Multitap(cycle) => {
                        write_text(&cycle.iter().collect::<String>(), output)
                    }
                    _ => {}
                }
            }
        }
    }
}
