//! annotation 섹션 — 낱말에 곁들이는 이모지·기호·얼굴 문자 표.
//!
//! 키는 lexicon과 같은 조회 키 공간에 있다(한국어면 자모 두벌식 ASCII). 그래야 지금
//! 치고 있는 어절을 그대로 들고 물어볼 수 있다.
//!
//! 섹션 레이아웃 (little-endian):
//! ```text
//! entry_count u32 | offset u32 × n | (key_length u8, key utf-8,
//!                                     item_count u8,
//!                                     (group u8, text_length u8, text utf-8) × m) × n
//! ```
//! group은 `CandidateGroup::tag()`이고, 낱말 갈래는 이 표에 설 자리가 없다 — 표에 담기는
//! 것은 낱말에 *곁들이는* 것뿐이다.
//!
//! 항목은 키 오름차순이고 offset은 섹션 시작 기준이다. 항목 길이가 가변이라 metadata처럼
//! 앞에서부터 세면 조회가 O(n)이 되는데, 이 표는 키를 칠 때마다 조회되므로 색인을 둔다.

use crate::contract::{CandidateGroup, EmojiCategory};

/// 한 낱말에 한 갈래로 달 수 있는 항목 수의 상한. "얼굴" 같은 일반적인 낱말에는 이모지가
/// 수백 개 걸리는데, 후보 바에 그만큼 내놓을 자리도 없고 담을 값어치도 없다.
pub const MAX_PER_GROUP: usize = 3;

/// 낱말에 달린 항목 하나.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Annotation<'bytes> {
    pub group: CandidateGroup,
    pub text: &'bytes str,
}

/// 바이트 슬라이스 위의 zero-copy 뷰. 손상된 데이터는 조회 실패로 처리한다.
pub struct AnnotationTable<'bytes> {
    bytes: &'bytes [u8],
}

impl<'bytes> AnnotationTable<'bytes> {
    pub(crate) fn new(bytes: &'bytes [u8]) -> Self {
        AnnotationTable { bytes }
    }

    pub fn entry_count(&self) -> usize {
        self.bytes
            .get(0..4)
            .map(|slice| u32::from_le_bytes(slice.try_into().unwrap()) as usize)
            .unwrap_or(0)
    }

    fn entry_offset(&self, index: usize) -> Option<usize> {
        let at = 4 + index * 4;
        let slice = self.bytes.get(at..at + 4)?;
        Some(u32::from_le_bytes(slice.try_into().unwrap()) as usize)
    }

    fn key_at(&self, index: usize) -> Option<&'bytes str> {
        let offset = self.entry_offset(index)?;
        let length = *self.bytes.get(offset)? as usize;
        std::str::from_utf8(self.bytes.get(offset + 1..offset + 1 + length)?).ok()
    }

    /// 이 낱말에 달린 항목들. 없으면 빈 목록이다. 순서는 팩에 담긴 순서 그대로이고
    /// 같은 갈래끼리 붙어 있다 — 빌더가 갈래별로 모아 담는다.
    pub fn lookup(&self, key: &str) -> Vec<Annotation<'bytes>> {
        let Some(index) = self.index_of(key) else {
            return Vec::new();
        };
        self.annotations_at(index).unwrap_or_default()
    }

    /// 이 낱말에서 한 갈래만. 후보 바는 갈래마다 자리를 따로 주므로 갈래를 나눠 묻는다.
    pub fn lookup_group(&self, key: &str, group: CandidateGroup) -> Vec<&'bytes str> {
        self.lookup(key)
            .into_iter()
            .filter(|annotation| annotation.group == group)
            .map(|annotation| annotation.text)
            .collect()
    }

    /// 낱말 접두로 훑어 갈래별 항목을 모은다 — 통합 검색이 쓰는 통로다. 검색은 "이 낱말로
    /// 부르는 것"을 찾는 일이므로 조회 키 공간에서 접두가 맞는 낱말을 모두 본다.
    pub fn search(&self, prefix: &str, limit: usize) -> Vec<Annotation<'bytes>> {
        let mut found: Vec<Annotation<'bytes>> = Vec::new();
        for index in self.first_index_with(prefix)..self.entry_count() {
            let Some(key) = self.key_at(index) else { break };
            if !key.starts_with(prefix) {
                break;
            }
            for annotation in self.annotations_at(index).unwrap_or_default() {
                if found.iter().any(|kept| kept.text == annotation.text) {
                    continue;
                }
                found.push(annotation);
                if found.len() >= limit {
                    return found;
                }
            }
        }
        found
    }

    /// 접두가 맞는 첫 항목의 자리. 없으면 그 자리에 들어갈 위치이므로, 훑기는 곧 끝난다.
    fn first_index_with(&self, prefix: &str) -> usize {
        let mut low = 0usize;
        let mut high = self.entry_count();
        while low < high {
            let middle = (low + high) / 2;
            match self.key_at(middle) {
                Some(key) if key < prefix => low = middle + 1,
                Some(_) => high = middle,
                None => return self.entry_count(),
            }
        }
        low
    }

    fn index_of(&self, key: &str) -> Option<usize> {
        let mut low = 0usize;
        let mut high = self.entry_count();
        while low < high {
            let middle = (low + high) / 2;
            match self.key_at(middle)?.cmp(key) {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
                std::cmp::Ordering::Equal => return Some(middle),
            }
        }
        None
    }

    fn annotations_at(&self, index: usize) -> Option<Vec<Annotation<'bytes>>> {
        let offset = self.entry_offset(index)?;
        let key_length = *self.bytes.get(offset)? as usize;
        let mut at = offset + 1 + key_length;
        let count = *self.bytes.get(at)? as usize;
        at += 1;
        let mut annotations = Vec::with_capacity(count);
        for _ in 0..count {
            let group = CandidateGroup::from_tag(*self.bytes.get(at)?)?;
            at += 1;
            let length = *self.bytes.get(at)? as usize;
            at += 1;
            let text = std::str::from_utf8(self.bytes.get(at..at + length)?).ok()?;
            at += length;
            annotations.push(Annotation { group, text });
        }
        Some(annotations)
    }
}

