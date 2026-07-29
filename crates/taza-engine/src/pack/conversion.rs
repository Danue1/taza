//! conversion 섹션 — 읽기에서 표기로. 가나-한자 변환이 서는 자리다.
//!
//! lexicon과 나뉜 까닭은 **되돌릴 수 없기 때문**이다. 라틴과 한글의 조회 키는 표시 형태를
//! 그대로 담거나(utf8) 규칙으로 되돌릴 수 있어서(자모 두벌식) 표제어 하나에 표기 하나가
//! 딸리지만, 「きしゃ」에는 汽車·記者·貴社가 함께 딸린다. 노드에 값을 달 자리가 없는
//! lexicon 노드(4바이트)를 늘리면 영어·한국어 팩이 함께 커지므로, 값을 갖는 표를 따로 둔다.
//!
//! 표는 둘로 나뉜다 — 읽기를 찾는 trie(이 섹션)와 표기를 담는 곳간(`conversion_entry`).
//! 나눈 까닭은 라티스가 **읽기의 자리마다 공통 접두 탐색**을 하기 때문이다: 순회는 trie
//! 안에서만 일어나고 표기 바이트는 살아남은 마디에서만 읽힌다.
//!
//! 노드 레이아웃 (섹션 시작 기준 offset, little-endian):
//! ```text
//! entry_offset u32 (0 = 표제어 아님) | minimum_cost u16 | child_count u16
//! | (edge_byte u8, child_offset u32) × n
//! ```
//! 섹션 첫 4바이트는 루트 노드 offset이고, 자식 엣지는 바이트 오름차순이라 이진 탐색이다.
//!
//! `minimum_cost`는 이 노드 아래에서 나올 수 있는 **가장 싼** 표기의 비용이다. lexicon이
//! 최고 빈도로 가지를 치는 것과 뜻이 같고 방향만 반대다 — 변환은 비용이 낮을수록 좋다.

/// 앞말에 붙는 말임을 나타내는 flags 비트.
pub const DEPENDENT_FLAG: u8 = 1;

/// 표기 하나. 연접 비용을 매기려면 품사 자리(문맥 id)가 필요하므로 비용과 함께 담는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConversionEntry<'bytes> {
    pub surface: &'bytes str,
    /// 앞말과 이어질 때 쓰는 자리 — 연접 행렬의 열
    pub left_id: u16,
    /// 뒷말과 이어질 때 쓰는 자리 — 연접 행렬의 행
    pub right_id: u16,
    /// 이 표기 자체의 비용. 낮을수록 흔하다.
    pub cost: u16,
    /// 홀로 서지 못하고 앞말에 붙는 말인가(조사·조동사·접미사). 변환 결과를 **문절**로
    /// 묶는 유일한 근거다 — 사람이 후보를 고르는 단위가 형태소가 아니라 문절이므로,
    /// 이 한 비트가 없으면 「庭」과 「に」가 따로 선다.
    pub dependent: bool,
}

/// 읽기 하나에 딸린 표기들 — 비용 오름차순이다.
///
/// 레이아웃: `entry_count u16 | (surface_length u8, surface utf-8, left_id u16, right_id u16,
/// cost u16, flags u8) × n` — flags의 0번 비트가 `dependent`다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConversionEntries<'bytes> {
    bytes: &'bytes [u8],
}

impl<'bytes> ConversionEntries<'bytes> {
    pub fn iter(&self) -> impl Iterator<Item = ConversionEntry<'bytes>> + 'bytes {
        let bytes = self.bytes;
        let count = read_u16(bytes, 0).unwrap_or(0) as usize;
        let mut cursor = 2usize;
        (0..count).map_while(move |_| {
            let length = *bytes.get(cursor)? as usize;
            let surface = std::str::from_utf8(bytes.get(cursor + 1..cursor + 1 + length)?).ok()?;
            let numbers = cursor + 1 + length;
            let entry = ConversionEntry {
                surface,
                left_id: read_u16(bytes, numbers)?,
                right_id: read_u16(bytes, numbers + 2)?,
                cost: read_u16(bytes, numbers + 4)?,
                dependent: bytes.get(numbers + 6)? & DEPENDENT_FLAG != 0,
            };
            cursor = numbers + 7;
            Some(entry)
        })
    }

    /// 가장 싼 표기 — 라티스가 아직 서지 않은 자리(문맥이 없는 낱말 조회)에서 쓴다.
    pub fn best(&self) -> Option<ConversionEntry<'bytes>> {
        self.iter().next()
    }
}

/// trie 노드의 위치.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Node(usize);

/// 바이트 슬라이스 위의 zero-copy 변환표. 손상된 데이터는 조회 실패로 처리하며
/// 패닉하지 않는다.
#[derive(Debug, Clone, Copy)]
pub struct ConversionTable<'bytes> {
    trie: &'bytes [u8],
    entries: &'bytes [u8],
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    bytes
        .get(offset..offset + 2)
        .map(|slice| u16::from_le_bytes(slice.try_into().unwrap()))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    bytes
        .get(offset..offset + 4)
        .map(|slice| u32::from_le_bytes(slice.try_into().unwrap()))
}

impl<'bytes> ConversionTable<'bytes> {
    pub(crate) fn new(trie: &'bytes [u8], entries: &'bytes [u8]) -> Self {
        ConversionTable { trie, entries }
    }

    pub(crate) fn root(&self) -> Option<Node> {
        read_u32(self.trie, 0).map(|offset| Node(offset as usize))
    }

