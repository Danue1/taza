//! emoji 섹션 빌더. 섹션 바이트 레이아웃은 `taza_engine::pack::emoji` 참조.

use std::collections::BTreeMap;
use taza_engine::pack::emoji::MAX_PER_KEY;

#[derive(Default)]
pub struct EmojiBuilder {
    /// 키 오름차순으로 모은다 — 섹션이 이진 탐색되므로 정렬이 곧 형식의 일부다.
    entries: BTreeMap<String, Vec<String>>,
}

impl EmojiBuilder {
    pub fn new() -> Self {
        EmojiBuilder::default()
    }

    /// 낱말 하나에 이모지를 단다. 같은 이모지를 두 번 달지 않고, 상한을 넘으면 버린다.
    pub fn insert(&mut self, key: &str, emoji: &str) {
        let slot = self.entries.entry(key.to_string()).or_default();
        if slot.len() >= MAX_PER_KEY || slot.iter().any(|kept| kept == emoji) {
            return;
        }
        slot.push(emoji.to_string());
    }

    pub fn key_count(&self) -> usize {
        self.entries.len()
    }

    pub fn build(self) -> Vec<u8> {
        let count = self.entries.len();
        // 색인 자리를 먼저 비워 두고 본문을 채운 뒤 되돌아와 채운다
        let mut bytes = vec![0u8; 4 + count * 4];
        bytes[0..4].copy_from_slice(&(count as u32).to_le_bytes());
        for (index, (key, emoji)) in self.entries.iter().enumerate() {
            let offset = bytes.len() as u32;
            let slot = 4 + index * 4;
            bytes[slot..slot + 4].copy_from_slice(&offset.to_le_bytes());
            bytes.push(key.len() as u8);
            bytes.extend_from_slice(key.as_bytes());
            bytes.push(emoji.len() as u8);
            for one in emoji {
                bytes.push(one.len() as u8);
                bytes.extend_from_slice(one.as_bytes());
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
    fn table_of(builder: EmojiBuilder) -> Vec<u8> {
        let mut writer = PackWriter::new("ko");
        writer.add_section(SectionKind::Emoji, builder.build());
        writer.finish()
    }

    #[test]
    fn round_trips_through_the_section() {
        let mut builder = EmojiBuilder::new();
        builder.insert("dntda", "😀");
        builder.insert("dntda", "😄");
        builder.insert("dkfma", "😀");
        let bytes = table_of(builder);
        let pack = Pack::open(&bytes).unwrap();
        let table = pack.emoji().unwrap();
        assert_eq!(table.entry_count(), 2);
        assert_eq!(table.lookup("dntda"), vec!["😀", "😄"]);
        assert_eq!(table.lookup("dkfma"), vec!["😀"]);
        assert!(table.lookup("djqtsms").is_empty());
    }

    /// 같은 이모지를 두 번 달지 않고, 한 낱말에 붙는 수는 상한이 있다.
    #[test]
    fn duplicates_and_overflow_are_dropped() {
        let mut builder = EmojiBuilder::new();
        for emoji in ["😀", "😀", "😄", "😃", "😁", "😆"] {
            builder.insert("dntda", emoji);
        }
        let bytes = table_of(builder);
        let pack = Pack::open(&bytes).unwrap();
        assert_eq!(pack.emoji().unwrap().lookup("dntda").len(), MAX_PER_KEY);
    }

    /// 이모지를 싣지 않은 팩에서도 조회가 실패로 끝나지 않아야 한다.
    #[test]
    fn a_pack_without_emoji_has_no_table() {
        let mut writer = PackWriter::new("en");
        writer.add_section(SectionKind::Lexicon, Vec::new());
        let bytes = writer.finish();
        assert!(Pack::open(&bytes).unwrap().emoji().is_none());
    }
}
