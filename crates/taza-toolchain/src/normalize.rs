//! 원천 신호들을 팩에 담을 하나의 점수표로 합친다.
//!
//! 점수는 원천 코퍼스의 절대 빈도가 아니라 [1, `MAX_FREQUENCY`]로 정규화된 값이다.
//! 절대 빈도를 그대로 실으면 (1) 흔한 낱말의 점수가 개인화 가중치를 압도해 학습이
//! 랭킹에 닿지 못하고, (2) 원천을 갈아치울 때마다 랭킹 스케일이 달라져 평가 결과를
//! 비교할 수 없다. 로그 스케일로 옮겨 두 문제를 함께 없앤다.

use crate::recipe::{LanguageModelRules, LexiconRules, Role};
use std::collections::{HashMap, HashSet};
use taza_engine::lang::jamo::decompose_word;
use taza_engine::pack::lexicon::MAX_FREQUENCY;

#[derive(Debug, Default)]
struct Accumulated {
    /// 인벤토리 원천이 준 흔함 등급의 가중 합
    rank: f64,
    /// 빈도 원천이 준 실사용 횟수의 가중 합
    count: f64,
    in_inventory: bool,
}

pub struct SourceSignal<'call> {
    pub role: Role,
    pub weight: f64,
    pub entries: &'call [(String, f64)],
    /// (앞말, 뒷말, 관측 횟수) — 문장 코퍼스만 채운다
    pub bigrams: &'call [(String, String, u64)],
    /// 활용형이 뻗어 나오는 어간 — 형태소 사전만 채운다
    pub stems: &'call [String],
}

/// 예산에 밀려 팩에서 빠진 낱말 중 남겨 둘 표본 수. 오교정률 평가의 코퍼스가 된다 —
/// 잘린 낱말은 "사전에 없지만 사람이 실제로 쓰는 말"이므로, 이것을 제대로 쳤을 때
/// 자동교정이 건드리는지가 곧 오교정률이다.
const ABSENT_SAMPLE: usize = 2000;

#[derive(Debug)]
pub struct NormalizeReport {
    pub inventory_size: usize,
    /// 빈도 원천에서 관측된 인벤토리 표제어 수 — 원천 조합이 실제로 맞물리는지의 지표
    pub observed_in_corpus: usize,
    pub dropped_by_filter: usize,
    pub dropped_by_budget: usize,
    /// 인벤토리에 없지만 알려진 어간에서 뻗어 나와 받아들인 활용형 수
    pub accepted_inflections: usize,
    /// 예산 바로 아래에서 잘린 낱말들 — 점수가 높을수록 실제로 쳐질 법한 말이다
    pub absent_words: Vec<String>,
}

/// 반환값은 (표제어, 정규화 점수)를 점수 내림차순·사전순으로 정렬한 목록이다.
pub fn normalize(
    signals: &[SourceSignal<'_>],
    rules: &LexiconRules,
) -> (Vec<(String, u32)>, NormalizeReport) {
    let has_inventory = signals.iter().any(|signal| signal.role == Role::Inventory);
    let stems: HashSet<Vec<char>> = signals
        .iter()
        .flat_map(|signal| signal.stems.iter().map(|stem| inflection_key(stem)))
        .collect();
    let mut accumulated: HashMap<&str, Accumulated> = HashMap::new();
    let mut accepted_inflections = 0usize;
    for signal in signals {
        for (word, value) in signal.entries {
            match signal.role {
                Role::Inventory => {
                    let entry = accumulated.entry(word).or_default();
                    entry.in_inventory = true;
                    entry.rank += value * signal.weight;
                }
                Role::Frequency => {
                    // 인벤토리가 있으면 그 밖의 낱말은 받지 않는다 — 코퍼스 잡음(오타·
                    // 고유명사·외국어)이 사전에 스며드는 경로를 막는다. 다만 알려진
                    // 어간에서 뻗어 나온 활용형은 예외다(accept_inflections).
                    let entry = match accumulated.get_mut(word.as_str()) {
                        Some(entry) => entry,
                        None if !has_inventory => accumulated.entry(word).or_default(),
                        None if rules.accept_inflections && grows_from_stem(word, &stems) => {
                            accepted_inflections += 1;
                            accumulated.entry(word).or_default()
                        }
                        None => continue,
                    };
                    entry.count += value * signal.weight;
                }
            }
        }
    }

    let inventory_size = accumulated.len();
    let observed_in_corpus = accumulated
        .values()
        .filter(|entry| entry.count > 0.0)
        .count();

    let mut filtered: Vec<(&str, f64)> = Vec::with_capacity(accumulated.len());
    for (word, entry) in &accumulated {
        if word.chars().count() < rules.minimum_word_length || !rules.character_set.accepts(word) {
            continue;
        }
        filtered.push((word, entry.rank + logarithmic(entry.count)));
    }
    let dropped_by_filter = accumulated.len() - filtered.len();

    filtered.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.0.cmp(right.0))
    });
    let dropped_by_budget = filtered.len().saturating_sub(rules.max_words);
    let absent_words: Vec<String> = filtered
        .iter()
        .skip(rules.max_words)
        .take(ABSENT_SAMPLE)
        .map(|(word, _)| word.to_string())
        .collect();
    filtered.truncate(rules.max_words);

    let highest = filtered.first().map(|(_, score)| *score).unwrap_or(1.0);
    let words = filtered
        .into_iter()
        .map(|(word, score)| (word.to_string(), quantize(score, highest)))
        .collect();
    (
        words,
        NormalizeReport {
            inventory_size,
            observed_in_corpus,
            dropped_by_filter,
            dropped_by_budget,
            accepted_inflections,
            absent_words,
        },
    )
}

