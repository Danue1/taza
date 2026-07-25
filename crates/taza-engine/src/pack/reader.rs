use crate::pack::layout::KeyboardLayoutSet;
use crate::pack::lexicon::Lexicon;
use crate::pack::metadata::Metadata;
use crate::pack::ngram::NgramModel;
use crate::pack::{FORMAT_VERSION, MAGIC, SectionKind};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackError {
    InvalidMagic,
    UnsupportedVersion(u16),
    Truncated,
}

impl fmt::Display for PackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackError::InvalidMagic => write!(formatter, "not a taza pack"),
            PackError::UnsupportedVersion(version) => {
                write!(formatter, "unsupported pack format version {version}")
            }
            PackError::Truncated => write!(formatter, "pack data is truncated or corrupted"),
        }
    }
}

impl std::error::Error for PackError {}

/// 바이트 슬라이스(통상 mmap) 위의 zero-copy 팩 뷰.
pub struct Pack<'bytes> {
    bytes: &'bytes [u8],
    language_range: (usize, usize),
    sections: Vec<(SectionKind, usize, usize)>,
}

impl fmt::Debug for Pack<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Pack")
            .field("language", &self.language())
            .field("sections", &self.sections.len())
            .finish()
    }
}

fn read_slice(bytes: &[u8], offset: usize, length: usize) -> Result<&[u8], PackError> {
    bytes
        .get(offset..offset + length)
        .ok_or(PackError::Truncated)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, PackError> {
    Ok(u16::from_le_bytes(
        read_slice(bytes, offset, 2)?.try_into().unwrap(),
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, PackError> {
    Ok(u32::from_le_bytes(
        read_slice(bytes, offset, 4)?.try_into().unwrap(),
    ))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, PackError> {
    Ok(u64::from_le_bytes(
        read_slice(bytes, offset, 8)?.try_into().unwrap(),
    ))
}

impl<'bytes> Pack<'bytes> {
    pub fn open(bytes: &'bytes [u8]) -> Result<Self, PackError> {
        if read_slice(bytes, 0, MAGIC.len())? != MAGIC {
            return Err(PackError::InvalidMagic);
        }
        let version = read_u16(bytes, 4)?;
        if version != FORMAT_VERSION {
            return Err(PackError::UnsupportedVersion(version));
        }
        let language_length = *read_slice(bytes, 6, 1)?.first().unwrap() as usize;
        let language_start = 7;
        std::str::from_utf8(read_slice(bytes, language_start, language_length)?)
            .map_err(|_| PackError::Truncated)?;

        let section_count_offset = language_start + language_length;
        let section_count = read_u16(bytes, section_count_offset)? as usize;
        let mut sections = Vec::new();
        let mut entry_offset = section_count_offset + 2;
        for _ in 0..section_count {
            let tag = read_u32(bytes, entry_offset)?;
            let offset = read_u64(bytes, entry_offset + 4)? as usize;
            let length = read_u64(bytes, entry_offset + 12)? as usize;
            read_slice(bytes, offset, length)?;
            // 미지 태그는 무시 — 구버전 리더가 신버전 팩을 읽기 위한 규칙
            if let Some(kind) = SectionKind::from_tag(tag) {
                sections.push((kind, offset, length));
            }
            entry_offset += 20;
        }

        Ok(Pack {
            bytes,
            language_range: (language_start, language_length),
            sections,
        })
    }

    pub fn language(&self) -> &'bytes str {
        let (start, length) = self.language_range;
        std::str::from_utf8(&self.bytes[start..start + length]).unwrap()
    }

    fn section(&self, kind: SectionKind) -> Option<&'bytes [u8]> {
        self.sections
            .iter()
            .find(|(section_kind, _, _)| *section_kind == kind)
            .map(|&(_, offset, length)| &self.bytes[offset..offset + length])
    }

    pub fn lexicon(&self) -> Option<Lexicon<'bytes>> {
        self.section(SectionKind::Lexicon).map(Lexicon::new)
    }

    /// 팩에 담긴 언어모델 뷰. 지금은 ngram-v1 하나 — 신경망 등 새 LM 섹션이 생기면
    /// 여기서 태그로 디스패치한다 (소비자는 이 메서드만 보므로 교체가 팩 배포로 끝난다).
    pub fn language_model(&self) -> Option<NgramModel<'bytes>> {
        self.section(SectionKind::NgramModel).map(NgramModel::new)
    }

    pub fn layout(&self) -> Option<KeyboardLayoutSet> {
        crate::pack::layout::deserialize(self.section(SectionKind::Layout)?)
    }

    /// 팩의 출처·라이선스·스크립트 특성. 고지 화면과 갱신 판단이 이 값을 읽는다.
    pub fn metadata(&self) -> Option<Metadata<'bytes>> {
        self.section(SectionKind::Metadata).map(Metadata::new)
    }
}
