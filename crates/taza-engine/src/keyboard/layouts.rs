//! 내장 레이아웃. 장기적으로는 언어팩 데이터로 옮겨 코드 수정 없이 배열을 추가한다.
//! 심볼 1·2면과 길게 눌러 나오는 변형 문자는 iOS 순정 배열을 따른다
//! (빌트인 UX 계승 원칙).

use super::{KeyAction, KeyboardLayout, KeyboardLayoutSet, LayoutKey, LayoutRow};

/// 내장 배열은 모두 표준 높이 행이다 — 높이가 다른 행(숫자행 등)은 팩 데이터로 온다.
const STANDARD_ROW_HEIGHT: f32 = 1.0;
const LETTER_WIDTH: f32 = 0.1;
const CONTROL_WIDTH: f32 = 0.15;
/// 하단 행: 심볼 · 이모지 · 언어 · 스페이스 · 엔터
const LAYER_KEY_WIDTH: f32 = 0.115;
const PANEL_KEY_WIDTH: f32 = 0.115;
const LANGUAGE_KEY_WIDTH: f32 = 0.115;
const SPACE_WIDTH: f32 = 0.375;
const ENTER_WIDTH: f32 = 0.28;
/// 통합 검색면 하단 행: 문자 복귀 · (빈자리) · 삭제. 순정 이모지 화면처럼 낱말을 치는
/// 키(스페이스·엔터)는 두지 않는다 — 이 면에서 하는 일은 고르는 것뿐이다.
const PANEL_RETURN_WIDTH: f32 = 0.2;
const PANEL_GAP_WIDTH: f32 = 0.6;
const PANEL_BACKSPACE_WIDTH: f32 = 0.2;
/// 통합 검색면 패널이 차지하는 높이 — 표준 행 셋. 하단 행까지 더하면 문자면과 같은 높이가
/// 되어 레이어를 넘나들 때 키보드가 커지거나 작아지지 않는다.
const PANEL_ROWS: f32 = 3.0;
/// 통합 검색면 레이어 번호 — 팩 데이터의 `layer3`과 같은 자리다.
const PANEL_LAYER: u8 = 3;
/// 심볼면 셋째 행 — 순정은 양끝(#+=·⌫)을 가장자리에 붙이고 가운데 5키를 넓게 편다
const SYMBOL_PUNCTUATION_WIDTH: f32 = 0.14;

/// 길게 눌러 고르는 변형 문자. 없는 글자는 표에 넣지 않는다.
const LATIN_ALTERNATES: &[(char, &str)] = &[
    ('a', "àáâäæãåā"),
    ('c', "çćč"),
    ('e', "èéêëēėę"),
    ('i', "îïíīįì"),
    ('l', "ł"),
    ('n', "ñń"),
    ('o', "ôöòóœøōõ"),
    ('s', "ßśš"),
    ('u', "ûüùúū"),
    ('y', "ÿ"),
    ('z', "žźż"),
];

const SYMBOL_ALTERNATES: &[(char, &str)] = &[
    ('0', "°"),
    ('-', "–—•"),
    ('/', "\\"),
    ('$', "₩¢£¥€"),
    ('&', "§"),
    ('"', "\u{201C}\u{201D}«»„"),
    ('\'', "\u{2018}\u{2019}`"),
    ('(', "[{<"),
    (')', "]}>"),
    ('.', "…"),
    ('?', "¿"),
    ('!', "¡"),
    ('%', "‰"),
    ('=', "≠≈"),
];

fn alternates_for(table: &[(char, &str)], base: char) -> Vec<char> {
    table
        .iter()
        .find(|(key, _)| *key == base)
        .map(|(_, characters)| characters.chars().collect())
        .unwrap_or_default()
}

fn character_key(base: char, shifted: char, alternates: Vec<char>) -> LayoutKey {
    LayoutKey {
        action: KeyAction::Character { base, shifted },
        width_ratio: LETTER_WIDTH,
        row_span: 1,
        alternates,
    }
}

fn control_key(action: KeyAction, width_ratio: f32) -> LayoutKey {
    LayoutKey {
        action,
        width_ratio,
        row_span: 1,
        alternates: Vec::new(),
    }
}

fn row(keys: Vec<LayoutKey>) -> LayoutRow {
    LayoutRow {
        keys,
        height_ratio: STANDARD_ROW_HEIGHT,
    }
}

fn latin_row(letters: &str) -> LayoutRow {
    row(letters
        .chars()
        .map(|base| {
            character_key(
                base,
                base.to_ascii_uppercase(),
                alternates_for(LATIN_ALTERNATES, base),
            )
        })
        .collect())
}

/// 한글 자모는 shift로 나오는 짝(ㄱ→ㄲ, ㅐ→ㅒ)이 곧 길게 눌러 고를 변형이다 — shift를
/// 켜지 않고도 된소리·이중모음에 닿는 길을 준다.
fn hangul_row(pairs: &[(char, char)]) -> LayoutRow {
    row(pairs
        .iter()
        .map(|&(base, shifted)| {
            let alternates = if shifted == base {
                Vec::new()
            } else {
                vec![shifted]
            };
            character_key(base, shifted, alternates)
        })
        .collect())
}

fn symbol_row(characters: &str) -> LayoutRow {
    symbol_row_with_width(characters, LETTER_WIDTH)
}

fn symbol_row_with_width(characters: &str, width_ratio: f32) -> LayoutRow {
    row(characters
        .chars()
        .map(|base| {
            let mut key = character_key(base, base, alternates_for(SYMBOL_ALTERNATES, base));
            key.width_ratio = width_ratio;
            key
        })
        .collect())
}

