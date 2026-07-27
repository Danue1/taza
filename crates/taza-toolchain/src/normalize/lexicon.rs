//! 낱말 점수표 — 어떤 낱말이 표제어가 되고 몇 점을 받는가.

use std::collections::{HashMap, HashSet};

use crate::lang::korean::InflectionStems;
use crate::recipe::{LexiconRules, Role};

use super::SourceSignal;
use super::scale::{logarithmic, quantize};

#[derive(Debug, Default)]
struct Accumulated {
    /// 인벤토리 원천이 준 흔함 등급의 가중 합
    rank: f64,
    /// 빈도 원천이 준 실사용 횟수의 가중 합
    count: f64,
    in_inventory: bool,
}

/// 인벤토리에 없지만 discovery 원천이 본 낱말 — 승격 판정을 기다리는 자리
#[derive(Debug, Default)]
struct Candidate {
    /// 가중치를 먹인 횟수. 승격되면 그대로 점수가 된다.
    count: f64,
    /// 가중치를 먹이지 않은 관측 횟수 — 문턱은 원천의 크기가 아니라 실제로 몇 번
    /// 나타났는지로 재야 판단이 원천 가중치와 뒤섞이지 않는다.
    observations: u64,
    /// 이 낱말을 본 discovery 원천의 수
    sources: usize,
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
    /// 사전에 없지만 코퍼스 증거로 표제어가 된 낱말 — 외래어·신어가 여기로 들어온다.
    /// 무엇이 들어왔는지는 사람이 눈으로 봐야 문턱을 조일지 풀지 판단할 수 있어 전부 남긴다.
    pub promoted_words: Vec<String>,
    /// 문턱을 넘지 못해 승격되지 않은 후보 수
    pub rejected_candidates: usize,
    /// 예산 바로 아래에서 잘린 낱말들 — 점수가 높을수록 실제로 쳐질 법한 말이다
    pub absent_words: Vec<String>,
    /// 원천별 기여 — 넘겨받은 순서 그대로다
    pub contributions: Vec<Contribution>,
}

/// 원천 하나가 팩에 실제로 무엇을 보탰는가.
///
/// 새 말뭉치를 꽂았을 때 그것이 사전을 바꿨는지는 전체 표제어 수만 봐서는 알 수 없다 —
/// 큰 코퍼스를 더해도 이미 있던 낱말만 다시 보고 있을 수 있다. 원천을 늘리는 일이 값을
/// 하는지 판단하려면 이 표가 있어야 한다.
#[derive(Debug)]
pub struct Contribution {
    /// 이 원천이 낸 낱말 가운데 팩 표제어가 된 것
    pub in_lexicon: usize,
    /// 이 원천이 처음 데려온 낱말 — 다른 어느 원천도 내지 않은 표제어
    pub unique_to_source: usize,
    /// 이 원천의 증거로 승격된 낱말
    pub promoted: usize,
}

