//! 후보 생성·랭킹 — 언어와 직교한다. 합성기는 "지금 무엇에 대한 제안이 필요한가"만
//! 내고(조회 키), 사전·언어모델·개인화를 어떻게 합쳐 순위를 매길지는 전부 여기서 정한다.
//! 언어가 늘어도 이 코드는 늘지 않는다.

pub mod dictionary;
pub mod encoding;
mod score;
mod search;

pub use dictionary::{Dictionary, Entry, Query};
pub use encoding::KeyEncoding;

use crate::contract::{CandidateKind, Pack};
use crate::keyboard::KeySignal;
use crate::personalization::PersonalizationStore;

/// 언어모델에서 끌어와 재랭킹할 후보 수. 팩의 문맥 그룹은 이득 순으로 정렬돼 있는데
/// 최종 순위는 거기에 낱말 빈도를 더한 값이라, 그룹 앞에서 limit개만 꺼내면 결합력만
/// 강한 희귀어가 자리를 다 차지한다.
const LANGUAGE_MODEL_POOL: usize = 32;

/// 사전에서 표시할 개수의 몇 배를 끌어올지. 사전 탐색은 빈도와 편집거리만 보고 자르는데
/// 최종 순위에는 개인화와 문맥이 더해지므로, 표시할 만큼만 끌어오면 재랭킹이 뒤집을
/// 재료가 없다.
const DICTIONARY_POOL_FACTOR: usize = 4;

/// 언어별로 달라지는 랭킹 정책. 합성기가 자기 골격에 맞는 값을 선언한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuggestionPolicy {
    pub encoding: KeyEncoding,
    /// 단어 경계에서 자동교정을 시도하는가. 원문(as-typed) 후보를 함께 노출할지도
    /// 이 값에 딸린다 — 교정을 피해 원문을 고르는 것이 곧 학습 경로이기 때문이다.
    /// 한글처럼 조합 자체가 표시 단위인 스크립트는 경계 교정 대신 후보 선택으로 고친다.
    pub autocorrect: bool,
    pub limit: usize,
}

/// 후보 하나. `key`는 학습·문맥 추적이 쓰는 조회 키이고 `text`는 화면에 나가는 형태다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub key: String,
    pub text: String,
    pub kind: CandidateKind,
}

/// 랭킹이 참조하는 온디바이스 자료 묶음. 팩은 mmap 뷰라 이벤트마다 새로 만든다.
pub struct SuggestionSources<'call> {
    pub pack: Option<&'call Pack<'call>>,
    pub personalization: &'call PersonalizationStore,
    /// 직전에 확정된 어휘의 조회 키 — 언어모델 문맥
    pub previous_word: Option<&'call str>,
    /// 지금 어절에 눌린 터치 신호 — 조회 키의 끝에서부터 맞춘다
    pub touches: &'call [KeySignal],
}

pub struct Suggester {
    policy: SuggestionPolicy,
}

impl Suggester {
    pub fn new(policy: SuggestionPolicy) -> Self {
        Suggester { policy }
    }

    pub fn policy(&self) -> SuggestionPolicy {
        self.policy
    }