/// 하단 행 — 순서는 심볼 · 이모지 · 언어 · 스페이스 · 엔터로 고정한다. 이모지 키는 낱말을
/// 치지 않고도 통합 검색면(이모지·기호·얼굴 문자)에 한 번에 닿는 길이다.
fn bottom_row(switch_target: u8) -> LayoutRow {
    row(vec![
        control_key(
            KeyAction::LayerSwitch {
                target: switch_target,
            },
            LAYER_KEY_WIDTH,
        ),
        control_key(
            KeyAction::LayerSwitch {
                target: PANEL_LAYER,
            },
            PANEL_KEY_WIDTH,
        ),
        control_key(KeyAction::LanguageSwitch, LANGUAGE_KEY_WIDTH),
        control_key(KeyAction::Space, SPACE_WIDTH),
        control_key(KeyAction::Enter, ENTER_WIDTH),
    ])
}

fn shift_row(keys: Vec<LayoutKey>) -> LayoutRow {
    let mut all = vec![control_key(KeyAction::Shift, CONTROL_WIDTH)];
    all.extend(keys);
    all.push(control_key(KeyAction::Backspace, CONTROL_WIDTH));
    row(all)
}

/// iOS 순정 심볼 1면: 숫자·기본 기호. 셋째 행 왼쪽이 #+= 진입.
fn symbols_first_layer() -> KeyboardLayout {
    let mut third = vec![control_key(
        KeyAction::LayerSwitch { target: 2 },
        CONTROL_WIDTH,
    )];
    third.extend(symbol_row_with_width(".,?!'", SYMBOL_PUNCTUATION_WIDTH).keys);
    third.push(control_key(KeyAction::Backspace, CONTROL_WIDTH));
    KeyboardLayout {
        panel_rows: 0.0,
        rows: vec![
            symbol_row("1234567890"),
            symbol_row("-/:;()$&@\""),
            row(third),
            bottom_row(0),
        ],
    }
}

/// iOS 순정 심볼 2면. 셋째 행 왼쪽이 123 복귀.
fn symbols_second_layer() -> KeyboardLayout {
    let mut third = vec![control_key(
        KeyAction::LayerSwitch { target: 1 },
        CONTROL_WIDTH,
    )];
    third.extend(symbol_row_with_width(".,?!'", SYMBOL_PUNCTUATION_WIDTH).keys);
    third.push(control_key(KeyAction::Backspace, CONTROL_WIDTH));
    KeyboardLayout {
        panel_rows: 0.0,
        rows: vec![
            symbol_row("[]{}#%^*+="),
            symbol_row("_\\|~<>€£¥·"),
            row(third),
            bottom_row(0),
        ],
    }
}

/// 통합 검색면 — 키는 하단 행 하나뿐이고 나머지 자리는 패널이 갖는다. 패널을 무엇으로
/// 채우는지는 검색어·최근 사용에 따라 달라지므로 배열이 아니라 코어가 정한다.
fn annotation_panel_layer() -> KeyboardLayout {
    KeyboardLayout {
        rows: vec![row(vec![
            control_key(KeyAction::LayerSwitch { target: 0 }, PANEL_RETURN_WIDTH),
            control_key(KeyAction::Blank, PANEL_GAP_WIDTH),
            control_key(KeyAction::Backspace, PANEL_BACKSPACE_WIDTH),
        ])],
        panel_rows: PANEL_ROWS,
    }
}

fn with_symbol_layers(letters: KeyboardLayout) -> KeyboardLayoutSet {
    KeyboardLayoutSet {
        layers: vec![
            letters,
            symbols_first_layer(),
            symbols_second_layer(),
            annotation_panel_layer(),
        ],
    }
}

pub fn qwerty() -> KeyboardLayoutSet {
    with_symbol_layers(KeyboardLayout {
        panel_rows: 0.0,
        rows: vec![
            latin_row("qwertyuiop"),
            latin_row("asdfghjkl"),
            shift_row(latin_row("zxcvbnm").keys),
            bottom_row(1),
        ],
    })
}

pub fn dubeolsik() -> KeyboardLayoutSet {
    with_symbol_layers(KeyboardLayout {
        panel_rows: 0.0,
        rows: vec![
            hangul_row(&[
                ('ㅂ', 'ㅃ'),
                ('ㅈ', 'ㅉ'),
                ('ㄷ', 'ㄸ'),
                ('ㄱ', 'ㄲ'),
                ('ㅅ', 'ㅆ'),
                ('ㅛ', 'ㅛ'),
                ('ㅕ', 'ㅕ'),
                ('ㅑ', 'ㅑ'),
                ('ㅐ', 'ㅒ'),
                ('ㅔ', 'ㅖ'),
            ]),
            hangul_row(&[
                ('ㅁ', 'ㅁ'),
                ('ㄴ', 'ㄴ'),
                ('ㅇ', 'ㅇ'),
                ('ㄹ', 'ㄹ'),
                ('ㅎ', 'ㅎ'),
                ('ㅗ', 'ㅗ'),
                ('ㅓ', 'ㅓ'),
                ('ㅏ', 'ㅏ'),
                ('ㅣ', 'ㅣ'),
            ]),
            shift_row(
                hangul_row(&[
                    ('ㅋ', 'ㅋ'),
                    ('ㅌ', 'ㅌ'),
                    ('ㅊ', 'ㅊ'),
                    ('ㅍ', 'ㅍ'),
                    ('ㅠ', 'ㅠ'),
                    ('ㅜ', 'ㅜ'),
                    ('ㅡ', 'ㅡ'),
                ])
                .keys,
            ),
            bottom_row(1),
        ],
    })
}
