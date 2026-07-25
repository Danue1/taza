use std::collections::{BTreeMap, HashMap};
use taza_engine::pack::lexicon::MAX_FREQUENCY;

/// 섹션 바이트 레이아웃은 `taza_engine::pack::lexicon` 참조.
pub struct LexiconBuilder {
    root: BuildNode,
    word_count: usize,
}

#[derive(Default)]
struct BuildNode {
    frequency: u16,
    children: BTreeMap<u8, BuildNode>,
}

/// 하위 그래프의 동일성 판정 키 — 빈도와 (엣지, 확정된 자식 offset) 목록이 같으면
/// 두 하위 그래프는 구분되지 않으므로 한 노드를 공유한다.
type NodeSignature = (u16, Vec<(u8, u32)>);

impl LexiconBuilder {
    pub fn new() -> Self {
        LexiconBuilder {
            root: BuildNode::default(),
            word_count: 0,
        }
    }

    /// frequency는 1 이상 `MAX_FREQUENCY` 이하의 정규화된 점수다 — 원천 코퍼스의 절대
    /// 빈도를 그대로 넣는 자리가 아니다. 중복 단어는 큰 점수가 남는다.
    pub fn insert(&mut self, word: &str, frequency: u32) {
        assert!(
            (1..=MAX_FREQUENCY).contains(&frequency),
            "빈도 점수가 범위를 벗어남: {frequency} (1..={MAX_FREQUENCY})"
        );
        let mut node = &mut self.root;
        for byte in word.bytes() {
            node = node.children.entry(byte).or_default();
        }
        if node.frequency == 0 {
            self.word_count += 1;
        }
        node.frequency = node.frequency.max(frequency as u16);
    }

    pub fn word_count(&self) -> usize {
        self.word_count
    }

    /// 최소화(DAWG)한 뒤 직렬화한다 — 접미사가 같은 하위 그래프가 한 노드로 합쳐지므로
    /// 굴절이 많은 언어에서 섹션이 크게 줄어든다. 리더는 노드를 offset으로만 다루므로
    /// 공유는 조회 경로에 영향을 주지 않는다.
    pub fn build(self) -> Vec<u8> {
        fn serialize(
            node: &BuildNode,
            buffer: &mut Vec<u8>,
            shared: &mut HashMap<NodeSignature, u32>,
        ) -> u32 {
            let children: Vec<(u8, u32)> = node
                .children
                .iter()
                .map(|(byte, child)| (*byte, serialize(child, buffer, shared)))
                .collect();
            let signature: NodeSignature = (node.frequency, children);
            if let Some(&offset) = shared.get(&signature) {
                return offset;
            }
            let offset = buffer.len() as u32;
            buffer.extend_from_slice(&node.frequency.to_le_bytes());
            buffer.extend_from_slice(&(signature.1.len() as u16).to_le_bytes());
            for &(byte, child_offset) in &signature.1 {
                buffer.push(byte);
                buffer.extend_from_slice(&child_offset.to_le_bytes());
            }
            shared.insert(signature, offset);
            offset
        }

        let mut buffer = vec![0u8; 4];
        let mut shared = HashMap::new();
        let root_offset = serialize(&self.root, &mut buffer, &mut shared);
        buffer[0..4].copy_from_slice(&root_offset.to_le_bytes());
        buffer
    }
}

impl Default for LexiconBuilder {
    fn default() -> Self {
        LexiconBuilder::new()
    }
}
