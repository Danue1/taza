//! 후보 생성·랭킹 — 언어와 직교한다. 합성기는 "지금 무엇에 대한 제안이 필요한가"만
//! 내고(조회 키), 사전·언어모델·개인화를 어떻게 합쳐 순위를 매길지는 전부 여기서 정한다.
//! 언어가 늘어도 이 코드는 늘지 않는다.

pub mod dictionary;
pub mod encoding;
mod lookup;
mod score;
mod search;

pub use dictionary::{Dictionary, Entry, Query};
pub use encoding::KeyEncoding;

use std::collections::HashMap;

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

/// 언어별로 달라지는 랭킹 정책. 입력 방식이 자기에 맞는 값을 선언한다.
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

/// 직전 어휘가 뒷말마다 주는 문맥 이득. 후보를 하나 볼 때마다 언어모델을 새로 뒤지면
/// 타건 한 번에 같은 조회가 후보 수만큼 되풀이되므로, 한 번 걷어 두고 꺼내 쓴다.
#[derive(Debug, Default)]
struct ContextWeights(HashMap<String, u32>);

impl ContextWeights {
    fn gather(sources: &SuggestionSources<'_>) -> Self {
        let (Some(previous_word), Some(language_model)) = (
            sources.previous_word,
            sources.pack.and_then(|pack| pack.language_model()),
        ) else {
            return ContextWeights::default();
        };
        ContextWeights(
            language_model
                .predict_next(previous_word, LANGUAGE_MODEL_POOL)
                .into_iter()
                .map(|prediction| (prediction.word, prediction.weight))
                .collect(),
        )
    }

