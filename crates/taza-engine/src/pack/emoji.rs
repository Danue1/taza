//! emoji 섹션 — 낱말을 이모지로 옮기는 표.
//!
//! 키는 lexicon과 같은 조회 키 공간에 있다(한국어면 자모 두벌식 ASCII). 그래야 지금
//! 치고 있는 어절을 그대로 들고 물어볼 수 있다.
//!
//! 섹션 레이아웃 (little-endian):
//! ```text
//! entry_count u32 | offset u32 × n | (key_length u8, key utf-8,
//!                                     emoji_count u8, (emoji_length u8, emoji utf-8) × m) × n
//! ```
//! 항목은 키 오름차순이고 offset은 섹션 시작 기준이다. 항목 길이가 가변이라 metadata처럼
//! 앞에서부터 세면 조회가 O(n)이 되는데, 이 표는 키를 칠 때마다 조회되므로 색인을 둔다.

/// 한 낱말에 달 수 있는 이모지 수의 상한. "얼굴" 같은 일반적인 낱말에는 수백 개가
/// 걸리는데, 후보 바에 그만큼 내놓을 자리도 없고 담을 값어치도 없다.
pub const MAX_PER_KEY: usize = 3;

/// 바이트 슬라이스 위의 zero-copy 뷰. 손상된 데이터는 조회 실패로 처리한다.
pub struct EmojiTable<'bytes> {
    bytes: &'bytes [u8],
}

impl<'bytes> EmojiTable<'bytes> {
    pub(crate) fn new(bytes: &'bytes [u8]) -> Self {
        EmojiTable { bytes }
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

    /// 이 낱말에 달린 이모지들. 없으면 빈 목록이다.
    pub fn lookup(&self, key: &str) -> Vec<&'bytes str> {
        let Some(index) = self.index_of(key) else {
            return Vec::new();
        };
        self.emoji_at(index).unwrap_or_default()
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

    fn emoji_at(&self, index: usize) -> Option<Vec<&'bytes str>> {
        let offset = self.entry_offset(index)?;
        let key_length = *self.bytes.get(offset)? as usize;
        let mut at = offset + 1 + key_length;
        let count = *self.bytes.get(at)? as usize;
        at += 1;
        let mut emoji = Vec::with_capacity(count);
        for _ in 0..count {
            let length = *self.bytes.get(at)? as usize;
            at += 1;
            emoji.push(std::str::from_utf8(self.bytes.get(at..at + length)?).ok()?);
            at += length;
        }
        Some(emoji)
    }
}