/// catalog 섹션 — 검색하지 않았을 때 보이는 묶음들.
///
/// 낱말→항목 표만으로는 "무엇을 먼저 보일지"를 알 수 없다(그 표는 조회 키 순서라 사람이
/// 기대하는 순서가 아니다). 그래서 묶음을 순서대로 따로 싣는다. 이모지는 빌트인 키보드와
/// 같은 묶음(스마일리·동물·음식…)으로 나뉘고, 기호·얼굴 문자는 묶음 없이 한 덩이다.
///
/// 섹션 레이아웃 (little-endian):
/// ```text
/// section_count u8 | 묶음마다: group u8, category u8(0=없음), item_count u16,
///                              (text_length u8, text utf-8) × n
/// ```
pub struct AnnotationCatalog<'bytes> {
    bytes: &'bytes [u8],
}

/// 카탈로그에 실린 묶음 하나.
pub struct CatalogSection<'bytes> {
    pub group: CandidateGroup,
    /// 이모지 묶음이면 그 자리, 아니면 없음
    pub category: Option<EmojiCategory>,
    pub items: Vec<&'bytes str>,
}

impl<'bytes> AnnotationCatalog<'bytes> {
    pub(crate) fn new(bytes: &'bytes [u8]) -> Self {
        AnnotationCatalog { bytes }
    }

    /// 실려 있는 순서 그대로의 묶음들. 묶음마다 최대 `limit`개까지 읽는다.
    pub fn sections(&self, limit: usize) -> Vec<CatalogSection<'bytes>> {
        self.read(limit).unwrap_or_default()
    }

    fn read(&self, limit: usize) -> Option<Vec<CatalogSection<'bytes>>> {
        let mut at = 0usize;
        let section_count = *self.bytes.get(at)? as usize;
        at += 1;
        let mut sections = Vec::with_capacity(section_count);
        for _ in 0..section_count {
            let group = CandidateGroup::from_tag(*self.bytes.get(at)?)?;
            at += 1;
            let category = EmojiCategory::from_tag(*self.bytes.get(at)?);
            at += 1;
            let count =
                u16::from_le_bytes(self.bytes.get(at..at + 2)?.try_into().unwrap()) as usize;
            at += 2;
            let mut items = Vec::new();
            for _ in 0..count {
                let length = *self.bytes.get(at)? as usize;
                at += 1;
                let text = std::str::from_utf8(self.bytes.get(at..at + length)?).ok()?;
                at += length;
                if items.len() < limit {
                    items.push(text);
                }
            }
            sections.push(CatalogSection {
                group,
                category,
                items,
            });
        }
        Some(sections)
    }
}
