//! annotation 섹션 빌더. 섹션 바이트 레이아웃은 `taza_engine::pack::annotation` 참조.

use std::collections::BTreeMap;
use taza_engine::contract::{CandidateGroup, EmojiCategory};
use taza_engine::pack::annotation::MAX_PER_GROUP;

#[derive(Default)]
pub struct AnnotationBuilder {
    /// 키 오름차순으로 모은다 — 섹션이 이진 탐색되므로 정렬이 곧 형식의 일부다.
    /// 한 키의 항목은 갈래 순서대로 모아 담는다 — 리더가 갈래별로 꺼내 쓴다.
    entries: BTreeMap<String, Vec<(CandidateGroup, String)>>,
}

impl AnnotationBuilder {
    pub fn new() -> Self {
        AnnotationBuilder::default()
    }

    /// 낱말 하나에 곁들일 것을 단다. 같은 것을 두 번 달지 않고, 갈래마다 상한을 둔다.
    /// 낱말 갈래는 이 표에 담기지 않으므로 조용히 버린다.
    pub fn insert(&mut self, key: &str, group: CandidateGroup, text: &str) {
        if group.tag().is_none() {
            return;
        }
        let slot = self.entries.entry(key.to_string()).or_default();
        let kept_in_group = slot.iter().filter(|(kept, _)| *kept == group).count();
        if kept_in_group >= MAX_PER_GROUP || slot.iter().any(|(_, kept)| kept == text) {
            return;
        }
        slot.push((group, text.to_string()));
        slot.sort_by_key(|(group, _)| group.tag());
    }

    pub fn key_count(&self) -> usize {
        self.entries.len()
    }

    pub fn build(self) -> Vec<u8> {
        let count = self.entries.len();
        // 색인 자리를 먼저 비워 두고 본문을 채운 뒤 되돌아와 채운다
        let mut bytes = vec![0u8; 4 + count * 4];
        bytes[0..4].copy_from_slice(&(count as u32).to_le_bytes());
        for (index, (key, annotations)) in self.entries.iter().enumerate() {
            let offset = bytes.len() as u32;
            let slot = 4 + index * 4;
            bytes[slot..slot + 4].copy_from_slice(&offset.to_le_bytes());
            bytes.push(key.len() as u8);
            bytes.extend_from_slice(key.as_bytes());
            bytes.push(annotations.len() as u8);
            for (group, text) in annotations {
                bytes.push(group.tag().unwrap_or_default());
                bytes.push(text.len() as u8);
                bytes.extend_from_slice(text.as_bytes());
            }
        }
        bytes
    }
}

/// 검색면 묶음 목록 빌더 — 넣은 순서를 그대로 지킨다. 섹션 바이트 레이아웃은
/// `taza_engine::pack::annotation::AnnotationCatalog` 참조.
#[derive(Default)]
pub struct AnnotationCatalogBuilder {
    /// 묶음마다 (갈래, 이모지 묶음, 항목들). 묶음 순서와 항목 순서 모두 들어온 순서다.
    sections: Vec<(CandidateGroup, Option<EmojiCategory>, Vec<String>)>,
}

impl AnnotationCatalogBuilder {
    pub fn new() -> Self {
        AnnotationCatalogBuilder::default()
    }

    /// 묶음 끝에 하나 붙인다. 같은 묶음에 이미 있는 것은 앞선 자리를 지킨다 — 원천에서
    /// 처음 나온 자리가 그 항목의 차례다.
    pub fn insert(&mut self, group: CandidateGroup, category: Option<EmojiCategory>, text: &str) {
        if group.tag().is_none() {
            return;
        }
        let slot = match self
            .sections
            .iter_mut()
            .find(|(kept, kept_category, _)| *kept == group && *kept_category == category)
        {
            Some((_, _, items)) => items,
            None => {
                self.sections.push((group, category, Vec::new()));
                &mut self.sections.last_mut().unwrap().2
            }
        };
        if slot.iter().any(|kept| kept == text) {
            return;
        }
        slot.push(text.to_string());
    }

    pub fn item_count(&self) -> usize {
        self.sections.iter().map(|(_, _, items)| items.len()).sum()
    }

