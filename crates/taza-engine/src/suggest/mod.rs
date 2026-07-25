//! 후보 생성·랭킹 — 언어와 직교한다. 합성기는 "지금 무엇에 대한 제안이 필요한가"만
//! 내고(조회 키), 사전·언어모델·개인화를 어떻게 합쳐 순위를 매길지는 전부 여기서 정한다.
//! 언어가 늘어도 이 코드는 늘지 않는다.

pub mod dictionary;
pub mod encoding;
mod score;
mod search;

pub use dictionary::{Dictionary, Entry, Query};
pub use encoding::KeyEncoding;

use crate::contract::{CandidateGroup, CandidateKind, Pack};
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
    /// 어절 하나에 곁들일 항목 수 — 갈래마다 따로 센다. 낱말 후보 뒤에 붙으므로 `limit`과
    /// 따로 둔다 — 곁들이는 것이 낱말이 설 자리를 가져가서는 안 된다.
    pub annotation_limit: usize,
}

/// 후보 하나. `key`는 학습·문맥 추적이 쓰는 조회 키이고 `text`는 화면에 나가는 형태다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub key: String,
    pub text: String,
    pub kind: CandidateKind,
    pub group: CandidateGroup,
}

/// 랭킹이 참조하는 온디바이스 자료 묶음. 팩은 mmap 뷰라 이벤트마다 새로 만든다.
pub struct SuggestionSources<'call> {
    pub pack: Option<&'call Pack<'call>>,
    /// 개인화가 꺼진 입력(설정 off·비밀번호 필드 등)에서는 None — 그때는 기록도
    /// 조회도 없이 사전과 언어모델만으로 랭킹한다.
    pub personalization: Option<&'call PersonalizationStore>,
    /// 직전에 확정된 어휘의 조회 키 — 언어모델 문맥
    pub previous_word: Option<&'call str>,
    /// 지금 어절에 눌린 터치 신호 — 조회 키의 끝에서부터 맞춘다
    pub touches: &'call [KeySignal],
}