    /// 이 노드에서 끝나는 읽기에 딸린 표기들. 표제어가 아니면 None.
    pub fn entries_at(&self, node: Node) -> Option<ConversionEntries<'bytes>> {
        let offset = read_u32(self.trie, node.0)? as usize;
        // 곳간의 0번지에는 목록 수가 놓이므로 표기 목록이 거기서 시작할 수 없다
        if offset == 0 {
            return None;
        }
        Some(ConversionEntries {
            bytes: self.entries.get(offset..)?,
        })
    }

    /// 이 노드 아래에서 나올 수 있는 가장 싼 비용 — 가지치기의 하한.
    pub fn minimum_cost(&self, node: Node) -> u16 {
        read_u16(self.trie, node.0 + 4).unwrap_or(u16::MAX)
    }

    fn child(&self, node: Node, edge: u8) -> Option<Node> {
        let count = read_u16(self.trie, node.0 + 6)? as usize;
        let entries_start = node.0 + 8;
        let mut low = 0usize;
        let mut high = count;
        while low < high {
            let middle = (low + high) / 2;
            let entry = entries_start + middle * 5;
            let byte = *self.trie.get(entry)?;
            match byte.cmp(&edge) {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
                std::cmp::Ordering::Equal => {
                    return read_u32(self.trie, entry + 1).map(|offset| Node(offset as usize));
                }
            }
        }
        None
    }

    fn children_into(&self, node: Node, into: &mut Vec<(u8, Node)>) {
        into.clear();
        let Some(count) = read_u16(self.trie, node.0 + 6) else {
            return;
        };
        for index in 0..count as usize {
            let entry = node.0 + 8 + index * 5;
            let (Some(byte), Some(offset)) = (self.trie.get(entry), read_u32(self.trie, entry + 1))
            else {
                return;
            };
            into.push((*byte, Node(offset as usize)));
        }
    }

    pub(crate) fn walk(&self, reading: &str) -> Option<Node> {
        let mut node = self.root()?;
        for byte in reading.bytes() {
            node = self.child(node, byte)?;
        }
        Some(node)
    }

    /// 읽기 하나에 딸린 표기들.
    pub fn lookup(&self, reading: &str) -> Option<ConversionEntries<'bytes>> {
        self.entries_at(self.walk(reading)?)
    }

    /// 이 접두로 시작하는 읽기들 — 싼 것부터 `limit`개. 아직 다 치지 않은 읽기로 낱말을
    /// 미리 내놓는 자리(予測変換)가 쓴다.
    ///
    /// 가지치기가 `minimum_cost`를 보므로 하위 트리를 전수 순회하지 않는다 — 접두가 짧을
    /// 때(「か」) 그 아래에 표제어가 수만 개라도 싼 것부터 limit개에서 멈춘다.
    pub fn completions(
        &self,
        prefix: &str,
        limit: usize,
    ) -> Vec<(String, ConversionEntries<'bytes>)> {
        let mut found: Vec<(u16, String, ConversionEntries<'bytes>)> = Vec::new();
        let Some(start) = self.walk(prefix) else {
            return Vec::new();
        };
        // 싼 가지부터 — 우선순위 큐 대신 정렬된 변경 목록을 쓴다. 깊이가 읽기 길이만큼
        // 얕고 limit이 작아 목록이 길어지지 않는다.
        let mut frontier = vec![(self.minimum_cost(start), Vec::<u8>::new(), start)];
        let mut children = Vec::new();
        loop {
            frontier.sort_by_key(|(cost, _, _)| *cost);
            let Some(&(bound, _, _)) = frontier.first() else {
                break;
            };
            // 지나는 길에 만난 짧은 읽기가 자기보다 싼 긴 읽기를 밀어내지 않도록, 이미
            // 채운 자리의 값이 남은 가지의 하한보다 싸질 때까지만 판다
            if found.len() >= limit && found[limit - 1].0 <= bound {
                break;
            }
            let (_, path, node) = frontier.remove(0);
            if let Some(entries) = self.entries_at(node)
                && let Ok(suffix) = std::str::from_utf8(&path)
            {
                let cost = entries.best().map(|entry| entry.cost).unwrap_or(u16::MAX);
                found.push((cost, format!("{prefix}{suffix}"), entries));
                found.sort_by_key(|(cost, reading, _)| (*cost, reading.clone()));
            }
            self.children_into(node, &mut children);
            for &(byte, child) in &children {
                let mut next = path.clone();
                next.push(byte);
                frontier.push((self.minimum_cost(child), next, child));
            }
        }
        found.truncate(limit);
        found
            .into_iter()
            .map(|(_, reading, entries)| (reading, entries))
            .collect()
    }

    /// `reading`의 `start` 바이트 자리에서 시작하는 **모든** 표제어 — (끝 바이트 자리,
    /// 표기들). 라티스가 마디를 세우는 통로이므로 한 번의 순회로 끝난다.
    pub fn prefixes(&self, reading: &str, start: usize) -> Vec<(usize, ConversionEntries<'bytes>)> {
        let mut found = Vec::new();
        let Some(mut node) = self.root() else {
            return found;
        };
        for (index, byte) in reading.bytes().enumerate().skip(start) {
            let Some(next) = self.child(node, byte) else {
                break;
            };
            node = next;
            // 글자 경계에서만 마디가 선다 — 가나 한 글자는 UTF-8 세 바이트다
            if !reading.is_char_boundary(index + 1) {
                continue;
            }
            if let Some(entries) = self.entries_at(node) {
                found.push((index + 1, entries));
            }
        }
        found
    }
}