/// 반환값은 (표제어, 정규화 점수)를 점수 내림차순·사전순으로 정렬한 목록이다.
pub fn normalize(
    signals: &[SourceSignal<'_>],
    rules: &LexiconRules,
) -> (Vec<(String, u32)>, NormalizeReport) {
    // 인벤토리 역할 원천이 하나라도 보증한 낱말이 있으면 표제어 집합이 닫힌다 — 그 밖의
    // 낱말은 활용형이거나 승격 문턱을 넘어야 들어온다.
    let has_inventory = signals
        .iter()
        .any(|signal| signal.role == Role::Inventory && !signal.attested.is_empty());
    let mut stems = InflectionStems::default();
    for signal in signals {
        for stem in signal.stems {
            stems.insert(stem);
        }
    }
    // 인벤토리를 먼저 다 세운다 — 뒤따르는 코퍼스가 "이 낱말이 사전에 있는가"를 물을 때
    // 원천을 적은 순서에 답이 달라지면 안 된다.
    let mut accumulated: HashMap<&str, Accumulated> = HashMap::new();
    for signal in signals {
        if signal.role != Role::Inventory {
            continue;
        }
        for (word, rank) in signal.attested {
            let entry = accumulated.entry(word).or_default();
            entry.in_inventory = true;
            entry.rank += rank * signal.weight;
        }
    }
    let inventory_size = accumulated.len();

    let mut accepted_inflections = 0usize;
    let mut candidates: HashMap<&str, Candidate> = HashMap::new();
    for signal in signals {
        // 사전은 낱말을 보증할 뿐 세지 않는다. 사전 원천도 `observed`를 낼 수는 있지만
        // (구 표제어의 이웃 짝을 재려면 구성 어절의 수가 있어야 한다) 그것은 표제어가
        // 사전에 한 번 실렸다는 사실이지 사람이 그만큼 쳤다는 뜻이 아니다. 낱말 점수에
        // 더하면 사전 등재 한 번이 코퍼스 수백 회 출현을 눌러 버린다.
        if signal.role == Role::Inventory {
            continue;
        }
        for (word, count) in signal.observed {
            let value = *count as f64;
            if let Some(entry) = accumulated.get_mut(word.as_str()) {
                entry.count += value * signal.weight;
                continue;
            }
            // 인벤토리가 있으면 그 밖의 낱말은 그냥 받지 않는다 — 코퍼스 잡음(오타·
            // 고유명사·외국어)이 사전에 스며드는 경로를 막는다. 예외는 둘이다: 알려진
            // 어간에서 뻗어 나온 활용형(accept_inflections)과, 증거가 문턱을 넘은
            // discovery 원천의 낱말(admission).
            if !has_inventory || (rules.accept_inflections && stems.grew(word)) {
                if has_inventory {
                    accepted_inflections += 1;
                }
                accumulated.entry(word).or_default().count += value * signal.weight;
                continue;
            }
            if signal.role == Role::Discovery {
                let candidate = candidates.entry(word).or_default();
                candidate.count += value * signal.weight;
                candidate.observations += *count;
                candidate.sources += 1;
            }
        }
    }

    // 조사·어미는 홀로 쓰이는 어절이 아니다. 코퍼스에는 이것만 남은 자리가 있어
    // ("{{일본어|平和}}라고"에서 마크업을 버린 뒤) 어떤 실제 낱말보다도 흔해 보인다.
    let affixes: HashSet<&str> = signals
        .iter()
        .flat_map(|signal| signal.affixes.iter().map(String::as_str))
        .collect();

    let mut promoted_words = Vec::new();
    let mut rejected_candidates = candidates.len();
    if let Some(admission) = &rules.admission {
        let mut promotable: Vec<(&str, &Candidate)> = candidates
            .iter()
            .filter(|(word, candidate)| {
                candidate.observations >= admission.minimum_count
                    && candidate.sources >= admission.minimum_sources
                    && word.chars().count() >= rules.minimum_word_length
                    && rules.character_set.accepts(word)
                    && !affixes.contains(**word)
            })
            .map(|(word, candidate)| (*word, candidate))
            .collect();
        // 자리가 한정돼 있으니 증거가 많은 것부터 — 그것이 곧 실제로 쳐질 법한 말이다
        promotable.sort_by(|left, right| {
            right
                .1
                .observations
                .cmp(&left.1.observations)
                .then_with(|| left.0.cmp(right.0))
        });
        promotable.truncate(admission.maximum);
        rejected_candidates -= promotable.len();
        for (word, candidate) in promotable {
            // rank가 0이므로 승격어는 같은 빈도의 사전 표제어보다 뒤에 선다
            accumulated.entry(word).or_default().count += candidate.count;
            promoted_words.push(word.to_string());
        }
    }

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
    let in_lexicon: HashSet<&str> = filtered.iter().map(|(word, _)| *word).collect();
    let contributions = contributions(signals, &in_lexicon, &promoted_words);
    let words = filtered
        .iter()
        .map(|(word, score)| (word.to_string(), quantize(*score, highest)))
        .collect();
    (
        words,
        NormalizeReport {
            inventory_size,
            observed_in_corpus,
            dropped_by_filter,
            dropped_by_budget,
            accepted_inflections,
            promoted_words,
            rejected_candidates,
            contributions,
            absent_words,
        },
    )
}

/// 원천별로 팩에 무엇을 보탰는지 센다. "처음 데려온 낱말"은 그 원천이 없었다면 표제어가
/// 못 되었을 것을 뜻하므로, 원천을 늘린 값어치가 여기 드러난다.
fn contributions(
    signals: &[SourceSignal<'_>],
    in_lexicon: &HashSet<&str>,
    promoted_words: &[String],
) -> Vec<Contribution> {
    let promoted: HashSet<&str> = promoted_words.iter().map(String::as_str).collect();
    let mut providers: HashMap<&str, usize> = HashMap::new();
    for signal in signals {
        for word in signal.words() {
            if in_lexicon.contains(word) {
                *providers.entry(word).or_default() += 1;
            }
        }
    }
    signals
        .iter()
        .map(|signal| {
            let mut contribution = Contribution {
                in_lexicon: 0,
                unique_to_source: 0,
                promoted: 0,
            };
            for word in signal.words() {
                if !in_lexicon.contains(word) {
                    continue;
                }
                contribution.in_lexicon += 1;
                if providers.get(word) == Some(&1) {
                    contribution.unique_to_source += 1;
                }
                if promoted.contains(word) {
                    contribution.promoted += 1;
                }
            }
            contribution
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::fixture::*;
    use super::*;
    use crate::recipe::{AdmissionRules, CharacterSet};
    use taza_engine::pack::lexicon::MAX_FREQUENCY;

    #[test]
    fn frequency_source_only_boosts_inventory_words() {
        let attested = vec![("the".to_string(), 0.9), ("theme".to_string(), 0.3)];
        let observed = vec![("theme".to_string(), 5000), ("qwertyish".to_string(), 9)];
        let (words, report) = normalize(
            &[inventory(&attested), corpus(Role::Frequency, &observed)],
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

    /// 사전에 없는 낱말은 discovery 원천이 문턱만큼 증거를 대야 표제어가 된다.
    #[test]
    fn admission_promotes_only_words_with_enough_evidence() {
        let attested = vec![("keyboard".to_string(), 0.9)];
        let news = vec![("podcast".to_string(), 60), ("zzzq".to_string(), 90)];
        let chat = vec![("podcast".to_string(), 50), ("meme".to_string(), 400)];
        let mut rules = rules(10);
        rules.admission = Some(AdmissionRules {
            minimum_count: 100,
            minimum_sources: 2,
            maximum: 10,
        });
        let (words, report) = normalize(
            &[
                inventory(&attested),
                corpus(Role::Discovery, &news),
                corpus(Role::Discovery, &chat),
            ],
            &rules,
        );
        assert_eq!(report.inventory_size, 1);
        // 두 원천이 합쳐 110번 봤다 — 어느 한쪽만으로는 문턱에 못 미친다
        assert_eq!(report.promoted_words, vec!["podcast".to_string()]);
        // 한 원천에서만 보인 낱말은 아무리 흔해도 그 원천의 버릇일 뿐이다
        assert_eq!(report.rejected_candidates, 2);
        assert!(words.iter().any(|(word, _)| word == "podcast"));
        for rejected in ["zzzq", "meme"] {
            assert!(!words.iter().any(|(word, _)| word == rejected));
        }
    }

    /// 조사·어미는 홀로 쓰이는 어절이 아니다 — 코퍼스에 아무리 흔해도 표제어가 아니다.
    #[test]
    fn admission_rejects_bare_affixes() {
        let attested = vec![("타자".to_string(), 0.9)];
        let observed = vec![("는".to_string(), 90000), ("팟캐스트".to_string(), 500)];
        let affixes = vec!["는".to_string(), "를".to_string()];
        let mut rules = rules(10);
        rules.minimum_word_length = 1;
        rules.character_set = CharacterSet::HangulSyllables;
        rules.admission = Some(AdmissionRules {
            minimum_count: 100,
            minimum_sources: 1,
            maximum: 10,
        });
        let (_, report) = normalize(
            &[
                SourceSignal {
                    affixes: &affixes,
                    ..inventory(&attested)
                },
                corpus(Role::Discovery, &observed),
            ],
            &rules,
        );
        assert_eq!(report.promoted_words, vec!["팟캐스트".to_string()]);
    }

    /// 사전이 낸 관측은 등재 사실이지 실사용 횟수가 아니다 — 낱말 점수에 더하면 사전에
    /// 한 번 실린 것이 코퍼스 수백 회 출현을 눌러 버린다.
    #[test]
    fn inventory_observations_do_not_score_words() {
        let attested = vec![("규격서".to_string(), 0.05), ("사람".to_string(), 0.05)];
        // 사전이 표제어를 훑으며 낸 관측 — 낱말마다 한 번씩
        let listing = vec![("규격서".to_string(), 1), ("사람".to_string(), 1)];
        let observed = vec![("사람".to_string(), 900)];
        let mut rules = rules(1);
        rules.character_set = CharacterSet::HangulSyllables;
        let (words, _) = normalize(
            &[
                SourceSignal {
                    observed: &listing,
                    ..inventory(&attested)
                },
                corpus(Role::Frequency, &observed),
            ],
            &rules,
        );
        // 예산이 하나뿐이면 코퍼스가 실제로 본 낱말이 남아야 한다
        assert_eq!(words, vec![("사람".to_string(), MAX_FREQUENCY)]);
    }

    /// 승격 규칙이 없으면 discovery 원천도 빈도만 보탠다 — 기존 팩의 동작이 그대로다.
    #[test]
    fn discovery_without_admission_only_boosts() {
        let attested = vec![("keyboard".to_string(), 0.9)];
        let observed = vec![("keyboard".to_string(), 500), ("podcast".to_string(), 900)];
        let (words, report) = normalize(
            &[inventory(&attested), corpus(Role::Discovery, &observed)],
            &rules(10),
        );
        assert!(report.promoted_words.is_empty());
        assert_eq!(report.observed_in_corpus, 1);
        assert_eq!(words, vec![("keyboard".to_string(), MAX_FREQUENCY)]);
    }

    #[test]
    fn applies_filters_and_budget() {
        let attested = vec![
            ("a".to_string(), 1.0),
            ("na\u{ef}ve".to_string(), 0.8),
            ("keyboard".to_string(), 0.6),
            ("language".to_string(), 0.4),
        ];
        let (words, report) = normalize(&[inventory(&attested)], &rules(1));
        // 한 글자와 문자 집합을 벗어난 낱말이 걸러지고, 예산이 나머지를 자른다
        assert_eq!(report.dropped_by_filter, 2);
        assert_eq!(report.dropped_by_budget, 1);
        assert_eq!(words, vec![("keyboard".to_string(), MAX_FREQUENCY)]);
    }
}
