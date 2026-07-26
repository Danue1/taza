//! 키보드 레이아웃 섹션. 레이아웃은 수십 키 규모라 mmap 조회 대신 로드 시 파싱한다
//! (mmap 원칙은 사전·LM처럼 큰 섹션에 적용).
//!
//! 레이어 관례: 0 = 문자(언어별), 1 = 심볼 1면(숫자·기본 기호), 2 = 심볼 2면,
//! 3 = 통합 검색면(이모지·기호·얼굴 문자 — 키 대신 패널이 자리를 갖는다).
//!
//! 한 언어가 배열을 여러 벌 실을 수 있다(두벌식·세벌식, QWERTY·Dvorak). 첫 배열이
//! 그 언어의 기본이고, 어느 것을 쓸지는 설정이 정한다.
//!
//! 와이어 레이아웃 (little-endian):
//! ```text
//! marker u8 | layout_count u8
//! 배열마다: name_length u8 | name UTF-8 × n
//!           | (marker=1일 때만) skeleton_length u8 | skeleton UTF-8 × n
//!           | layer_count u8
//! 레이어마다: panel_per_mille u16 | row_count u8
//!   행마다: height_per_mille u16 | key_count u8
//!     키마다: kind u8 | width_per_mille u16 | base u32 | shifted u32
//!             | alternate_count u8 | alternate u32 × n
//!             | (kind=8·10일 때만) text_length u8 | text UTF-8 × n
//! ```
//! kind: 1=Character, 2=Shift, 3=Backspace, 4=Space, 5=Enter,
//! 6=LayerSwitch(base=대상 레이어), 7=LanguageSwitch, 8=Text, 9=Blank, 10=Multitap.
//! base/shifted는 Character·LayerSwitch만 의미하고, alternate는 Character만 의미한다.
//!
//! 맨 앞의 marker는 그 뒤에 무엇이 오는지를 밝힌다: 0=배열 목록, 1=배열 목록 + 배열별
//! 골격 표기. 배열이 하나뿐이던 시절의 팩은 그 자리에 layer_count가 있었으므로 레이어가
//! 딱 하나인 옛 팩은 marker=1과 겹친다 — 표시 바이트만으로는 갈리지 않으니, 새 형식으로
//! 읽어 보고 섹션을 정확히 채웠을 때만 새 형식으로 인정한다(`deserialize`).