/// 문맥 이득이 차지할 수 있는 점수 공간의 몫 — 낱말 빈도 공간의 1/2까지다.
/// 문맥이 빈도를 뒤집을 수는 있어야 하지만, 결합력만 강한 희귀어("the meantime")가
/// 흔한 낱말을 밀어내면 예측이 쓸모없어진다. 두 눈금이 같은 단위가 아니라서
/// (한쪽은 상호정보량, 한쪽은 등급+로그 빈도의 혼합) 나오는 한계이며, 어휘 점수를
/// 순수 로그확률로 다시 세우기 전까지의 실용적 상한이다.
const LIFT_CEILING_DIVISOR: u32 = 2;

/// 짝 하나에 대해 모은 것 — 담을 값(이득)과 자를 기준(관측 횟수)은 다른 수다.
#[derive(Debug, Default)]
struct PairSignal {
    lift: f64,
    count: u64,
}

#[derive(Debug)]
pub struct BigramReport {
    pub observed: usize,
    /// 표제어에 없는 낱말이 끼어 있어 버린 짝
    pub dropped_outside_lexicon: usize,
    /// 문맥이 이득을 주지 않아(상호정보량 ≤ 0) 버린 짝 — 흔한 낱말끼리의 우연한 이웃
    pub dropped_without_lift: usize,
    pub dropped_by_budget: usize,
}