    pub fn build(self) -> Vec<u8> {
        let mut bytes = vec![self.sections.len() as u8];
        for (group, category, items) in &self.sections {
            bytes.push(group.tag().unwrap_or_default());
            bytes.push(category.map_or(0, EmojiCategory::tag));
            bytes.extend_from_slice(&(items.len() as u16).to_le_bytes());
            for text in items {
                bytes.push(text.len() as u8);
                bytes.extend_from_slice(text.as_bytes());
            }
        }
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PackWriter;
    use taza_engine::pack::{Pack, SectionKind};

    /// 섹션은 팩을 거쳐 읽는다 — 빌더가 낸 바이트를 리더가 그대로 이해하는지가 계약이다.
    fn table_of(builder: AnnotationBuilder) -> Vec<u8> {
        let mut writer = PackWriter::new("ko");
        writer.add_section(SectionKind::Annotation, builder.build());
        writer.finish()
    }

    #[test]
    fn round_trips_through_the_section() {
        let mut builder = AnnotationBuilder::new();
        builder.insert("dntda", CandidateGroup::Emoji, "😀");
        builder.insert("dntda", CandidateGroup::Emoji, "😄");
        builder.insert("dkfma", CandidateGroup::Emoji, "😀");
        let bytes = table_of(builder);
        let pack = Pack::open(&bytes).unwrap();
        let table = pack.annotations().unwrap();
        assert_eq!(table.entry_count(), 2);
        assert_eq!(
            table.lookup_group("dntda", CandidateGroup::Emoji),
            vec!["😀", "😄"]
        );
        assert_eq!(
            table.lookup_group("dkfma", CandidateGroup::Emoji),
            vec!["😀"]
        );
        assert!(table.lookup("djqtsms").is_empty());
    }

    /// 한 낱말에 갈래가 여럿 달리면 갈래 순서대로 담기고, 갈래별로 꺼낼 수 있다.
    #[test]
    fn groups_are_kept_apart() {
        let mut builder = AnnotationBuilder::new();
        builder.insert("dntda", CandidateGroup::Emoticon, "(^_^)");
        builder.insert("dntda", CandidateGroup::Emoji, "😀");
        builder.insert("dntda", CandidateGroup::Symbol, "☺");
        let bytes = table_of(builder);
        let pack = Pack::open(&bytes).unwrap();
        let table = pack.annotations().unwrap();
        assert_eq!(
            table
                .lookup("dntda")
                .into_iter()
                .map(|annotation| annotation.text)
                .collect::<Vec<_>>(),
            vec!["😀", "☺", "(^_^)"]
        );
        assert_eq!(
            table.lookup_group("dntda", CandidateGroup::Emoticon),
            vec!["(^_^)"]
        );
    }

    /// 같은 것을 두 번 달지 않고, 한 낱말에 한 갈래로 붙는 수는 상한이 있다.
    #[test]
    fn duplicates_and_overflow_are_dropped() {
        let mut builder = AnnotationBuilder::new();
        for emoji in ["😀", "😀", "😄", "😃", "😁", "😆"] {
            builder.insert("dntda", CandidateGroup::Emoji, emoji);
        }
        let bytes = table_of(builder);
        let pack = Pack::open(&bytes).unwrap();
        assert_eq!(
            pack.annotations()
                .unwrap()
                .lookup_group("dntda", CandidateGroup::Emoji)
                .len(),
            MAX_PER_GROUP
        );
    }

    /// 낱말로 훑어 찾는다 — 통합 검색은 접두가 맞는 낱말의 항목을 모두 본다.
    #[test]
    fn searches_by_word_prefix() {
        let mut builder = AnnotationBuilder::new();
        builder.insert("dntdaa", CandidateGroup::Emoji, "😀");
        builder.insert("dntdb", CandidateGroup::Emoji, "😄");
        builder.insert("ekfa", CandidateGroup::Emoji, "🌙");
        let bytes = table_of(builder);
        let pack = Pack::open(&bytes).unwrap();
        let table = pack.annotations().unwrap();
        assert_eq!(
            table
                .search("dntd", 8)
                .into_iter()
                .map(|annotation| annotation.text)
                .collect::<Vec<_>>(),
            vec!["😀", "😄"]
        );
        assert!(table.search("wjs", 8).is_empty());
    }

    /// 카탈로그는 넣은 순서와 묶음을 그대로 지킨다.
    #[test]
    fn the_catalog_keeps_the_source_order() {
        let mut catalog = AnnotationCatalogBuilder::new();
        for emoji in ["😀", "😄", "😀"] {
            catalog.insert(
                CandidateGroup::Emoji,
                Some(EmojiCategory::SmileysAndPeople),
                emoji,
            );
        }
        catalog.insert(CandidateGroup::Emoji, Some(EmojiCategory::Flags), "🇰🇷");
        catalog.insert(CandidateGroup::Symbol, None, "★");
        let mut writer = PackWriter::new("ko");
        writer.add_section(SectionKind::AnnotationCatalog, catalog.build());
        let bytes = writer.finish();
        let pack = Pack::open(&bytes).unwrap();
        let catalog = pack.annotation_catalog().unwrap();
        let sections = catalog.sections(8);
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0].category, Some(EmojiCategory::SmileysAndPeople));
        assert_eq!(sections[0].items, vec!["😀", "😄"]);
        assert_eq!(sections[1].category, Some(EmojiCategory::Flags));
        assert_eq!(sections[2].group, CandidateGroup::Symbol);
        assert_eq!(sections[2].items, vec!["★"]);
        // 상한은 묶음을 앞에서부터 자른다
        assert_eq!(catalog.sections(1)[0].items, vec!["😀"]);
    }

    /// 곁들일 것을 싣지 않은 팩에서도 조회가 실패로 끝나지 않아야 한다.
    #[test]
    fn a_pack_without_annotations_has_no_table() {
        let mut writer = PackWriter::new("en");
        writer.add_section(SectionKind::Lexicon, Vec::new());
        let bytes = writer.finish();
        assert!(Pack::open(&bytes).unwrap().annotations().is_none());
    }
}
