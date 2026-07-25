//! 키보드 레이아웃 섹션. 레이아웃은 수십 키 규모라 mmap 조회 대신 로드 시 파싱한다
//! (mmap 원칙은 사전·LM처럼 큰 섹션에 적용).
//!
//! 레이어 관례: 0 = 문자(언어별), 1 = 심볼 1면(숫자·기본 기호), 2 = 심볼 2면.
//!
//! 와이어 레이아웃 (little-endian):
//! ```text
//! layer_count u8
//! 레이어마다: row_count u8
//!   행마다: height_per_mille u16 | key_count u8
//!     키마다: kind u8 | width_per_mille u16 | base u32 | shifted u32
//!             | alternate_count u8 | alternate u32 × n
//! ```
//! kind: 1=Character, 2=Shift, 3=Backspace, 4=Space, 5=Enter,
//! 6=LayerSwitch(base=대상 레이어), 7=LanguageSwitch.
//! base/shifted는 Character·LayerSwitch만 의미하고, alternate는 Character만 의미한다.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    Character {
        base: char,
        shifted: char,
    },
    Shift,
    Backspace,
    Space,
    Enter,
    LayerSwitch {
        target: u8,
    },
    /// 다음 언어로 전환. 언어 목록·순서는 셸이 소유하므로 코어는 요청만 낸다.
    LanguageSwitch,
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
}

/// 레이어 묶음 — 문자·심볼 등 전환 가능한 레이아웃들의 집합.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyboardLayoutSet {
    pub layers: Vec<KeyboardLayout>,
}

pub fn deserialize(bytes: &[u8]) -> Option<KeyboardLayoutSet> {
    let mut offset = 0usize;
    let read_u8 = |offset: &mut usize| -> Option<u8> {
        let value = *bytes.get(*offset)?;
        *offset += 1;
        Some(value)
    };
    let layer_count = read_u8(&mut offset)? as usize;
    let mut layers = Vec::with_capacity(layer_count);
    for _ in 0..layer_count {
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
                    2 => KeyAction::Shift,
                    3 => KeyAction::Backspace,
                    4 => KeyAction::Space,
                    5 => KeyAction::Enter,
                    6 => KeyAction::LayerSwitch {
                        target: u8::try_from(base).ok()?,
                    },
                    7 => KeyAction::LanguageSwitch,
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
        layers.push(KeyboardLayout { rows });
    }
    Some(KeyboardLayoutSet { layers })
}