    fn weight(&self, key: &str) -> u32 {
        self.0.get(key).copied().unwrap_or(0)
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

    /// 확정된 어휘를 언어모델 문맥으로 쓸 꼴로 옮긴다.
    ///
    /// 팩의 bigram 토큰은 사전 표제어와 같은 **접힌** 키 공간에 있는데 합성기가 내는 키는
    /// 친 꼴 그대로다. 접지 않으면 자동 대문자화가 shift를 올리는 문장 첫 낱말마다 문맥이
    /// 끊겨, 문장의 두 번째 낱말이 매번 예측도 재랭킹도 받지 못한다.
    pub fn context_key(&self, key: &str) -> String {
        self.policy
            .encoding
            .fold(key)
            .map_or_else(|| key.to_string(), |(folded, _)| folded)
    }

    /// 진행 중인 단어의 완성·교정. 사전에 없는 개인화 어휘와, 자동교정을 쓰는
    /// 방식에서는 원문 그대로의 후보까지 합쳐 낸다.
    pub fn suggest(&self, key: &str, sources: &SuggestionSources<'_>) -> Vec<Suggestion> {
        if key.is_empty() {
            return Vec::new();
        }
        let lexicon = sources.pack.and_then(|pack| pack.lexicon());
        // 사전은 소문자 표제어만 담으므로 대문자가 섞인 키는 접어서 찾고, 찾아낸 표제어에
        // 원문의 꼴을 되씌운다. 접기가 성립하는지는 키 공간이 정한다 — 두벌식 ASCII처럼
        // 대문자가 다른 자모인 공간에서는 접지 않는다.
        let folded = self.policy.encoding.fold(key);
        let lookup_key = folded.as_ref().map_or(key, |(folded, _)| folded.as_str());
        let restore = folded
            .as_ref()
            .map_or(lookup::VERBATIM, |(_, restore)| *restore);
        let query = Query {
            key: lookup_key,
            max_cost: search::edit_budget(key.chars().count()),
            touches: sources.touches,
            encoding: self.policy.encoding,
            extending: true,
        };
        let context = ContextWeights::gather(sources);
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
                    context.weight(&entry.key),
                    entry.cost,
                );
                let kind = if entry.cost == 0 {
                    CandidateKind::Prediction
                } else {
                    CandidateKind::Correction
                };
                self.push_ranked(&mut ranked, score, entry.key, kind, restore);
            }
        }
        // 개인화 스토어가 아는 표제어. 사전에 없는 사용자 어휘(이름 등)가 여기서 들어오고,
        // 사전에 **있는** 표제어도 여기를 한 번 더 지난다 — 사전 탐색은 빈도만 보고 가지를
        // 치므로, 빈도가 낮은 표제어는 학습이 아무리 쌓여도 그 pool에 들지 못한다.
        // 사전 빈도까지 더해 넣으면 위에서 나온 같은 후보보다 점수가 높아 앞자리를 잡고,
        // 뒤따르는 중복은 표시 형태로 걸러진다.
        for (entry, restore) in self.learned_lookup(key, restore, &query, sources) {
            let dictionary_frequency = lexicon
                .as_ref()
                .and_then(|lexicon| lexicon.frequency(&entry.key))
                .unwrap_or(0);
            let score = score::combine(
                entry.frequency,
                dictionary_frequency,
                context.weight(&entry.key),
                entry.cost,
            );
            self.push_ranked(
                &mut ranked,
                score,
                entry.key,
                CandidateKind::Prediction,
                restore,
            );
        }
        for (combined, weight) in self.learned_with_affix(lookup_key, sources) {
            let score = score::combine(weight, 0, 0, 0);
            self.push_ranked(
                &mut ranked,
                score,
                combined,
                CandidateKind::Prediction,
                restore,
            );
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
        suggestions.extend(self.annotations_for(lookup_key, sources));
        suggestions
    }

    /// 개인화 스토어에서 찾은 표제어와, 그 표제어에 되씌울 꼴.
    ///
    /// 스토어는 사용자가 확정한 꼴 그대로 담는다 — 고유명사에서는 대문자가 곧 뜻이라
    /// 접어서 넣을 수 없다. 그래서 접은 키뿐 아니라 원문 키로도 찾는다. 원문 키로 찾은
    /// 것은 이미 제 꼴을 갖고 있으므로 되씌우지 않는다.
    fn learned_lookup(
        &self,
        key: &str,
        restore: lookup::Restore,
        query: &Query<'_>,
        sources: &SuggestionSources<'_>,
    ) -> Vec<(Entry, lookup::Restore)> {
        let mut found: Vec<(Entry, lookup::Restore)> = sources
            .learned_entries(query, self.policy.limit)
            .into_iter()
            .map(|entry| (entry, restore))
            .collect();
        if key == query.key {
            return found;
        }
        let typed = Query { key, ..*query };
        for entry in sources.learned_entries(&typed, self.policy.limit) {
            if !found.iter().any(|(kept, _)| kept.key == entry.key) {
                found.push((entry, lookup::VERBATIM));
            }
        }
        found
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
                lookup::VERBATIM,
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
        // 조회는 접은 키로 — 접지 않으면 문장 첫 낱말("The")이 사전에 없는 말로 보여
        // 교정 대상이 된다
        let folded = self.policy.encoding.fold(key);
        let lookup_key = folded.as_ref().map_or(key, |(folded, _)| folded.as_str());
        let restore = folded
            .as_ref()
            .map_or(lookup::VERBATIM, |(_, restore)| *restore);
        if lexicon.contains(lookup_key) {
            return None;
        }
        // 이미 끝난 어절이므로 뒤에 글자가 남는 표제어는 교정이 아니다
        let query = Query {
            key: lookup_key,
            max_cost: search::edit_budget(key.chars().count()),
            touches: sources.touches,
            encoding: self.policy.encoding,
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
            ContextWeights::gather(sources).weight(&best.key),
            best.cost,
        );
        let typed = score::combine(0, sources.learned_weight(key), 0, 0);
        if corrected - typed <= score::AUTOCORRECT_MARGIN {
            return None;
        }
        let text = restore.apply(&self.policy.encoding.decode(&best.key)?);
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

    fn push_ranked(
        &self,
        ranked: &mut Vec<(i64, Suggestion)>,
        score: i64,
        key: String,
        kind: CandidateKind,
        restore: lookup::Restore,
    ) {
        let Some(text) = self
            .policy
            .encoding
            .decode(&key)
            .map(|text| restore.apply(&text))
        else {
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