/// 글자 하나의 대문자. 대문자가 두 글자 이상이 되는 글자(ß→SS)는 키 한 칸에도 변형
/// 문자 팝업 한 칸에도 담을 수 없으므로 그대로 둔다 — 그런 글자는 배열이 시프트 표기를
/// 직접 적어야 한다.
pub fn uppercase(character: char) -> char {
    let mut upper = character.to_uppercase();
    match (upper.next(), upper.next()) {
        (Some(single), None) => single,
        _ => character,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    Character {
        base: char,
        shifted: char,
    },
    /// 한 번에 여러 글자를 넣는 키 (`.com` 등). 필드 성격이 불러오는 키가 주로 이것이라
    /// 히트 테스트의 이웃 확률에는 참여하지 않는다 — 어느 키인지가 이미 확실하다.
    Text(String),
    /// 이어 누를 때마다 글자가 갈리는 키 (천지인의 ㄱ→ㅋ→ㄲ). 무엇이 몇 번째인지는
    /// 배열 데이터가 정하고, 지금 몇 번째인지는 코어가 시간과 직전 키로 판정한다.
    Multitap(Vec<char>),
    Shift,
    Backspace,
    Space,
    Enter,
    LayerSwitch {
        target: u8,
    },
    /// 다음 언어로 전환. 언어 목록·순서는 셸이 소유하므로 코어는 요청만 낸다.
    LanguageSwitch,
    /// 자리만 차지하고 눌리지 않는 칸 — 숫자 패드 좌하단처럼 순정이 비워 두는 자리다.
    Blank,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutKey {
    pub action: KeyAction,
    pub width_ratio: f32,
    /// 길게 눌러 고르는 변형 문자 (é, ¿ 등). 순서가 팝업 표시 순서다.
    pub alternates: Vec<char>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutRow {
    pub keys: Vec<LayoutKey>,
    /// 표준 행 높이(폼팩터가 정한다) 대비 배수. 1.0이 보통 행이고, 0.8짜리 숫자행
    /// 같은 것을 팩 데이터만으로 끼워 넣을 수 있다.
    pub height_ratio: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KeyboardLayout {
    pub rows: Vec<LayoutRow>,
    /// 키 위에 놓이는 패널(통합 검색면)의 높이 — 표준 행 대비 배수. 0이면 키만 있는
    /// 보통 레이어다. 패널 안을 무엇으로 채우는지는 레이아웃이 아니라 코어가 정한다
    /// (검색어·최근 사용에 따라 달라지므로 배열 데이터에 담길 수 없다).
    pub panel_rows: f32,
}

/// 레이어 묶음 — 문자·심볼 등 전환 가능한 레이아웃들의 집합.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyboardLayoutSet {
    pub layers: Vec<KeyboardLayout>,
}

/// 이름이 붙은 배열 한 벌. 이름은 설정 화면에 그대로 나가므로 사람이 부르는 말이다
/// ("두벌식", "QWERTY").
#[derive(Debug, Clone, PartialEq)]
pub struct NamedLayoutSet {
    pub name: String,
    /// 이 배열이 요구하는 조합 골격의 태그. 대개 비어 있고(언어가 밝힌 골격을 쓴다),
    /// 같은 언어 안에서 조합 규칙이 다른 배열(천지인)만 자기 골격을 밝힌다.
    pub skeleton: Option<String>,
    pub layouts: KeyboardLayoutSet,
}

fn read_text<'bytes>(bytes: &'bytes [u8], offset: &mut usize) -> Option<&'bytes str> {
    let length = *bytes.get(*offset)? as usize;
    *offset += 1;
    let text = std::str::from_utf8(bytes.get(*offset..*offset + length)?).ok()?;
    *offset += length;
    Some(text)
}

/// 팩이 싣는 배열 전부. 첫 항목이 그 언어의 기본 배열이다.
pub fn deserialize(bytes: &[u8]) -> Option<Vec<NamedLayoutSet>> {
    // 표시 바이트만으로는 옛 형식과 갈리지 않는다: 레이어가 딱 하나인 옛 팩은 그 자리에
    // 1이 적혀 있어 `marker=1`(골격 표기가 붙은 새 형식)과 겹친다. 그래서 새 형식으로
    // 읽어 보고 **섹션을 정확히 채웠을 때만** 새 형식으로 인정한다 — 옛 팩을 새 형식으로
    // 읽으면 길이가 남거나 모자란다.
    if matches!(bytes.first(), Some(0 | 1))
        && let Some(layouts) = deserialize_named(bytes)
    {
        return Some(layouts);
    }
    // 배열이 하나뿐이던 시절의 팩 — 이름은 팩 메타데이터의 것을 쓰라는 뜻으로 비운다
    let mut offset = 0usize;
    Some(vec![NamedLayoutSet {
        name: String::new(),
        skeleton: None,
        layouts: deserialize_set(bytes, &mut offset)?,
    }])
}

/// 이름(과 골격)이 붙은 배열 목록. 섹션을 남김없이 쓰지 않으면 이 형식이 아니다.
fn deserialize_named(bytes: &[u8]) -> Option<Vec<NamedLayoutSet>> {
    let marker = *bytes.first()?;
    let mut offset = 1usize;
    let layout_count = *bytes.get(offset)? as usize;
    offset += 1;
    let mut layouts = Vec::with_capacity(layout_count);
    for _ in 0..layout_count {
        let name = read_text(bytes, &mut offset)?.to_string();
        let skeleton = match marker {
            1 => Some(read_text(bytes, &mut offset)?)
                .filter(|tag| !tag.is_empty())
                .map(str::to_string),
            _ => None,
        };
        layouts.push(NamedLayoutSet {
            name,
            skeleton,
            layouts: deserialize_set(bytes, &mut offset)?,
        });
    }
    (offset == bytes.len()).then_some(layouts)
}

/// 배열 한 벌을 읽고 `cursor`를 그 뒤로 옮긴다. 실패하면 커서는 옮기지 않는다 —
/// 어차피 섹션 전체를 버린다.
fn deserialize_set(bytes: &[u8], cursor: &mut usize) -> Option<KeyboardLayoutSet> {
    let mut offset = *cursor;
    let read_u8 = |offset: &mut usize| -> Option<u8> {
        let value = *bytes.get(*offset)?;
        *offset += 1;
        Some(value)
    };
    let layer_count = read_u8(&mut offset)? as usize;
    let mut layers = Vec::with_capacity(layer_count);
    for _ in 0..layer_count {
        let panel_per_mille =
            u16::from_le_bytes(bytes.get(offset..offset + 2)?.try_into().unwrap());
        offset += 2;
        let row_count = read_u8(&mut offset)? as usize;
        let mut rows = Vec::with_capacity(row_count);
        for _ in 0..row_count {
            let height_per_mille =
                u16::from_le_bytes(bytes.get(offset..offset + 2)?.try_into().unwrap());
            offset += 2;
            let key_count = read_u8(&mut offset)? as usize;
            let mut keys = Vec::with_capacity(key_count);
            for _ in 0..key_count {
                let kind = read_u8(&mut offset)?;
                let width_per_mille =
                    u16::from_le_bytes(bytes.get(offset..offset + 2)?.try_into().unwrap());
                offset += 2;
                let base = u32::from_le_bytes(bytes.get(offset..offset + 4)?.try_into().unwrap());
                offset += 4;
                let shifted =
                    u32::from_le_bytes(bytes.get(offset..offset + 4)?.try_into().unwrap());
                offset += 4;
                let alternate_count = read_u8(&mut offset)? as usize;
                let mut alternates = Vec::with_capacity(alternate_count);
                for _ in 0..alternate_count {
                    let code_point =
                        u32::from_le_bytes(bytes.get(offset..offset + 4)?.try_into().unwrap());
                    offset += 4;
                    alternates.push(char::from_u32(code_point)?);
                }
                let action = match kind {
                    1 => KeyAction::Character {
                        base: char::from_u32(base)?,
                        shifted: char::from_u32(shifted)?,
                    },
                    8 => KeyAction::Text(read_text(bytes, &mut offset)?.to_string()),
                    10 => KeyAction::Multitap(read_text(bytes, &mut offset)?.chars().collect()),
                    2 => KeyAction::Shift,
                    3 => KeyAction::Backspace,
                    4 => KeyAction::Space,
                    5 => KeyAction::Enter,
                    6 => KeyAction::LayerSwitch {
                        target: u8::try_from(base).ok()?,
                    },
                    7 => KeyAction::LanguageSwitch,
                    9 => KeyAction::Blank,
                    _ => return None,
                };
                keys.push(LayoutKey {
                    action,
                    width_ratio: width_per_mille as f32 / 1000.0,
                    alternates,
                });
            }
            rows.push(LayoutRow {
                keys,
                height_ratio: height_per_mille as f32 / 1000.0,
            });
        }
        layers.push(KeyboardLayout {
            rows,
            panel_rows: panel_per_mille as f32 / 1000.0,
        });
    }
    *cursor = offset;
    Some(KeyboardLayoutSet { layers })
}
