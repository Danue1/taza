use std::collections::BTreeMap;

/// 섹션 바이트 레이아웃은 `taza_engine::pack::ngram` 참조.
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
