use std::collections::BTreeMap;
use taza_engine::pack::conversion::DEPENDENT_FLAG;

/// 섹션 바이트 레이아웃은 `taza_engine::pack::conversion` 참조. trie와 곳간을 함께 내는
/// 까닭은 둘이 offset으로 묶여 있어 따로 지을 수 없기 때문이다.
pub struct ConversionBuilder {
    readings: BTreeMap<String, Vec<Entry>>,
}

/// 표기 하나 — 사전에서 온 그대로다. 비용 정규화는 파이프라인이 이미 마쳤다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub surface: String,
    pub left_id: u16,
    pub right_id: u16,
    pub cost: u16,
    /// 홀로 서지 못하고 앞말에 붙는 말인가 — 변환 결과를 문절로 묶는 근거다.
    pub dependent: bool,
}

#[derive(Default)]
struct BuildNode {
    /// 곳간에서의 자리 — 표제어가 아니면 0
    entry_offset: u32,
    minimum_cost: u16,
    children: BTreeMap<u8, BuildNode>,
}

impl Default for ConversionBuilder {
    fn default() -> Self {
        ConversionBuilder::new()
    }
}

impl ConversionBuilder {
    pub fn new() -> Self {
        ConversionBuilder {
            readings: BTreeMap::new(),
        }
    }

    /// 같은 읽기에 같은 표기가 두 번 오면 싼 쪽이 남는다 — 원천이 여럿일 때 흔한 일이다.
    pub fn insert(&mut self, reading: &str, entry: Entry) {
        let entries = self.readings.entry(reading.to_string()).or_default();
        match entries
            .iter_mut()
            .find(|existing| existing.surface == entry.surface)
        {
            Some(existing) if existing.cost > entry.cost => *existing = entry,
            Some(_) => {}
            None => entries.push(entry),
        }
    }

    pub fn reading_count(&self) -> usize {
        self.readings.len()
    }

    pub fn entry_count(&self) -> usize {
        self.readings.values().map(Vec::len).sum()
    }

    /// (trie 바이트, 곳간 바이트).
    pub fn build(mut self) -> (Vec<u8>, Vec<u8>) {
        // 곳간의 0번지는 목록 수가 갖는다 — 그래야 offset 0이 "표제어 아님"을 뜻할 수 있다
        let mut store = (self.readings.len() as u32).to_le_bytes().to_vec();
        let mut root = BuildNode {
            minimum_cost: u16::MAX,
            ..BuildNode::default()
        };
        for (reading, entries) in &mut self.readings {
            entries.sort_by_key(|entry| (entry.cost, entry.surface.clone()));
            let offset = store.len() as u32;
            store.extend_from_slice(&(entries.len() as u16).to_le_bytes());
            for entry in entries.iter() {
                let surface = entry.surface.as_bytes();
                assert!(surface.len() <= u8::MAX as usize, "표기가 너무 김");
                store.push(surface.len() as u8);
                store.extend_from_slice(surface);
                store.extend_from_slice(&entry.left_id.to_le_bytes());
                store.extend_from_slice(&entry.right_id.to_le_bytes());
                store.extend_from_slice(&entry.cost.to_le_bytes());
                store.push(match entry.dependent {
                    true => DEPENDENT_FLAG,
                    false => 0,
                });
            }
            let cheapest = entries
                .iter()
                .map(|entry| entry.cost)
                .min()
                .unwrap_or(u16::MAX);
            let mut node = &mut root;
            node.minimum_cost = node.minimum_cost.min(cheapest);
            for byte in reading.bytes() {
                node = node.children.entry(byte).or_insert_with(|| BuildNode {
                    minimum_cost: u16::MAX,
                    ..BuildNode::default()
                });
                node.minimum_cost = node.minimum_cost.min(cheapest);
            }
            node.entry_offset = offset;
        }

        let mut trie = vec![0u8; 4];
        let root_offset = serialize(&root, &mut trie);
        trie[..4].copy_from_slice(&root_offset.to_le_bytes());
        (trie, store)
    }
}

/// 자식을 먼저 적고 그 자리를 부모가 가리킨다 — 앞에서부터 한 번에 쓰기 위해서다.
///
/// lexicon과 달리 **하위 그래프를 합치지 않는다**(DAWG 최소화 없음). 그쪽 노드가 담는 것은
/// 빈도 하나뿐이라 꼬리가 같은 가지가 수없이 겹치지만, 여기서는 종단마다 곳간 자리가
/// 달라 합쳐질 하위 그래프가 사실상 없다 — 실팩(읽기 120만)에서 재어 보니 줄어드는 것이
/// 없었고 빌드 시간과 메모리만 늘었다.
fn serialize(node: &BuildNode, buffer: &mut Vec<u8>) -> u32 {
    let children: Vec<(u8, u32)> = node
        .children
        .iter()
        .map(|(&byte, child)| (byte, serialize(child, buffer)))
        .collect();
    let offset = buffer.len() as u32;
    buffer.extend_from_slice(&node.entry_offset.to_le_bytes());
    buffer.extend_from_slice(&node.minimum_cost.to_le_bytes());
    buffer.extend_from_slice(&(children.len() as u16).to_le_bytes());
    for (byte, child_offset) in children {
        buffer.push(byte);
        buffer.extend_from_slice(&child_offset.to_le_bytes());
    }
    offset
}

/// 연접 행렬 — 앞말이 나가는 자리(행)와 뒷말이 들어오는 자리(열)가 만나는 칸의 값.
pub struct ConnectionBuilder {
    row_count: u16,
    column_count: u16,
    costs: Vec<i16>,
}

impl ConnectionBuilder {
    pub fn new(row_count: u16, column_count: u16) -> Self {
        ConnectionBuilder {
            row_count,
            column_count,
            costs: vec![0; row_count as usize * column_count as usize],
        }
    }

    pub fn set(&mut self, previous_right_id: u16, next_left_id: u16, cost: i16) {
        let index = previous_right_id as usize * self.column_count as usize + next_left_id as usize;
        if let Some(slot) = self.costs.get_mut(index) {
            *slot = cost;
        }
    }

    pub fn build(self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(4 + self.costs.len() * 2);
        bytes.extend_from_slice(&self.row_count.to_le_bytes());
        bytes.extend_from_slice(&self.column_count.to_le_bytes());
        for cost in self.costs {
            bytes.extend_from_slice(&cost.to_le_bytes());
        }
        bytes
    }
}
