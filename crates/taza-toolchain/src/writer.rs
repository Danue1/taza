use taza_engine::pack::{FORMAT_VERSION, MAGIC, SectionKind};

/// 팩 바이트 레이아웃은 `taza_engine::pack` 참조.
pub struct PackWriter {
    language: String,
    sections: Vec<(SectionKind, Vec<u8>)>,
}

impl PackWriter {
    pub fn new(language: impl Into<String>) -> Self {
        PackWriter {
            language: language.into(),
            sections: Vec::new(),
        }
    }

    pub fn add_section(&mut self, kind: SectionKind, bytes: Vec<u8>) {
        self.sections.push((kind, bytes));
    }

    pub fn finish(self) -> Vec<u8> {
        let language_bytes = self.language.as_bytes();
        assert!(
            language_bytes.len() <= u8::MAX as usize,
            "언어 태그가 너무 김"
        );

        let table_start = MAGIC.len() + 2 + 1 + language_bytes.len() + 2;
        let table_length = self.sections.len() * (4 + 8 + 8);
        let mut section_offset = (table_start + table_length) as u64;

        let mut output = Vec::new();
        output.extend_from_slice(MAGIC);
        output.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        output.push(language_bytes.len() as u8);
        output.extend_from_slice(language_bytes);
        output.extend_from_slice(&(self.sections.len() as u16).to_le_bytes());
        for (kind, bytes) in &self.sections {
            output.extend_from_slice(&kind.tag().to_le_bytes());
            output.extend_from_slice(&section_offset.to_le_bytes());
            output.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
            section_offset += bytes.len() as u64;
        }
        for (_, bytes) in &self.sections {
            output.extend_from_slice(bytes);
        }
        output
    }
}
