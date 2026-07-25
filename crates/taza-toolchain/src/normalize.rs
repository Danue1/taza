//! 원천 신호들을 팩에 담을 하나의 점수표로 합친다.
//!
//! 점수는 원천 코퍼스의 절대 빈도가 아니라 [1, `MAX_FREQUENCY`]로 정규화된 값이다.
//! 절대 빈도를 그대로 실으면 (1) 흔한 낱말의 점수가 개인화 가중치를 압도해 학습이
//! 랭킹에 닿지 못하고, (2) 원천을 갈아치울 때마다 랭킹 스케일이 달라져 평가 결과를
//! 비교할 수 없다. 로그 스케일로 옮겨 두 문제를 함께 없앤다.

use crate::recipe::{LexiconRules, Role};
use std::collections::HashMap;
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
}

#[derive(Debug)]
pub struct NormalizeReport {
    pub inventory_size: usize,
    /// 빈도 원천에서 관측된 인벤토리 표제어 수 — 원천 조합이 실제로 맞물리는지의 지표
    pub observed_in_corpus: usize,
    pub dropped_by_filter: usize,
    pub dropped_by_budget: usize,
}

/// 반환값은 (표제어, 정규화 점수)를 점수 내림차순·사전순으로 정렬한 목록이다.
pub fn normalize(
    signals: &[SourceSignal<'_>],
    rules: &LexiconRules,
) -> (Vec<(String, u32)>, NormalizeReport) {
    let has_inventory = signals.iter().any(|signal| signal.role == Role::Inventory);
    let mut accumulated: HashMap<&str, Accumulated> = HashMap::new();
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
                    // 고유명사·외국어)이 사전에 스며드는 경로를 막는다.
                    let entry = match accumulated.get_mut(word.as_str()) {
                        Some(entry) => entry,
                        None if has_inventory => continue,
                        None => accumulated.entry(word).or_default(),
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
        },
    )
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
                },
                SourceSignal {
                    role: Role::Frequency,
                    weight: 1.0,
                    entries: &corpus,
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
            }],
            &rules(1),
        );
        // 한 글자와 문자 집합을 벗어난 낱말이 걸러지고, 예산이 나머지를 자른다
        assert_eq!(report.dropped_by_filter, 2);
        assert_eq!(report.dropped_by_budget, 1);
        assert_eq!(words, vec![("keyboard".to_string(), MAX_FREQUENCY)]);
    }
}
