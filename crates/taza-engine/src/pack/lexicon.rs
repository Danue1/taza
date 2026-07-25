//! lexicon 섹션 — UTF-8 바이트 trie(부분 그래프 공유 = DAWG). 노드 레이아웃
//! (섹션 시작 기준 offset, little-endian):
//! ```text
//! frequency u16 (0 = 단어 아님) | max_subtree_frequency u16 | child_count u16
//! | (edge_byte u8, child_offset u32) × n
//! ```
//! 섹션 첫 4바이트는 루트 노드 offset. 자식 엣지는 바이트 오름차순 정렬 — 이진 탐색 조회.
//!
//! `max_subtree_frequency`는 이 노드 아래에서 나올 수 있는 최고 빈도다. 탐색이 이미
//! 확보한 후보보다 좋아질 수 없는 가지를 통째로 건너뛰게 해 주며, 이것이 없으면 짧은
//! 접두의 완성 조회가 하위 트리를 전수 순회하게 된다.
//!
//! 빈도는 원천 코퍼스의 절대 빈도가 아니라 [1, `MAX_FREQUENCY`]로 정규화된 점수다 —
//! 개인화 가중치와 같은 자릿수에서 더해지도록 파이프라인이 로그 스케일로 옮겨 담는다.
//! 같은 점수를 가진 하위 그래프는 하나로 합쳐지므로(컴파일러가 최소화) 노드가 여러
//! 접두사에서 공유될 수 있다 — 순회 코드는 노드를 상태가 아니라 offset으로만 다룬다.

/// 정규화된 빈도 점수의 상한 — 와이어에 u16으로 담긴다.
pub const MAX_FREQUENCY: u32 = u16::MAX as u32;

/// trie 노드의 위치. 하위 그래프가 공유되므로 노드는 상태가 아니라 위치일 뿐이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Node(usize);

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

    pub(crate) fn root(&self) -> Option<Node> {
        self.read_u32(0).map(|offset| Node(offset as usize))
    }

    /// 이 노드에서 끝나는 표제어의 빈도. 0이면 단어가 아니다.
    pub(crate) fn frequency_at(&self, node: Node) -> u32 {
        self.read_u16(node.0).map(u32::from).unwrap_or(0)
    }

    /// 이 노드 아래에서 나올 수 있는 최고 빈도 — 가지치기의 상한.
    pub(crate) fn max_subtree_frequency(&self, node: Node) -> u32 {
        self.read_u16(node.0 + 2).map(u32::from).unwrap_or(0)
    }

    /// 자식 엣지를 바이트 오름차순으로 담는다. 호출자가 버퍼를 재사용하므로 순회가
    /// 노드마다 할당하지 않는다.
    pub(crate) fn children_into(&self, node: Node, into: &mut Vec<(u8, Node)>) {
        into.clear();
        let Some(count) = self.read_u16(node.0 + 4) else {
            return;
        };
        for index in 0..count as usize {
            let entry = node.0 + 6 + index * 5;
            let (Some(byte), Some(offset)) = (self.bytes.get(entry), self.read_u32(entry + 1))
            else {
                return;
            };
            into.push((*byte, Node(offset as usize)));
        }
    }

    fn child(&self, node: Node, edge: u8) -> Option<Node> {
        let count = self.read_u16(node.0 + 4)? as usize;
        let entries_start = node.0 + 6;
        let mut low = 0usize;
        let mut high = count;
        while low < high {
            let middle = (low + high) / 2;
            let entry = entries_start + middle * 5;
            let byte = *self.bytes.get(entry)?;
            match byte.cmp(&edge) {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
                std::cmp::Ordering::Equal => {
                    return self.read_u32(entry + 1).map(|offset| Node(offset as usize));
                }
            }
        }
        None
    }

    pub(crate) fn walk(&self, word: &str) -> Option<Node> {
        let mut node = self.root()?;
        for byte in word.bytes() {
            node = self.child(node, byte)?;
        }
        Some(node)
    }

    pub fn frequency(&self, word: &str) -> Option<u32> {
        let frequency = self.frequency_at(self.walk(word)?);
        (frequency > 0).then_some(frequency)
    }

    pub fn contains(&self, word: &str) -> bool {
        self.frequency(word).is_some()
    }
}
