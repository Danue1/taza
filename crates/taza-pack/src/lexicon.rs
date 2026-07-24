use std::collections::BTreeMap;

/// UTF-8 바이트 trie. 노드 레이아웃 (섹션 시작 기준 offset, little-endian):
/// ```text
/// frequency u32 (0 = 단어 아님) | child_count u16 | (edge_byte u8, child_offset u32) × n
/// ```
/// 섹션 첫 4바이트는 루트 노드 offset. 자식 엣지는 바이트 오름차순 정렬 — 이진 탐색 조회.
pub struct LexiconBuilder {
    root: BuildNode,
}

#[derive(Default)]
struct BuildNode {
    frequency: u32,
    children: BTreeMap<u8, BuildNode>,
}

impl LexiconBuilder {
    pub fn new() -> Self {
        LexiconBuilder {
            root: BuildNode::default(),
        }
    }

    /// frequency는 1 이상이어야 단어로 취급된다. 중복 단어는 큰 빈도가 남는다.
    pub fn insert(&mut self, word: &str, frequency: u32) {
        let mut node = &mut self.root;
        for byte in word.bytes() {
            node = node.children.entry(byte).or_default();
        }
        node.frequency = node.frequency.max(frequency);
    }

    pub fn build(self) -> Vec<u8> {
        fn serialize(node: &BuildNode, buffer: &mut Vec<u8>) -> u32 {
            let children: Vec<(u8, u32)> = node
                .children
                .iter()
                .map(|(byte, child)| (*byte, serialize(child, buffer)))
                .collect();
            let offset = buffer.len() as u32;
            buffer.extend_from_slice(&node.frequency.to_le_bytes());
            buffer.extend_from_slice(&(children.len() as u16).to_le_bytes());
            for (byte, child_offset) in children {
                buffer.push(byte);
                buffer.extend_from_slice(&child_offset.to_le_bytes());
            }
            offset
        }

        let mut buffer = vec![0u8; 4];
        let root_offset = serialize(&self.root, &mut buffer);
        buffer[0..4].copy_from_slice(&root_offset.to_le_bytes());
        buffer
    }
}

impl Default for LexiconBuilder {
    fn default() -> Self {
        LexiconBuilder::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    pub word: String,
    pub frequency: u32,
}

/// 바이트 슬라이스 위의 zero-copy lexicon 뷰. 손상된 데이터는 조회 실패(None/빈 결과)로
/// 처리하며 패닉하지 않는다.
pub struct Lexicon<'bytes> {
    bytes: &'bytes [u8],
}

impl<'bytes> Lexicon<'bytes> {
    pub(crate) fn new(bytes: &'bytes [u8]) -> Self {
        Lexicon { bytes }
    }

    fn read_u32(&self, offset: usize) -> Option<u32> {
        self.bytes
            .get(offset..offset + 4)
            .map(|slice| u32::from_le_bytes(slice.try_into().unwrap()))
    }

    fn read_u16(&self, offset: usize) -> Option<u16> {
        self.bytes
            .get(offset..offset + 2)
            .map(|slice| u16::from_le_bytes(slice.try_into().unwrap()))
    }

    fn node_frequency(&self, node: usize) -> Option<u32> {
        self.read_u32(node)
    }

    fn node_children(&self, node: usize) -> Option<Vec<(u8, usize)>> {
        let count = self.read_u16(node + 4)? as usize;
        let mut children = Vec::with_capacity(count);
        for index in 0..count {
            let entry = node + 6 + index * 5;
            let byte = *self.bytes.get(entry)?;
            let offset = self.read_u32(entry + 1)? as usize;
            children.push((byte, offset));
        }
        Some(children)
    }

    fn child(&self, node: usize, edge: u8) -> Option<usize> {
        let count = self.read_u16(node + 4)? as usize;
        let entries_start = node + 6;
        let mut low = 0usize;
        let mut high = count;
        while low < high {
            let middle = (low + high) / 2;
            let entry = entries_start + middle * 5;
            let byte = *self.bytes.get(entry)?;
            match byte.cmp(&edge) {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
                std::cmp::Ordering::Equal => return Some(self.read_u32(entry + 1)? as usize),
            }
        }
        None
    }

    fn walk(&self, word: &str) -> Option<usize> {
        let mut node = self.read_u32(0)? as usize;
        for byte in word.bytes() {
            node = self.child(node, byte)?;
        }
        Some(node)
    }

    pub fn frequency(&self, word: &str) -> Option<u32> {
        let frequency = self.node_frequency(self.walk(word)?)?;
        (frequency > 0).then_some(frequency)
    }

    pub fn contains(&self, word: &str) -> bool {
        self.frequency(word).is_some()
    }

    /// prefix로 시작하는 단어들을 빈도 내림차순(동률은 사전순)으로 최대 limit개 반환.
    /// prefix 자신이 단어면 포함된다.
    pub fn complete(&self, prefix: &str, limit: usize) -> Vec<Completion> {
        let Some(start) = self.walk(prefix) else {
            return Vec::new();
        };
        let mut completions = Vec::new();
        let mut stack = vec![(start, prefix.as_bytes().to_vec())];
        while let Some((node, word_bytes)) = stack.pop() {
            if let Some(frequency) = self.node_frequency(node)
                && frequency > 0
                && let Ok(word) = String::from_utf8(word_bytes.clone())
            {
                completions.push(Completion { word, frequency });
            }
            let Some(children) = self.node_children(node) else {
                continue;
            };
            for (byte, child) in children {
                let mut extended = word_bytes.clone();
                extended.push(byte);
                stack.push((child, extended));
            }
        }
        completions.sort_by(|a, b| {
            b.frequency
                .cmp(&a.frequency)
                .then_with(|| a.word.cmp(&b.word))
        });
        completions.truncate(limit);
        completions
    }
}
