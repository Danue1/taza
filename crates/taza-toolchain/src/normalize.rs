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

pub struct SourceSignal<'call> {
    pub role: Role,
    pub weight: f64,
    pub entries: &'call [(String, f64)],
    /// (앞말, 뒷말, 관측 횟수) — 문장 코퍼스만 채운다
    pub bigrams: &'call [(String, String, u64)],
    /// 활용형이 뻗어 나오는 어간 — 형태소 사전만 채운다
    pub stems: &'call [String],
    /// 어절 뒤에 붙는 접사 — 형태소 사전만 채운다. 이것과 똑같은 낱말은 홀로 쓰이는
    /// 어절이 아니므로 승격 후보에서 뺀다.
    pub affixes: &'call [String],
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
}

/// 반환값은 (표제어, 정규화 점수)를 점수 내림차순·사전순으로 정렬한 목록이다.
pub fn normalize(
    signals: &[SourceSignal<'_>],
    rules: &LexiconRules,
) -> (Vec<(String, u32)>, NormalizeReport) {
    let has_inventory = signals.iter().any(|signal| signal.role == Role::Inventory);
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
        for (word, value) in signal.entries {
            let entry = accumulated.entry(word).or_default();
            entry.in_inventory = true;
            entry.rank += value * signal.weight;
        }
    }
    let inventory_size = accumulated.len();

    let mut accepted_inflections = 0usize;
    let mut candidates: HashMap<&str, Candidate> = HashMap::new();
    for signal in signals {
        if signal.role == Role::Inventory {
            continue;
        }
        for (word, value) in signal.entries {
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
                candidate.observations += *value as u64;
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
            promoted_words,
            rejected_candidates,
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

/// 어간 말모음이 어미와 축약돼 아예 다른 모음으로 나타나는 짝. 두벌식에서 두 키인
/// 축약(ㅗ+ㅏ→ㅘ, ㅜ+ㅓ→ㅝ)은 자모로 풀면 덧붙음으로 보여 이미 접두로 잡히므로,
/// 한 키짜리 모음으로 바뀌어 접두 관계가 끊기는 것만 적는다.
const VOWEL_CONTRACTIONS: [(char, &[char]); 3] = [
    ('ㅏ', &['ㅐ']),       // 하 + 여 → 해
    ('ㅣ', &['ㅕ']),       // 마시 + 어 → 마셔
    ('ㅡ', &['ㅓ', 'ㅏ']), // 쓰 + 어 → 써, 바쁘 + 아 → 바빠
];

/// 활용형을 알아보는 어간 색인. 축약형을 따로 두는 이유는 어절이 될 조건이 다르기
/// 때문이다 — 표층 어간은 어미가 붙어야 어절이 되지만("하"는 어절이 아니다),
/// 축약형은 이미 어미가 녹아든 형태라 그 자체로 어절이다("해", "미안해", "써").
#[derive(Default)]
struct InflectionStems {
    bare: HashSet<Vec<char>>,
    contracted: HashSet<Vec<char>>,
}

impl InflectionStems {
    fn insert(&mut self, stem: &str) {
        let key = inflection_key(stem);
        let Some(&last) = key.last() else {
            return;
        };
        for (vowel, contractions) in VOWEL_CONTRACTIONS {
            if vowel != last {
                continue;
            }
            for &replacement in contractions {
                let mut variant = key.clone();
                variant.pop();
                variant.push(replacement);
                self.contracted.insert(variant);
            }
        }
        self.bare.insert(key);
    }

    /// 알려진 어간에서 뻗어 나온 낱말인가 — 활용형으로 볼 수 있는 최소 조건이다.
    /// 한국어 용언 어간은 대개 한 음절(있·하·같)이라 길이로 더 조이면 정작 흔한
    /// 활용형이 다 걸러진다. 어간 집합이 용언 파일에서만 오므로(고유명사는 애초에
    /// 빠져 있다) 이 조건만으로도 잡음이 새는 길은 좁다.
    fn grew(&self, word: &str) -> bool {
        let key = inflection_key(word);
        (1..key.len()).any(|length| self.bare.contains(&key[..length]))
            || (1..=key.len()).any(|length| self.contracted.contains(&key[..length]))
    }
}

/// 실사용 횟수는 로그로 눌러 담는다 — 상위 몇 낱말이 점수 공간을 독점하지 않게.
fn logarithmic(count: f64) -> f64 {
    if count <= 0.0 {
        0.0
    } else {
        (1.0 + count).ln()
    }
}

/// 팩에 담는 점수의 눈금. 사전은 접미사가 같은 하위 그래프를 한 노드로 합쳐 저장하는데
/// (DAWG), 합칠 수 있는지는 끝 노드의 점수까지 같은지로 가린다. 점수를 65535단계로
/// 실으면 끝 노드가 거의 다 달라 공유가 막힌다 — 교착어처럼 접미사를 나눠 갖는 표제어가
/// 많을수록 손해가 크다.
///
/// 눈금을 이만큼 굵히면 한국어팩이 2188KB → 1542KB, 배포 아카이브가 1155KB → 715KB로
/// 줄면서 랭킹 지표는 그대로다(top1 0.972 / top3 0.999 / MRR 0.985 / 절약률 0.484,
/// 오교정률 0.000 모두 동일). 더 굵히면(1024, 4096) 더 줄지만 그때부터는 지표가 밀린다.
const SCORE_STEP: u32 = 256;

fn quantize(score: f64, highest: f64) -> u32 {
    let ratio = if highest > 0.0 {
        (score / highest).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let value = 1 + (ratio * (MAX_FREQUENCY - 1) as f64).round() as u32;
    ((value + SCORE_STEP / 2) / SCORE_STEP * SCORE_STEP).clamp(1, MAX_FREQUENCY)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe::{AdmissionRules, CharacterSet, LexiconEncoding};

    fn rules(max_words: usize) -> LexiconRules {
        LexiconRules {
            encoding: LexiconEncoding::Utf8,
            character_set: CharacterSet::LatinLowercase,
            max_words,
            minimum_word_length: 2,
            accept_inflections: false,
            admission: None,
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
                    affixes: &[],
                },
                SourceSignal {
                    role: Role::Frequency,
                    weight: 1.0,
                    entries: &corpus,
                    bigrams: &[],
                    stems: &[],
                    affixes: &[],
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
                affixes: &[],
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

    /// 사전에 없는 낱말은 discovery 원천이 문턱만큼 증거를 대야 표제어가 된다.
    #[test]
    fn admission_promotes_only_words_with_enough_evidence() {
        let inventory = vec![("keyboard".to_string(), 0.9)];
        let news = vec![("podcast".to_string(), 60.0), ("zzzq".to_string(), 90.0)];
        let chat = vec![("podcast".to_string(), 50.0), ("meme".to_string(), 400.0)];
        let signal = |role, entries| SourceSignal {
            role,
            weight: 1.0,
            entries,
            bigrams: &[],
            stems: &[],
            affixes: &[],
        };
        let mut rules = rules(10);
        rules.admission = Some(AdmissionRules {
            minimum_count: 100,
            minimum_sources: 2,
            maximum: 10,
        });
        let (words, report) = normalize(
            &[
                signal(Role::Inventory, &inventory),
                signal(Role::Discovery, &news),
                signal(Role::Discovery, &chat),
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
        let inventory = vec![("타자".to_string(), 0.9)];
        let corpus = vec![("는".to_string(), 90000.0), ("팟캐스트".to_string(), 500.0)];
        let affixes = vec!["는".to_string(), "를".to_string()];
        let mut rules = rules(10);
        rules.minimum_word_length = 1;
        rules.admission = Some(AdmissionRules {
            minimum_count: 100,
            minimum_sources: 1,
            maximum: 10,
        });
        rules.character_set = CharacterSet::HangulSyllables;
        let (_, report) = normalize(
            &[
                SourceSignal {
                    role: Role::Inventory,
                    weight: 1.0,
                    entries: &inventory,
                    bigrams: &[],
                    stems: &[],
                    affixes: &affixes,
                },
                SourceSignal {
                    role: Role::Discovery,
                    weight: 1.0,
                    entries: &corpus,
                    bigrams: &[],
                    stems: &[],
                    affixes: &[],
                },
            ],
            &rules,
        );
        assert_eq!(report.promoted_words, vec!["팟캐스트".to_string()]);
    }

    /// 승격 규칙이 없으면 discovery 원천도 빈도만 보탠다 — 기존 팩의 동작이 그대로다.
    #[test]
    fn discovery_without_admission_only_boosts() {
        let inventory = vec![("keyboard".to_string(), 0.9)];
        let corpus = vec![
            ("keyboard".to_string(), 500.0),
            ("podcast".to_string(), 900.0),
        ];
        let (words, report) = normalize(
            &[
                SourceSignal {
                    role: Role::Inventory,
                    weight: 1.0,
                    entries: &inventory,
                    bigrams: &[],
                    stems: &[],
                    affixes: &[],
                },
                SourceSignal {
                    role: Role::Discovery,
                    weight: 1.0,
                    entries: &corpus,
                    bigrams: &[],
                    stems: &[],
                    affixes: &[],
                },
            ],
            &rules(10),
        );
        assert!(report.promoted_words.is_empty());
        assert_eq!(report.observed_in_corpus, 1);
        assert_eq!(words, vec![("keyboard".to_string(), MAX_FREQUENCY)]);
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
                affixes: &[],
            }],
            &rules(1),
        );
        // 한 글자와 문자 집합을 벗어난 낱말이 걸러지고, 예산이 나머지를 자른다
        assert_eq!(report.dropped_by_filter, 2);
        assert_eq!(report.dropped_by_budget, 1);
        assert_eq!(words, vec![("keyboard".to_string(), MAX_FREQUENCY)]);
    }
}