/// 코퍼스에서 관측된 이웃 짝을 언어모델 섹션에 담을 가중치로 옮긴다.
///
/// 담는 값은 뒷말의 절대 빈도가 아니라 **문맥이 주는 이득**(상호정보량)이다. 절대 빈도를
/// 담으면 소비자가 "흔한 낱말"과 "이 문맥에서 흔한 낱말"을 구분할 수 없다. 이득만 담아
/// 두면 소비자가 lexicon 빈도를 더해 문맥 확률을 복원할 수 있고, 다음 단어 예측과 현재
/// 단어 재랭킹이 같은 식을 쓰게 된다.
///
/// 예산은 이득이 아니라 **관측 횟수**로 자른다. 이득 순으로 자르면 살아남는 것이 드물고
/// 특이한 결합("carbon dioxide")뿐이라, 정작 사람이 자주 치는 흔한 문맥에는 예측이
/// 하나도 남지 않는다.
pub fn normalize_bigrams(
    signals: &[SourceSignal<'_>],
    lexicon: &[(String, u32)],
    rules: &LanguageModelRules,
) -> (Vec<(String, String, u32)>, BigramReport) {
    let known: HashSet<&str> = lexicon.iter().map(|(word, _)| word.as_str()).collect();
    let mut pairs: HashMap<(&str, &str), PairSignal> = HashMap::new();
    let mut observed = 0usize;
    let mut dropped_outside_lexicon = 0usize;
    let mut dropped_without_lift = 0usize;

    for signal in signals {
        if signal.bigrams.is_empty() {
            continue;
        }
        let counts: HashMap<&str, f64> = signal
            .entries
            .iter()
            .map(|(word, count)| (word.as_str(), *count))
            .collect();
        let total: f64 = signal
            .bigrams
            .iter()
            .map(|(_, _, count)| *count as f64)
            .sum();
        if total <= 0.0 {
            continue;
        }
        for (left, right, count) in signal.bigrams {
            if *count < rules.minimum_count {
                continue;
            }
            observed += 1;
            if !known.contains(left.as_str()) || !known.contains(right.as_str()) {
                dropped_outside_lexicon += 1;
                continue;
            }
            let (Some(left_count), Some(right_count)) =
                (counts.get(left.as_str()), counts.get(right.as_str()))
            else {
                dropped_outside_lexicon += 1;
                continue;
            };
            let lift = (*count as f64 * total / (left_count * right_count)).ln();
            if lift <= 0.0 {
                dropped_without_lift += 1;
                continue;
            }
            let slot = pairs.entry((left.as_str(), right.as_str())).or_default();
            slot.lift += lift * signal.weight;
            slot.count += count;
        }
    }

    let mut ranked: Vec<((&str, &str), PairSignal)> = pairs.into_iter().collect();
    ranked.sort_by(|left, right| {
        right
            .1
            .count
            .cmp(&left.1.count)
            .then_with(|| {
                right
                    .1
                    .lift
                    .partial_cmp(&left.1.lift)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| left.0.cmp(&right.0))
    });
    let dropped_by_budget = ranked.len().saturating_sub(rules.max_bigrams);
    ranked.truncate(rules.max_bigrams);

    let highest = ranked
        .iter()
        .map(|(_, signal)| signal.lift)
        .fold(f64::MIN, f64::max)
        .max(1.0);
    let bigrams = ranked
        .into_iter()
        .map(|((left, right), signal)| {
            let scaled = quantize(signal.lift, highest) / LIFT_CEILING_DIVISOR;
            (left.to_string(), right.to_string(), scaled.max(1))
        })
        .collect();
    (
        bigrams,
        BigramReport {
            observed,
            dropped_outside_lexicon,
            dropped_without_lift,
            dropped_by_budget,
        },
    )
}

/// 활용 관계를 재는 단위. 한글은 자모로 풀고, 풀 수 없는 글자가 섞이면 표층 그대로 둔다.
///
/// 표층 음절로 재면 한국어 활용의 대부분을 놓친다 — 어미는 어간의 마지막 음절 **안으로**
/// 들어가 그 음절을 바꾸기 때문이다("하"→"했", "오"→"왔", "주"→"줘"). 자모로 풀면 이
/// 변화가 어미 자모의 덧붙음으로 드러나(ㅈㅜ → ㅈㅜㅓ) 접두 관계가 그대로 성립한다.
fn inflection_key(word: &str) -> Vec<char> {
    decompose_word(word).unwrap_or_else(|| word.chars().collect())
}

/// 알려진 어간에서 뻗어 나온 낱말인가 — 활용형으로 볼 수 있는 최소 조건이다.
/// 한국어 용언 어간은 대개 한 음절(있·하·같)이라 길이로 더 조이면 정작 흔한 활용형이
/// 다 걸러진다. 어간 집합이 용언 파일에서만 오므로(고유명사는 애초에 빠져 있다)
/// 이 조건만으로도 잡음이 새는 길은 좁다.
fn grows_from_stem(word: &str, stems: &HashSet<Vec<char>>) -> bool {
    let key = inflection_key(word);
    (1..key.len()).any(|length| stems.contains(&key[..length]))
}

/// 실사용 횟수는 로그로 눌러 담는다 — 상위 몇 낱말이 점수 공간을 독점하지 않게.
fn logarithmic(count: f64) -> f64 {
    if count <= 0.0 {
        0.0
    } else {
        (1.0 + count).ln()
    }
}

fn quantize(score: f64, highest: f64) -> u32 {
    let ratio = if highest > 0.0 {
        (score / highest).clamp(0.0, 1.0)
    } else {
        0.0
    };
    1 + (ratio * (MAX_FREQUENCY - 1) as f64).round() as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe::{CharacterSet, LexiconEncoding};

    fn rules(max_words: usize) -> LexiconRules {
        LexiconRules {
            encoding: LexiconEncoding::Utf8,
            character_set: CharacterSet::LatinLowercase,
            max_words,
            minimum_word_length: 2,
            accept_inflections: false,
        }
    }

    #[test]
    fn frequency_source_only_boosts_inventory_words() {
        let inventory = vec![("the".to_string(), 0.9), ("theme".to_string(), 0.3)];
        let corpus = vec![
            ("theme".to_string(), 5000.0),
            ("qwertyish".to_string(), 9.0),
        ];
        let (words, report) = normalize(
            &[
                SourceSignal {
                    role: Role::Inventory,
                    weight: 1.0,
                    entries: &inventory,
                    bigrams: &[],
                    stems: &[],
                },
                SourceSignal {
                    role: Role::Frequency,
                    weight: 1.0,
                    entries: &corpus,
                    bigrams: &[],
                    stems: &[],
                },
            ],
            &rules(10),
        );
        assert_eq!(report.inventory_size, 2);
        assert_eq!(report.observed_in_corpus, 1);
        assert!(!words.iter().any(|(word, _)| word == "qwertyish"));
        // 코퍼스에서 많이 관측된 낱말이 등급만 높은 낱말을 앞지른다
        assert_eq!(words[0].0, "theme");
        assert!(
            words
                .iter()
                .all(|(_, score)| (1..=MAX_FREQUENCY).contains(score))
        );
    }

    #[test]
    fn bigrams_keep_context_lift_not_raw_frequency() {
        // "fox"는 "quick"보다 드물게 이어졌지만 둘 다 문맥 이득이 있다. 담기는 값은
        // 이득이므로 순서가 관측 횟수가 아니라 이득을 따른다.
        let counts = vec![
            ("the".to_string(), 100.0),
            ("quick".to_string(), 10.0),
            ("fox".to_string(), 10.0),
        ];
        let bigrams = vec![
            ("the".to_string(), "quick".to_string(), 8),
            ("the".to_string(), "fox".to_string(), 2),
            // 표제어 밖 낱말 — 총량에는 들어가지만 팩에는 담기지 않는다
            ("zzz".to_string(), "yyy".to_string(), 990),
        ];
        let lexicon = vec![
            ("the".to_string(), 100u32),
            ("quick".to_string(), 50),
            ("fox".to_string(), 50),
        ];
        let (result, report) = normalize_bigrams(
            &[SourceSignal {
                role: Role::Frequency,
                weight: 1.0,
                entries: &counts,
                bigrams: &bigrams,
                stems: &[],
            }],
            &lexicon,
            &LanguageModelRules {
                max_bigrams: 10,
                minimum_count: 2,
            },
        );
        assert_eq!(report.observed, 3);
        assert_eq!(report.dropped_outside_lexicon, 1);
        // 이득 눈금의 상한은 낱말 빈도 공간의 1/2이다
        assert_eq!(
            result,
            vec![
                (
                    "the".to_string(),
                    "quick".to_string(),
                    MAX_FREQUENCY / LIFT_CEILING_DIVISOR
                ),
                ("the".to_string(), "fox".to_string(), 10923),
            ]
        );
    }

    #[test]
    fn applies_filters_and_budget() {
        let inventory = vec![
            ("a".to_string(), 1.0),
            ("naïve".to_string(), 0.8),
            ("keyboard".to_string(), 0.6),
            ("language".to_string(), 0.4),
        ];
        let (words, report) = normalize(
            &[SourceSignal {
                role: Role::Inventory,
                weight: 1.0,
                entries: &inventory,
                bigrams: &[],
                stems: &[],
            }],
            &rules(1),
        );
        // 한 글자와 문자 집합을 벗어난 낱말이 걸러지고, 예산이 나머지를 자른다
        assert_eq!(report.dropped_by_filter, 2);
        assert_eq!(report.dropped_by_budget, 1);
        assert_eq!(words, vec![("keyboard".to_string(), MAX_FREQUENCY)]);
    }
}
