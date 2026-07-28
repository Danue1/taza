//! 언어모델 — 이웃 짝에 담기는 것은 뒷말의 절대 빈도가 아니라 문맥이 주는 이득이다.

use std::collections::{HashMap, HashSet};

use crate::recipe::LanguageModelRules;

use super::SourceSignal;
use super::scale::quantize;

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
        // 이득은 같은 원천 안에서 재야 한다 — 낱말 빈도와 짝 빈도가 같은 코퍼스에서
        // 나온 수여야 상호정보량이 뜻을 갖는다.
        let total: f64 = signal
            .bigrams
            .iter()
            .map(|(_, _, count)| *count as f64)
            .sum();
        if total <= 0.0 {
            continue;
        }
        for &(left, right, count) in signal.bigrams {
            if count < rules.minimum_count {
                continue;
            }
            observed += 1;
            let (Some((left, left_count)), Some((right, right_count))) = (
                signal.observed.get(left as usize),
                signal.observed.get(right as usize),
            ) else {
                dropped_outside_lexicon += 1;
                continue;
            };
            let (left, right) = (left.as_str(), right.as_str());
            if !known.contains(left) || !known.contains(right) {
                dropped_outside_lexicon += 1;
                continue;
            }
            let lift = (count as f64 * total / (*left_count as f64 * *right_count as f64)).ln();
            if lift <= 0.0 {
                dropped_without_lift += 1;
                continue;
            }
            let slot = pairs.entry((left, right)).or_default();
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

#[cfg(test)]
mod tests {
    use super::super::fixture::*;
    use super::*;
    use crate::recipe::{LanguageModelRules, Role};
    use taza_engine::pack::lexicon::MAX_FREQUENCY;

    #[test]
    fn bigrams_keep_context_lift_not_raw_frequency() {
        // "fox"는 "quick"보다 드물게 이어졌지만 둘 다 문맥 이득이 있다. 담기는 값은
        // 이득이므로 순서가 관측 횟수가 아니라 이득을 따른다.
        let observed = vec![
            ("the".to_string(), 100),
            ("quick".to_string(), 10),
            ("fox".to_string(), 10),
        ];
        // 번호는 `observed`의 자리 번호다. 마지막 짝은 그 자리에 없는 번호를 가리킨다 —
        // 총량에는 들어가지만 팩에는 담기지 않는다.
        let bigrams = vec![(0, 1, 8), (0, 2, 2), (7, 8, 990)];
        let lexicon = vec![
            ("the".to_string(), 100u32),
            ("quick".to_string(), 50),
            ("fox".to_string(), 50),
        ];
        let (result, report) = normalize_bigrams(
            &[SourceSignal {
                bigrams: &bigrams,
                ..corpus(Role::Frequency, &observed)
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
                ("the".to_string(), "fox".to_string(), 10880),
            ]
        );
    }
}
