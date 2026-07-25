//! 조회 대상의 공통 인터페이스. 팩 lexicon(읽기 전용)과 개인화 스토어(쓰기 가능)가
//! 같은 모양으로 검색되므로, 랭킹은 사전이 몇 개든 한 번의 결합으로 끝난다.

use crate::keyboard::KeySignal;

/// 조회 키와 허용 편집 예산.
#[derive(Debug, Clone, Copy)]
pub struct Query<'key> {
    pub key: &'key str,
    /// 허용 편집 비용 (`EDIT_UNIT` 눈금). 0이면 완성만 찾는다.
    pub max_cost: u32,
    /// 키 자리마다 눌린 터치 신호 — **키의 끝에서부터** 맞춘다. 인접 키를 잘못 누른
    /// 오타가 무관한 치환보다 싸지는 근거이며, 비어 있으면 모든 치환이 같은 값이다.
    pub touches: &'key [KeySignal],
    /// 아직 입력이 끝나지 않았다고 보고 표제어 뒤에 남는 글자를 비용 없이 허용할지.
    /// 진행 중인 낱말의 완성에는 참("th"는 "theme"의 거리 0), 이미 끝난 어절의 교정에는
    /// 거짓이다 — 끝난 어절을 더 긴 낱말로 바꾸는 것은 교정이 아니다.
    pub extending: bool,
}

/// 사전 한 곳에서 나온 표제어. 전부 조회 키 공간의 값이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    /// 가산 점수 공간의 값 — 팩 lexicon은 정규화 빈도를, 개인화 스토어는 사용
    /// 가중치를 낸다. 두 값이 같은 자릿수라는 것이 랭킹 결합의 전제다.
    pub frequency: u32,
    /// 조회 키와의 편집 비용 (`EDIT_UNIT` 눈금). 접두 완성은 0.
    pub cost: u32,
}

pub trait Dictionary {
    fn search(&self, query: &Query<'_>, limit: usize) -> Vec<Entry>;

    /// 이 키가 정확히 표제어인가 — 자동교정 억제 판단에 쓴다.
    fn contains(&self, key: &str) -> bool;
}

impl Dictionary for crate::pack::lexicon::Lexicon<'_> {
    fn search(&self, query: &Query<'_>, limit: usize) -> Vec<Entry> {
        crate::suggest::search::search(self, query, limit)
    }

    fn contains(&self, key: &str) -> bool {
        crate::pack::lexicon::Lexicon::contains(self, key)
    }
}

/// 개인화 스토어는 교정을 만들지 않는다 — 사용자 어휘는 표본이 적어 오교정 위험이 크다.
/// 접두 완성만 내고, 점수는 사용 가중치를 그대로 쓴다.
impl Dictionary for crate::personalization::PersonalizationStore {
    fn search(&self, query: &Query<'_>, limit: usize) -> Vec<Entry> {
        if !query.extending {
            return Vec::new();
        }
        self.complete(query.key, limit)
            .into_iter()
            .map(|(key, weight)| Entry {
                key,
                frequency: weight,
                cost: 0,
            })
            .collect()
    }

    fn contains(&self, key: &str) -> bool {
        self.is_learned(key)
    }
}
