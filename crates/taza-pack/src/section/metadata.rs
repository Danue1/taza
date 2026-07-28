use std::collections::BTreeMap;

/// 섹션 바이트 레이아웃은 `taza_engine::pack::metadata` 참조.
#[derive(Default)]
pub struct MetadataBuilder {
    entries: BTreeMap<String, String>,
}

impl MetadataBuilder {
    pub fn new() -> Self {
        MetadataBuilder::default()
    }

    /// 같은 키를 다시 넣으면 마지막 값이 남는다. 예약 키는
    /// `taza_engine::pack::metadata::keys` 참조.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = value.into();
        assert!(
            key.len() <= u8::MAX as usize,
            "메타데이터 키가 너무 김: {key}"
        );
        assert!(
            value.len() <= u16::MAX as usize,
            "메타데이터 값이 너무 김: {key}"
        );
        self.entries.insert(key, value);
    }

    pub fn build(self) -> Vec<u8> {
        let mut output = Vec::new();
        output.extend_from_slice(&(self.entries.len() as u16).to_le_bytes());
        for (key, value) in &self.entries {
            output.push(key.len() as u8);
            output.extend_from_slice(key.as_bytes());
            output.extend_from_slice(&(value.len() as u16).to_le_bytes());
            output.extend_from_slice(value.as_bytes());
        }
        output
    }
}