impl SuggestionSources<'_> {
    fn learned_weight(&self, key: &str) -> u32 {
        self.personalization.map_or(0, |store| store.weight(key))
    }

    fn is_learned(&self, key: &str) -> bool {
        self.personalization
            .is_some_and(|store| store.is_learned(key))
    }

    fn learned_entries(&self, query: &Query<'_>, limit: usize) -> Vec<Entry> {
        self.personalization
            .map_or_else(Vec::new, |store| store.search(query, limit))
    }

    fn learned_prefixes(&self, key: &str) -> Vec<(String, u32)> {
        self.personalization
            .map_or_else(Vec::new, |store| store.learned_prefixes(key))
    }
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

        // 원문은 순정 관습대로 언제나 첫 자리 — 사전이 무엇을 내놓든 친 대로 두는 길이
        // 열려 있어야 하고, 사전에 없는 말을 칠 때 후보 바가 비지 않는 것도 이 슬롯이다.
        // 사전이 없으면 이 슬롯도 없다 — 고를 다른 후보가 애초에 없으니 "친 대로 두기"가
        // 선택지가 되지 못하고, 후보 바에 방금 친 글자만 되비칠 뿐이다.
        let mut suggestions = Vec::new();
        if lexicon.is_some()
            && let Some(text) = self.policy.encoding.decode(key)
        {
            suggestions.push(Suggestion {
                key: key.to_string(),
                text,
                kind: CandidateKind::Typed,
                group: CandidateGroup::Word,
            });
        }

        if let Some(lexicon) = &lexicon {
            for entry in lexicon.search(&query, self.policy.limit * DICTIONARY_POOL_FACTOR) {
                let score = score::combine(
                    entry.frequency,
                    sources.learned_weight(&entry.key),
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
        for entry in sources.learned_entries(&query, self.policy.limit) {
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
        for (combined, weight) in self.learned_with_affix(key, sources) {
            let score = score::combine(weight, 0, 0, 0);
            self.push_ranked(&mut ranked, score, combined, CandidateKind::Prediction);
        }

        ranked.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| left.1.text.cmp(&right.1.text))
        });
        // 원문 슬롯은 랭킹 자리를 빼앗지 않는다 — limit은 사전이 내놓는 후보의 수다
        let ranked_limit = self.policy.limit + suggestions.len();
        for (_, suggestion) in ranked {
            if suggestions.len() >= ranked_limit {
                break;
            }
            if suggestions.iter().any(|kept| kept.text == suggestion.text) {
                continue;
            }
            suggestions.push(suggestion);
        }
        suggestions.extend(self.annotations_for(key, sources));
        suggestions
    }

    /// 지금 치고 있는 어절에 달린 이모지·기호·얼굴 문자. 낱말 후보 뒤에 갈래 순서대로
    /// 붙는다 — 순정 키보드가 그렇듯 곁들이는 것은 낱말을 밀어내지 않는다. 치던 것이
    /// 그대로 확정될 길을 막아서는 안 된다.
    ///
    /// 어절이 다 완성된 뒤에만 내놓는다(정확히 일치). 치는 도중의 접두마다 튀어나오면
    /// 후보 바가 어절이 끝나기 전에 흔들린다.
    fn annotations_for(&self, key: &str, sources: &SuggestionSources<'_>) -> Vec<Suggestion> {
        let Some(table) = sources.pack.and_then(|pack| pack.annotations()) else {
            return Vec::new();
        };
        let mut annotations = Vec::new();
        for group in CandidateGroup::DISPLAY_ORDER {
            if group == CandidateGroup::Word {
                continue;
            }
            annotations.extend(
                table
                    .lookup_group(key, group)
                    .into_iter()
                    .take(self.policy.annotation_limit)
                    .map(|text| Suggestion {
                        key: key.to_string(),
                        text: text.to_string(),
                        kind: CandidateKind::Conversion,
                        group,
                    }),
            );
        }
        annotations
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
                sources.learned_weight(&prediction.word),
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
        if !self.policy.autocorrect || sources.is_learned(key) {
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
            sources.learned_weight(&best.key),
            self.language_model_weight(&best.key, sources),
            best.cost,
        );
        let typed = score::combine(0, sources.learned_weight(key), 0, 0);
        if corrected - typed <= score::AUTOCORRECT_MARGIN {
            return None;
        }
        let text = self.policy.encoding.decode(&best.key)?;
        Some(Suggestion {
            key: best.key,
            text,
            kind: CandidateKind::Correction,
            group: CandidateGroup::Word,
        })
    }

    /// 학습한 어휘에 접사가 붙은 어절 — (조회 키, 개인화 가중치).
    ///
    /// 교착어에서는 사용자가 "타자"를 쓰기 시작하면 "타자를"·"타자는"도 곧 치게 되는데,
    /// 개인화 스토어에는 확정한 형태 그대로만 남으므로 결합형은 사전에도 스토어에도
    /// 없다. 팩이 밝힌 접사 목록으로 그 자리를 메운다 — 사전을 넓힐 때 쓴 것과 같은
    /// 목록이라 둘이 어긋나지 않는다.
    ///
    /// 지금 치고 있는 어절이 완성돼 가는 중일 수도 있으므로("타자ㄹ") 접사는 접두만
    /// 맞아도 받아들인다. 결합형이 이미 사전에 있으면 사전 쪽 점수가 옳으니 내지 않는다.
    fn learned_with_affix(&self, key: &str, sources: &SuggestionSources<'_>) -> Vec<(String, u32)> {
        let Some(pack) = sources.pack else {
            return Vec::new();
        };
        let Some(affixes) = pack.affixes() else {
            return Vec::new();
        };
        let lexicon = pack.lexicon();
        let mut combined = Vec::new();
        for (stem, weight) in sources.learned_prefixes(key) {
            let typed_affix = &key[stem.len()..];
            for affix in affixes.split('\n') {
                let Some(encoded) = self.policy.encoding.encode(affix) else {
                    continue;
                };
                if !encoded.starts_with(typed_affix) {
                    continue;
                }
                let word = format!("{stem}{encoded}");
                if lexicon
                    .as_ref()
                    .is_some_and(|lexicon| lexicon.contains(&word))
                {
                    continue;
                }
                combined.push((word, weight));
            }
        }
        combined
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
        ranked.push((
            score,
            Suggestion {
                key,
                text,
                kind,
                group: CandidateGroup::Word,
            },
        ));
    }
}
