//! 온디바이스 개인화 스토어 — 언어팩(읽기 전용)과 분리된 쓰기 가능 상태.
//! 시각 대신 논리 시계(기록 틱)를 쓰므로 플랫폼 무관·결정론적이며 스냅샷에 그대로 담긴다.
//! 가중치 상수는 v1 휴리스틱 — 평가 게이트를 통과하는 범위에서 튜닝한다.
//!
//! 저장 단위는 표시 형태가 아니라 **사전 조회 키**다. 팩 lexicon과 같은 공간에 있어야
//! 접두 검색이 성립하기 때문이다 (한글은 자모 ASCII — "안내"의 접두는 "안ㄴ"이 아니라
//! 키 공간에서만 제대로 잡힌다).

use std::collections::BTreeMap;

use crate::contract::CandidateGroup;
use crate::pack::lexicon::MAX_FREQUENCY;

const CAPACITY: usize = 1000;
/// 가중치는 팩 빈도와 **같은 점수 공간**의 값이어야 한다 — 상수로 못 박으면 어휘가 큰
/// 실팩에서 학습이 랭킹에 닿지 못한다(빈도 65535 옆의 250은 없는 것과 같다).
/// 그래서 점수 공간의 비율로 잡는다: 한 번 확정에 1/10, 최근 사용에 1/4.
const COUNT_WEIGHT: u32 = MAX_FREQUENCY / 10;
const RECENCY_WINDOW: u64 = 10;
const RECENCY_BONUS: u32 = MAX_FREQUENCY / 4;
/// 이 횟수 이상 확정된 단어는 "학습됨" — 자동교정을 억제한다
const LEARNED_THRESHOLD: u32 = 2;
/// 통합 검색면의 "자주 쓰는"에 남기는 항목 수. 한 줄 남짓 채우고 나면 더 담아도 보이지
/// 않으므로, 스냅샷을 무겁게 하지 않을 만큼만 갖는다.
const RECENT_ANNOTATION_CAPACITY: usize = 24;

#[derive(Debug, Clone, PartialEq, Eq)]
struct PersonalEntry {
    count: u32,
    last_used: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonalizationState {
    pub entries: Vec<(String, u32, u64)>,
    pub clock: u64,
    /// 최근에 고른 이모지·기호·얼굴 문자 — 최근순
    pub recent_annotations: Vec<(CandidateGroup, String)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PersonalizationStore {
    entries: BTreeMap<String, PersonalEntry>,
    clock: u64,
    /// 최근에 고른 이모지·기호·얼굴 문자 — 최근순. 낱말 학습과 달리 랭킹에 쓰이지 않고
    /// 통합 검색면의 "자주 쓰는"만 채우므로 점수 없이 순서만 갖는다.
    recent_annotations: Vec<(CandidateGroup, String)>,
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

    /// 통합 검색면에서 고른 것을 최근순 맨 앞에 남긴다.
    pub fn record_annotation(&mut self, group: CandidateGroup, text: &str) {
        if text.is_empty() {
            return;
        }
        self.recent_annotations
            .retain(|(kept_group, kept)| !(*kept_group == group && kept == text));
        self.recent_annotations.insert(0, (group, text.to_string()));
        self.recent_annotations.truncate(RECENT_ANNOTATION_CAPACITY);
    }

    pub fn recent_annotations(&self) -> &[(CandidateGroup, String)] {
        &self.recent_annotations
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

    /// 이 어절의 접두 가운데 학습된 것들 — (접두, 가중치). 교착어에서 어절은
    /// "학습한 말 + 접사"로 자라므로, 접두를 되짚어야 그 자란 형태를 알아볼 수 있다.
    /// 어절 전체가 학습돼 있으면 그것은 결합형이 아니라 그 자체이므로 뺀다.
    pub fn learned_prefixes(&self, word: &str) -> Vec<(String, u32)> {
        (1..word.len())
            .filter(|&length| word.is_char_boundary(length))
            .filter(|&length| self.is_learned(&word[..length]))
            .map(|length| (word[..length].to_string(), self.weight(&word[..length])))
            .collect()
    }

    pub fn snapshot(&self) -> PersonalizationState {
        PersonalizationState {
            entries: self
                .entries
                .iter()
                .map(|(word, entry)| (word.clone(), entry.count, entry.last_used))
                .collect(),
            clock: self.clock,
            recent_annotations: self.recent_annotations.clone(),
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
            recent_annotations: state.recent_annotations,
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
        store.record_annotation(CandidateGroup::Emoji, "😀");
        let restored = PersonalizationStore::restore(store.snapshot());
        assert_eq!(restored, store);
    }

    /// 같은 것을 다시 고르면 최근순 맨 앞으로 올라오고 중복은 남지 않는다.
    #[test]
    fn recent_annotations_move_to_the_front() {
        let mut store = PersonalizationStore::new();
        store.record_annotation(CandidateGroup::Emoji, "😀");
        store.record_annotation(CandidateGroup::Emoticon, "(^_^)");
        store.record_annotation(CandidateGroup::Emoji, "😀");
        assert_eq!(
            store.recent_annotations(),
            [
                (CandidateGroup::Emoji, "😀".to_string()),
                (CandidateGroup::Emoticon, "(^_^)".to_string()),
            ]
        );
    }
}
