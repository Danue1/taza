//! 온디바이스 개인화 스토어 — 언어팩(읽기 전용)과 분리된 쓰기 가능 상태.
//! 시각 대신 논리 시계(기록 틱)를 쓰므로 플랫폼 무관·결정론적이며 스냅샷에 그대로 담긴다.
//! 가중치 상수는 v1 휴리스틱 — 평가 게이트를 통과하는 범위에서 튜닝한다.
//!
//! 저장 단위는 표시 형태가 아니라 **사전 조회 키**다. 팩 lexicon과 같은 공간에 있어야
//! 접두 검색이 성립하기 때문이다 (한글은 자모 ASCII — "안내"의 접두는 "안ㄴ"이 아니라
//! 키 공간에서만 제대로 잡힌다).

use std::collections::BTreeMap;

const CAPACITY: usize = 1000;
const COUNT_WEIGHT: u32 = 100;
const RECENCY_WINDOW: u64 = 10;
const RECENCY_BONUS: u32 = 250;
/// 이 횟수 이상 확정된 단어는 "학습됨" — 자동교정을 억제한다
const LEARNED_THRESHOLD: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
struct PersonalEntry {
    count: u32,
    last_used: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonalizationState {
    pub entries: Vec<(String, u32, u64)>,
    pub clock: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PersonalizationStore {
    entries: BTreeMap<String, PersonalEntry>,
    clock: u64,
}

impl PersonalizationStore {
    pub fn new() -> Self {
        PersonalizationStore::default()
    }

    pub fn record(&mut self, word: &str) {
        if word.is_empty() {
            return;
        }
        self.clock += 1;
        let clock = self.clock;
        self.entries
            .entry(word.to_string())
            .and_modify(|entry| {
                entry.count += 1;
                entry.last_used = clock;
            })
            .or_insert(PersonalEntry {
                count: 1,
                last_used: clock,
            });
        if self.entries.len() > CAPACITY {
            let evicted = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| (entry.count, entry.last_used))
                .map(|(word, _)| word.clone())
                .unwrap();
            self.entries.remove(&evicted);
        }
    }

    /// 랭킹에 더해지는 개인화 가중치: 사용 횟수 + 최근 사용 보너스
    pub fn weight(&self, word: &str) -> u32 {
        let Some(entry) = self.entries.get(word) else {
            return 0;
        };
        let recency_bonus = if self.clock - entry.last_used < RECENCY_WINDOW {
            RECENCY_BONUS
        } else {
            0
        };
        entry.count.saturating_mul(COUNT_WEIGHT) + recency_bonus
    }

    pub fn is_learned(&self, word: &str) -> bool {
        self.entries
            .get(word)
            .is_some_and(|entry| entry.count >= LEARNED_THRESHOLD)
    }

    /// prefix로 시작하는 개인화 표제어 — 사전에 없는 사용자 어휘(이름 등)의 제안 원천
    pub fn complete(&self, prefix: &str, limit: usize) -> Vec<(String, u32)> {
        let mut completions: Vec<(String, u32)> = self
            .entries
            .range(prefix.to_string()..)
            .take_while(|(word, _)| word.starts_with(prefix))
            .map(|(word, _)| (word.clone(), self.weight(word)))
            .collect();
        completions.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        completions.truncate(limit);
        completions
    }

    pub fn snapshot(&self) -> PersonalizationState {
        PersonalizationState {
            entries: self
                .entries
                .iter()
                .map(|(word, entry)| (word.clone(), entry.count, entry.last_used))
                .collect(),
            clock: self.clock,
        }
    }

    pub fn restore(state: PersonalizationState) -> Self {
        PersonalizationStore {
            entries: state
                .entries
                .into_iter()
                .map(|(word, count, last_used)| (word, PersonalEntry { count, last_used }))
                .collect(),
            clock: state.clock,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_accumulate_and_boost_recent_words() {
        let mut store = PersonalizationStore::new();
        store.record("hello");
        store.record("hello");
        assert_eq!(store.weight("hello"), 2 * COUNT_WEIGHT + RECENCY_BONUS);
        assert!(store.is_learned("hello"));
        assert!(!store.is_learned("world"));

        for _ in 0..RECENCY_WINDOW {
            store.record("other");
        }
        assert_eq!(store.weight("hello"), 2 * COUNT_WEIGHT);
    }

    #[test]
    fn completes_by_prefix() {
        let mut store = PersonalizationStore::new();
        store.record("zzva");
        store.record("zzva");
        store.record("zzb");
        let completions = store.complete("zz", 10);
        assert_eq!(completions[0].0, "zzva");
        assert_eq!(completions.len(), 2);
        assert!(store.complete("q", 10).is_empty());
    }

    #[test]
    fn evicts_least_used_entry_over_capacity() {
        let mut store = PersonalizationStore::new();
        store.record("keeper");
        store.record("keeper");
        for index in 0..CAPACITY {
            store.record(&format!("word{index}"));
        }
        assert!(store.entries.len() <= CAPACITY);
        assert!(store.is_learned("keeper"));
    }

    #[test]
    fn snapshot_roundtrip() {
        let mut store = PersonalizationStore::new();
        store.record("hello");
        store.record("hello");
        let restored = PersonalizationStore::restore(store.snapshot());
        assert_eq!(restored, store);
    }
}
