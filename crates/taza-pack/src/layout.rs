//! 키보드 레이아웃 섹션. 레이아웃은 수십 키 규모라 mmap 조회 대신 로드 시 파싱한다
//! (mmap 원칙은 사전·LM처럼 큰 섹션에 적용).
//!
//! 와이어 레이아웃 (little-endian):
//! ```text
//! row_count u8
//! 행마다: key_count u8, 키마다: kind u8 | width_per_mille u16 | base u32 | shifted u32
//! ```
//! kind: 1=Character, 2=Shift, 3=Backspace, 4=Space, 5=Enter (base/shifted는 Character만 의미)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    Character { base: char, shifted: char },
    Shift,
    Backspace,
    Space,
    Enter,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutKey {
    pub action: KeyAction,
    pub width_ratio: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutRow {
    pub keys: Vec<LayoutKey>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KeyboardLayout {
    pub rows: Vec<LayoutRow>,
}

pub fn serialize(layout: &KeyboardLayout) -> Vec<u8> {
    assert!(layout.rows.len() <= u8::MAX as usize);
    let mut output = vec![layout.rows.len() as u8];
    for row in &layout.rows {
        assert!(row.keys.len() <= u8::MAX as usize);
        output.push(row.keys.len() as u8);
        for key in &row.keys {
            let (kind, base, shifted) = match key.action {
                KeyAction::Character { base, shifted } => (1u8, base as u32, shifted as u32),
                KeyAction::Shift => (2, 0, 0),
                KeyAction::Backspace => (3, 0, 0),
                KeyAction::Space => (4, 0, 0),
                KeyAction::Enter => (5, 0, 0),
            };
            output.push(kind);
            let width_per_mille = (key.width_ratio * 1000.0).round() as u16;
            output.extend_from_slice(&width_per_mille.to_le_bytes());
            output.extend_from_slice(&base.to_le_bytes());
            output.extend_from_slice(&shifted.to_le_bytes());
        }
    }
    output
}

pub fn deserialize(bytes: &[u8]) -> Option<KeyboardLayout> {
    let mut offset = 0usize;
    let read_u8 = |offset: &mut usize| -> Option<u8> {
        let value = *bytes.get(*offset)?;
        *offset += 1;
        Some(value)
    };
    let row_count = read_u8(&mut offset)? as usize;
    let mut rows = Vec::with_capacity(row_count);
    for _ in 0..row_count {
        let key_count = read_u8(&mut offset)? as usize;
        let mut keys = Vec::with_capacity(key_count);
        for _ in 0..key_count {
            let kind = read_u8(&mut offset)?;
            let width_per_mille =
                u16::from_le_bytes(bytes.get(offset..offset + 2)?.try_into().unwrap());
            offset += 2;
            let base = u32::from_le_bytes(bytes.get(offset..offset + 4)?.try_into().unwrap());
            offset += 4;
            let shifted = u32::from_le_bytes(bytes.get(offset..offset + 4)?.try_into().unwrap());
            offset += 4;
            let action = match kind {
                1 => KeyAction::Character {
                    base: char::from_u32(base)?,
                    shifted: char::from_u32(shifted)?,
                },
                2 => KeyAction::Shift,
                3 => KeyAction::Backspace,
                4 => KeyAction::Space,
                5 => KeyAction::Enter,
                _ => return None,
            };
            keys.push(LayoutKey {
                action,
                width_ratio: width_per_mille as f32 / 1000.0,
            });
        }
        rows.push(LayoutRow { keys });
    }
    Some(KeyboardLayout { rows })
}
