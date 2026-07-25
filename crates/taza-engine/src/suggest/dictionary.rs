//! 조회 대상의 공통 인터페이스. 팩 lexicon(읽기 전용)과 개인화 스토어(쓰기 가능)가
//! 같은 모양으로 검색되므로, 랭킹은 사전이 몇 개든 한 번의 결합으로 끝난다.

/// 조회 키와 허용 편집 예산.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Query<'key> {
    pub key: &'key str,
    /// 0이면 접두 완성만, 그보다 크면 그 거리 안의 교정까지 찾는다
    pub max_distance: u32,
}

/// 사전 한 곳에서 나온 표제어. 전부 조회 키 공간의 값이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    /// 가산 점수 공간의 값 — 팩 lexicon은 정규화 빈도를, 개인화 스토어는 사용
    /// 가중치를 낸다. 두 값이 같은 자릿수라는 것이 랭킹 결합의 전제다.
    pub frequency: u32,
    /// 조회 키와의 편집거리. 접두 완성은 0.
    pub distance: u32,
}

pub trait Dictionary {
    fn search(&self, query: &Query<'_>, limit: usize) -> Vec<Entry>;

    /// 이 키가 정확히 표제어인가 — 자동교정 억제 판단에 쓴다.
    fn contains(&self, key: &str) -> bool;
}

impl Dictionary for crate::pack::lexicon::Lexicon<'_> {
    fn search(&self, query: &Query<'_>, limit: usize) -> Vec<Entry> {
        let mut entries: Vec<Entry> = self
            .complete(query.key, limit)
            .into_iter()
            .map(|completion| Entry {
                key: completion.word,
                frequency: completion.frequency,
                distance: 0,
            })
            .collect();
        if query.max_distance > 0 {
            entries.extend(
                self.corrections(query.key, query.max_distance, limit)
                    .into_iter()
                    .filter(|correction| correction.distance > 0)
                    .map(|correction| Entry {
                        key: correction.word,
                        frequency: correction.frequency,
                        distance: correction.distance,
                    }),
            );
        }
        entries
    }

    fn contains(&self, key: &str) -> bool {
        crate::pack::lexicon::Lexicon::contains(self, key)
    }
}

/// 개인화 스토어는 교정을 만들지 않는다 — 사용자 어휘는 표본이 적어 오교정 위험이 크다.
/// 접두 완성만 내고, 점수는 사용 가중치를 그대로 쓴다.
impl Dictionary for crate::personalization::PersonalizationStore {
    fn search(&self, query: &Query<'_>, limit: usize) -> Vec<Entry> {
        self.complete(query.key, limit)
            .into_iter()
            .map(|(key, weight)| Entry {
                key,
                frequency: weight,
                distance: 0,
            })
            .collect()
    }

    fn contains(&self, key: &str) -> bool {
        self.is_learned(key)
    }
}
