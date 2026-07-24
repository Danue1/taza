use std::collections::BTreeMap;

/// bigram 언어모델 섹션 (`ngram-v1`). 레이아웃 (섹션 시작 기준 offset, little-endian):
/// ```text
/// token_count u32 | entry_count u32
/// group_start u32 × (token_count + 1)      // 토큰별 bigram 슬라이스의 엔트리 인덱스
/// entries (right_id u32, weight u32) × m   // 그룹 내부는 weight 내림차순 — top-k는 앞에서 k개
/// token_offset u32 × token_count           // 사전순 정렬 — 문자열 이진 탐색, id = 순번
/// token bytes: (length u8, utf-8) × n
/// ```
pub struct NgramModelBuilder {
    bigrams: BTreeMap<String, BTreeMap<String, u32>>,
}

impl NgramModelBuilder {
    pub fn new() -> Self {
        NgramModelBuilder {
            bigrams: BTreeMap::new(),
        }
    }

    /// 중복 삽입은 가중치를 누적한다.
    pub fn insert_bigram(&mut self, left: &str, right: &str, weight: u32) {
        *self
            .bigrams
            .entry(left.to_string())
            .or_default()
            .entry(right.to_string())
            .or_insert(0) += weight;
    }

    pub fn build(self) -> Vec<u8> {
        let mut tokens: Vec<&str> = self
            .bigrams
            .iter()
            .flat_map(|(left, rights)| {
                std::iter::once(left.as_str()).chain(rights.keys().map(String::as_str))
            })
            .collect();
        tokens.sort_unstable();
        tokens.dedup();
        let token_id: BTreeMap<&str, u32> = tokens
            .iter()
            .enumerate()
            .map(|(index, &token)| (token, index as u32))
            .collect();

        let mut group_start: Vec<u32> = Vec::with_capacity(tokens.len() + 1);
        let mut entries: Vec<(u32, u32)> = Vec::new();
        for &token in &tokens {
            group_start.push(entries.len() as u32);
            if let Some(rights) = self.bigrams.get(token) {
                let mut group: Vec<(u32, u32)> = rights
                    .iter()
                    .map(|(right, &weight)| (token_id[right.as_str()], weight))
                    .collect();
                group.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
                entries.extend(group);
            }
        }
        group_start.push(entries.len() as u32);

        let token_count = tokens.len();
        let entry_count = entries.len();
        let token_offsets_start = 8 + 4 * (token_count + 1) + 8 * entry_count;
        let token_bytes_start = token_offsets_start + 4 * token_count;

        let mut output = Vec::new();
        output.extend_from_slice(&(token_count as u32).to_le_bytes());
        output.extend_from_slice(&(entry_count as u32).to_le_bytes());
        for start in &group_start {
            output.extend_from_slice(&start.to_le_bytes());
        }
        for (right, weight) in &entries {
            output.extend_from_slice(&right.to_le_bytes());
            output.extend_from_slice(&weight.to_le_bytes());
        }
        let mut token_offset = token_bytes_start as u32;
        let mut token_blob = Vec::new();
        for &token in &tokens {
            output.extend_from_slice(&token_offset.to_le_bytes());
            assert!(token.len() <= u8::MAX as usize, "토큰이 너무 김: {token}");
            token_blob.push(token.len() as u8);
            token_blob.extend_from_slice(token.as_bytes());
            token_offset += 1 + token.len() as u32;
        }
        output.extend_from_slice(&token_blob);
        output
    }
}

impl Default for NgramModelBuilder {
    fn default() -> Self {
        NgramModelBuilder::new()
    }
}

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
        self.read_u32(8 + 4 * token_index).map(|value| value as usize)
    }

    fn token_offsets_start(&self) -> usize {
        8 + 4 * (self.token_count() + 1) + 8 * self.entry_count()
    }

    fn token(&self, token_index: usize) -> Option<&'bytes str> {
        let offset =
            self.read_u32(self.token_offsets_start() + 4 * token_index)? as usize;
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
            let (Some(right_id), Some(weight)) = (
                self.read_u32(entry_offset),
                self.read_u32(entry_offset + 4),
            ) else {
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
