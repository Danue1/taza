//! bigram 언어모델 섹션 (`ngram-v1`). 레이아웃 (섹션 시작 기준 offset, little-endian):
//! ```text
//! token_count u32 | entry_count u32
//! group_start u32 × (token_count + 1)      // 토큰별 bigram 슬라이스의 엔트리 인덱스
//! entries (right_id u32, weight u32) × m   // 그룹 내부는 weight 내림차순 — top-k는 앞에서 k개
//! token_offset u32 × token_count           // 사전순 정렬 — 문자열 이진 탐색, id = 순번
//! token bytes: (length u8, utf-8) × n
//! ```

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prediction {
    pub word: String,
    pub weight: u32,
}

/// 바이트 슬라이스 위의 zero-copy bigram 모델 뷰. 손상 데이터는 빈 결과로 처리한다.
pub struct NgramModel<'bytes> {
    bytes: &'bytes [u8],
}

impl<'bytes> NgramModel<'bytes> {
    pub(crate) fn new(bytes: &'bytes [u8]) -> Self {
        NgramModel { bytes }
    }

    fn read_u32(&self, offset: usize) -> Option<u32> {
        self.bytes
            .get(offset..offset + 4)
            .map(|slice| u32::from_le_bytes(slice.try_into().unwrap()))
    }

    fn token_count(&self) -> usize {
        self.read_u32(0).unwrap_or(0) as usize
    }

    fn entry_count(&self) -> usize {
        self.read_u32(4).unwrap_or(0) as usize
    }

    fn group_start(&self, token_index: usize) -> Option<usize> {
        self.read_u32(8 + 4 * token_index)
            .map(|value| value as usize)
    }

    fn token_offsets_start(&self) -> usize {
        8 + 4 * (self.token_count() + 1) + 8 * self.entry_count()
    }

    fn token(&self, token_index: usize) -> Option<&'bytes str> {
        let offset = self.read_u32(self.token_offsets_start() + 4 * token_index)? as usize;
        let length = *self.bytes.get(offset)? as usize;
        std::str::from_utf8(self.bytes.get(offset + 1..offset + 1 + length)?).ok()
    }

    fn token_index(&self, word: &str) -> Option<usize> {
        let mut low = 0usize;
        let mut high = self.token_count();
        while low < high {
            let middle = (low + high) / 2;
            match self.token(middle)?.cmp(word) {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
                std::cmp::Ordering::Equal => return Some(middle),
            }
        }
        None
    }

    /// previous_word 다음에 올 단어를 가중치 내림차순으로 최대 limit개 반환.
    pub fn predict_next(&self, previous_word: &str, limit: usize) -> Vec<Prediction> {
        let Some(token_index) = self.token_index(previous_word) else {
            return Vec::new();
        };
        let (Some(start), Some(end)) = (
            self.group_start(token_index),
            self.group_start(token_index + 1),
        ) else {
            return Vec::new();
        };
        let entries_start = 8 + 4 * (self.token_count() + 1);
        let mut predictions = Vec::new();
        for entry_index in start..end.min(start + limit) {
            let entry_offset = entries_start + 8 * entry_index;
            let (Some(right_id), Some(weight)) =
                (self.read_u32(entry_offset), self.read_u32(entry_offset + 4))
            else {
                break;
            };
            let Some(word) = self.token(right_id as usize) else {
                break;
            };
            predictions.push(Prediction {
                word: word.to_string(),
                weight,
            });
        }
        predictions
    }
}