    /// 진행 중인 단어의 완성·교정. 사전에 없는 개인화 어휘와, 자동교정을 쓰는
    /// 골격에서는 원문 그대로의 후보까지 합쳐 낸다.
    pub fn suggest(&self, key: &str, sources: &SuggestionSources<'_>) -> Vec<Suggestion> {
        if key.is_empty() {
            return Vec::new();
        }
        let lexicon = sources.pack.and_then(|pack| pack.lexicon());
        let query = Query {
            key,
            max_cost: search::edit_budget(key.chars().count()),
            touches: sources.touches,
            extending: true,
        };
        let mut ranked: Vec<(i64, Suggestion)> = Vec::new();

        // 원문이 이미 표제어이거나 학습된 어휘면 순정 관습대로 맨 앞에 둔다
        let known = lexicon
            .as_ref()
            .is_some_and(|lexicon| lexicon.contains(key))
            || sources.personalization.is_learned(key);
        let mut suggestions = Vec::new();
        if known && let Some(text) = self.policy.encoding.decode(key) {
            suggestions.push(Suggestion {
                key: key.to_string(),
                text,
                kind: CandidateKind::Prediction,
            });
        }

        if let Some(lexicon) = &lexicon {
            for entry in lexicon.search(&query, self.policy.limit * DICTIONARY_POOL_FACTOR) {
                let score = score::combine(
                    entry.frequency,
                    sources.personalization.weight(&entry.key),
                    self.language_model_weight(&entry.key, sources),
                    entry.cost,
                );
                let kind = if entry.cost == 0 {
                    CandidateKind::Prediction
                } else {
                    CandidateKind::Correction
                };
                self.push_ranked(&mut ranked, score, entry.key, kind);
            }
        }
        // 사전에 없는 사용자 어휘(이름 등) — 개인화 스토어만이 아는 표제어
        for entry in sources.personalization.search(&query, self.policy.limit) {
            if lexicon
                .as_ref()
                .is_some_and(|lexicon| lexicon.contains(&entry.key))
            {
                continue;
            }
            let score = score::combine(
                entry.frequency,
                0,
                self.language_model_weight(&entry.key, sources),
                entry.cost,
            );
            self.push_ranked(&mut ranked, score, entry.key, CandidateKind::Prediction);
        }

        ranked.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| left.1.text.cmp(&right.1.text))
        });
        for (_, suggestion) in ranked {
            if suggestions.len() >= self.policy.limit {
                break;
            }
            if suggestions.iter().any(|kept| kept.text == suggestion.text) {
                continue;
            }
            suggestions.push(suggestion);
        }

        // 미등재 원문은 목록 끝에 그대로 노출 — 자동교정을 피해 이것을 고르는 것이
        // 학습 경로다. 교정하지 않는 골격에는 필요 없다.
        if self.policy.autocorrect
            && lexicon.is_some()
            && !known
            && let Some(text) = self.policy.encoding.decode(key)
            && !suggestions.iter().any(|kept| kept.text == text)
        {
            suggestions.push(Suggestion {
                key: key.to_string(),
                text,
                kind: CandidateKind::Prediction,
            });
        }
        suggestions
    }

    /// 단어 확정 직후의 다음 단어 예측.
    pub fn predict_next(&self, sources: &SuggestionSources<'_>) -> Vec<Suggestion> {
        let (Some(previous_word), Some(language_model)) = (
            sources.previous_word,
            sources.pack.and_then(|pack| pack.language_model()),
        ) else {
            return Vec::new();
        };
        let lexicon = sources.pack.and_then(|pack| pack.lexicon());
        let mut ranked: Vec<(i64, Suggestion)> = Vec::new();
        for prediction in language_model.predict_next(previous_word, LANGUAGE_MODEL_POOL) {
            // 저장된 가중치는 문맥이 주는 이득이므로 뒷말 자체의 빈도를 더해야
            // "문맥을 감안한 점수"가 된다 — 현재 단어 재랭킹과 같은 식이다
            let frequency = lexicon
                .as_ref()
                .and_then(|lexicon| lexicon.frequency(&prediction.word))
                .unwrap_or(0);
            let score = score::combine(
                frequency,
                sources.personalization.weight(&prediction.word),
                prediction.weight,
                0,
            );
            self.push_ranked(
                &mut ranked,
                score,
                prediction.word,
                CandidateKind::Prediction,
            );
        }
        ranked.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| left.1.text.cmp(&right.1.text))
        });
        ranked
            .into_iter()
            .map(|(_, suggestion)| suggestion)
            .take(self.policy.limit)
            .collect()
    }

    /// 단어 경계에서 원문을 대신할 교정. 없으면 원문을 그대로 둔다.
    pub fn autocorrection(&self, key: &str, sources: &SuggestionSources<'_>) -> Option<Suggestion> {
        // 학습된 어휘(사용자 사전)는 사전에 없어도 교정하지 않는다.
        // 짧은 입력의 오교정은 편집 예산(edit_budget)이 0을 주는 것으로 막힌다.
        if !self.policy.autocorrect || sources.personalization.is_learned(key) {
            return None;
        }
        let lexicon = sources.pack.and_then(|pack| pack.lexicon())?;
        if lexicon.contains(key) {
            return None;
        }
        // 이미 끝난 어절이므로 뒤에 글자가 남는 표제어는 교정이 아니다
        let query = Query {
            key,
            max_cost: search::edit_budget(key.chars().count()),
            touches: sources.touches,
            extending: false,
        };
        let best = lexicon
            .search(&query, 1)
            .into_iter()
            .find(|entry| entry.cost > 0)?;
        // 원문을 갈아치울 만큼 앞서는가. 원문은 사전에 없으므로(위에서 걸렀다) 그 점수는
        // 개인화 가중치뿐이며, 교정 후보는 편집 비용을 치르고도 그만큼을 넘어야 한다.
        let corrected = score::combine(
            best.frequency,
            sources.personalization.weight(&best.key),
            self.language_model_weight(&best.key, sources),
            best.cost,
        );
        let typed = score::combine(0, sources.personalization.weight(key), 0, 0);
        if corrected <= typed {
            return None;
        }
        let text = self.policy.encoding.decode(&best.key)?;
        Some(Suggestion {
            key: best.key,
            text,
            kind: CandidateKind::Correction,
        })
    }

    fn language_model_weight(&self, key: &str, sources: &SuggestionSources<'_>) -> u32 {
        let (Some(previous_word), Some(language_model)) = (
            sources.previous_word,
            sources.pack.and_then(|pack| pack.language_model()),
        ) else {
            return 0;
        };
        language_model
            .predict_next(previous_word, LANGUAGE_MODEL_POOL)
            .into_iter()
            .find(|prediction| prediction.word == key)
            .map(|prediction| prediction.weight)
            .unwrap_or(0)
    }

    fn push_ranked(
        &self,
        ranked: &mut Vec<(i64, Suggestion)>,
        score: i64,
        key: String,
        kind: CandidateKind,
    ) {
        let Some(text) = self.policy.encoding.decode(&key) else {
            return;
        };
        ranked.push((score, Suggestion { key, text, kind }));
    }
}
