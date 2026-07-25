//! 원천 형식별 추출기 — 내려받은 파일에서 (표제어, 신호)를 뽑는다.
//! 신호의 뜻은 원천의 역할이 정한다: 인벤토리 원천은 0보다 크고 1 이하인 흔함 등급,
//! 빈도 원천은 실사용 횟수. 병합·정규화는 `normalize`가 맡는다.

use crate::recipe::Extraction;
use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

/// 한 원천에서 뽑은 신호. 낱말 목록 원천은 문맥을 주지 못하므로 bigram이 비어 있다.
#[derive(Debug, Default)]
pub struct Extracted {
    /// (표제어, 신호)
    pub words: Vec<(String, f64)>,
    /// (앞말, 뒷말, 관측 횟수) — 문장 코퍼스만 낸다
    pub bigrams: Vec<(String, String, u64)>,
}

impl Extracted {
    fn words_only(words: Vec<(String, f64)>) -> Self {
        Extracted {
            words,
            bigrams: Vec::new(),
        }
    }
}

pub fn extract(extraction: &Extraction, path: &Path, language: &str) -> Result<Extracted, String> {
    match extraction {
        Extraction::Scowl {
            dialects,
            categories,
            maximum_size,
        } => scowl(path, dialects, categories, *maximum_size).map(Extracted::words_only),
        Extraction::Tatoeba { minimum_count } => tatoeba(path, language, *minimum_count),
        Extraction::MecabKoDic {
            files,
            verb_stem_files,
            particle_expansion_nouns,
        } => mecab_ko_dic(path, files, verb_stem_files, *particle_expansion_nouns)
            .map(Extracted::words_only),
    }
}

fn open_tar_gz(path: &Path) -> Result<tar::Archive<GzDecoder<std::fs::File>>, String> {
    let file = std::fs::File::open(path)
        .map_err(|error| format!("{} 열기 실패: {error}", path.display()))?;
    Ok(tar::Archive::new(GzDecoder::new(file)))
}

/// SCOWL 배포본의 `final/<방언>-<범주>.<크기>`. 크기 등급은 낮을수록 흔한 낱말이라
/// 그대로 뒤집어 흔함 등급으로 쓴다 — 별도 빈도 원천 없이도 랭킹의 골격이 선다.
fn scowl(
    path: &Path,
    dialects: &[String],
    categories: &[String],
    maximum_size: u32,
) -> Result<Vec<(String, f64)>, String> {
    let mut ranked: HashMap<String, f64> = HashMap::new();
    let mut archive = open_tar_gz(path)?;
    let entries = archive
        .entries()
        .map_err(|error| format!("{} 목록 읽기 실패: {error}", path.display()))?;
    for entry in entries {
        let mut entry = entry.map_err(|error| format!("항목 읽기 실패: {error}"))?;
        let entry_path = entry
            .path()
            .map_err(|error| format!("항목 경로 읽기 실패: {error}"))?
            .to_path_buf();
        let mut components = entry_path.components().rev();
        let Some(file_name) = components.next().and_then(|part| part.as_os_str().to_str()) else {
            continue;
        };
        let is_final = components
            .next()
            .and_then(|part| part.as_os_str().to_str())
            .is_some_and(|directory| directory == "final");
        if !is_final {
            continue;
        }
        let Some((stem, size)) = file_name.rsplit_once('.') else {
            continue;
        };
        let Ok(size) = size.parse::<u32>() else {
            continue;
        };
        let Some((dialect, category)) = stem.split_once('-') else {
            continue;
        };
        if size > maximum_size
            || !dialects.iter().any(|allowed| allowed == dialect)
            || !categories.iter().any(|allowed| allowed == category)
        {
            continue;
        }
        // SCOWL 목록은 ISO-8859-1 — 악센트 글자는 바이트 그대로 코드포인트로 옮긴 뒤
        // 문자 집합 필터에서 걸러진다.
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .map_err(|error| format!("{file_name} 읽기 실패: {error}"))?;
        let rank = 1.0 - (size as f64 / (maximum_size + 10) as f64);
        for line in bytes.split(|&byte| byte == b'\n') {
            let word: String = line
                .iter()
                .map(|&byte| char::from(byte))
                .collect::<String>()
                .trim()
                .to_string();
            if word.is_empty() {
                continue;
            }
            let slot = ranked.entry(word).or_insert(0.0);
            *slot = slot.max(rank);
        }
    }
    if ranked.is_empty() {
        return Err(format!("{}: 조건에 맞는 SCOWL 목록이 없음", path.display()));
    }
    Ok(ranked.into_iter().collect())
}

/// Tatoeba 문장 익스포트(bzip2 TSV `식별자<TAB>언어<TAB>문장`)에서 낱말 빈도와 이웃한
/// 낱말 짝을 센다.
/// 대소문자가 있는 스크립트에서는 소문자로 나타난 출현만 센다 — Tatoeba는 예문 인물
/// 이름("Tom")이 극단적으로 흔해서 대문자 출현을 함께 세면 이름이 흔한 낱말을 밀어내고
/// 상위권을 차지한다. 문장 첫머리 출현을 잃는 대신(흔한 낱말은 문장 중간에도 충분히
/// 나타난다) 고유명사 편향이 사라진다. 걸러진 낱말은 짝의 사슬도 끊는다 — 건너뛰어
/// 이으면 실제로 이웃하지 않은 낱말이 문맥으로 둔갑한다.
fn tatoeba(path: &Path, language: &str, minimum_count: u64) -> Result<Extracted, String> {
    let file = std::fs::File::open(path)
        .map_err(|error| format!("{} 열기 실패: {error}", path.display()))?;
    let reader = BufReader::new(BzDecoder::new(file));
    let cased = language == "en";

    let mut counts: HashMap<String, u64> = HashMap::new();
    let mut pairs: HashMap<(String, String), u64> = HashMap::new();
    for line in reader.lines() {
        let line = line.map_err(|error| format!("{} 읽기 실패: {error}", path.display()))?;
        let Some(sentence) = line.split('\t').nth(2) else {
            continue;
        };
        let mut previous: Option<&str> = None;
        for token in
            sentence.split(|character: char| !(character.is_alphabetic() || character == '\''))
        {
            if token.is_empty() || (cased && token.chars().next().is_some_and(char::is_uppercase)) {
                previous = None;
                continue;
            }
            *counts.entry(token.to_string()).or_insert(0) += 1;
            if let Some(left) = previous {
                *pairs
                    .entry((left.to_string(), token.to_string()))
                    .or_insert(0) += 1;
            }
            previous = Some(token);
        }
    }
    if counts.is_empty() {
        return Err(format!("{}: 문장을 읽지 못했음", path.display()));
    }
    Ok(Extracted {
        words: counts
            .into_iter()
            .filter(|(_, count)| *count >= minimum_count)
            .map(|(word, count)| (word, count as f64))
            .collect(),
        bigrams: pairs
            .into_iter()
            .map(|((left, right), count)| (left, right, count))
            .collect(),
    })
}

/// 체언에 붙는 조사 — 앞말의 종성 유무로 이형태가 갈리는 것은 (종성 있음, 없음) 짝으로
/// 적는다. 교착어의 어절은 사전에 통째로 담으면 폭발하므로, 흔한 체언에만 이 목록을
/// 붙여 예산 안에서 실제로 타이핑되는 어절을 덮는다.
const PARTICLES: [(&str, &str); 22] = [
    ("은", "는"),
    ("이", "가"),
    ("을", "를"),
    ("과", "와"),
    ("으로", "로"),
    ("이나", "나"),
    ("이라", "라"),
    ("이야", "야"),
    ("이다", "다"),
    ("이에요", "예요"),
    ("입니다", "입니다"),
    ("도", "도"),
    ("만", "만"),
    ("에", "에"),
    ("에서", "에서"),
    ("에게", "에게"),
    ("한테", "한테"),
    ("까지", "까지"),
    ("부터", "부터"),
    ("처럼", "처럼"),
    ("보다", "보다"),
    ("의", "의"),
];

/// 조사가 붙은 어절은 홑 체언보다 한 단계 뒤에 오게 한다 — 같은 비용이면 사전 표제어인
/// 홑 형태가 먼저 제안되는 것이 자연스럽다.
const PARTICLE_COST_PENALTY: i64 = 200;

fn has_final_consonant(word: &str) -> Option<bool> {
    let last = word.chars().next_back()?;
    if !('가'..='힣').contains(&last) {
        return None;
    }
    Some(!(last as u32 - '가' as u32).is_multiple_of(28))
}

/// mecab-ko-dic CSV: `표층형,좌문맥,우문맥,비용,품사,의미부류,종성유무,…`. 비용이
/// 낮을수록 흔한 형태소라 비용 구간을 뒤집어 흔함 등급으로 옮긴다. 용언 어간 파일은
/// 종결어미 `다`를 붙여 기본형으로 만든다 — 용언 활용형 생성은 형태소 언어모델 단계의
/// 과제로 남아 있다.
fn mecab_ko_dic(
    path: &Path,
    files: &[String],
    verb_stem_files: &[String],
    particle_expansion_nouns: usize,
) -> Result<Vec<(String, f64)>, String> {
    let mut costs: HashMap<String, i64> = HashMap::new();
    // 조사를 붙일 후보 체언 — (표층형, 비용)
    let mut nouns: Vec<(String, i64)> = Vec::new();
    let mut archive = open_tar_gz(path)?;
    let entries = archive
        .entries()
        .map_err(|error| format!("{} 목록 읽기 실패: {error}", path.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("항목 읽기 실패: {error}"))?;
        let entry_path = entry
            .path()
            .map_err(|error| format!("항목 경로 읽기 실패: {error}"))?
            .to_path_buf();
        let Some(file_name) = entry_path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let is_verb_stem = verb_stem_files.iter().any(|wanted| wanted == file_name);
        if !is_verb_stem && !files.iter().any(|wanted| wanted == file_name) {
            continue;
        }
        let reader = BufReader::new(entry);
        for line in reader.lines() {
            let line = line.map_err(|error| format!("{file_name} 읽기 실패: {error}"))?;
            let fields: Vec<&str> = line.split(',').collect();
            let [surface, _, _, cost, part_of_speech, ..] = fields.as_slice() else {
                continue;
            };
            let Ok(cost) = cost.parse::<i64>() else {
                continue;
            };
            let word = if is_verb_stem {
                format!("{surface}다")
            } else {
                surface.to_string()
            };
            if !is_verb_stem
                && particle_expansion_nouns > 0
                && (part_of_speech.starts_with("NN")
                    || part_of_speech.starts_with("NP")
                    || part_of_speech.starts_with("NR"))
            {
                nouns.push((word.clone(), cost));
            }
            let slot = costs.entry(word).or_insert(cost);
            *slot = (*slot).min(cost);
        }
    }

    if particle_expansion_nouns > 0 {
        nouns.sort_by(|left, right| left.1.cmp(&right.1).then_with(|| left.0.cmp(&right.0)));
        nouns.dedup_by(|left, right| left.0 == right.0);
        nouns.truncate(particle_expansion_nouns);
        for (noun, cost) in &nouns {
            let Some(final_consonant) = has_final_consonant(noun) else {
                continue;
            };
            for (after_consonant, after_vowel) in PARTICLES {
                let particle = if final_consonant {
                    after_consonant
                } else {
                    after_vowel
                };
                let combined = format!("{noun}{particle}");
                // 사전에 이미 있는 낱말이면 그 비용을 지키고, 없을 때만 결합형을 보탠다
                costs
                    .entry(combined)
                    .or_insert(cost + PARTICLE_COST_PENALTY);
            }
        }
    }
    if costs.is_empty() {
        return Err(format!("{}: 조건에 맞는 CSV가 없음", path.display()));
    }
    let minimum = costs.values().copied().min().unwrap();
    let maximum = costs.values().copied().max().unwrap();
    let span = (maximum - minimum).max(1) as f64;
    Ok(costs
        .into_iter()
        .map(|(word, cost)| {
            let rank = 1.0 - (cost - minimum) as f64 / span;
            // 최소 등급이 0이 되면 인벤토리에서 사라지므로 바닥을 남긴다
            (word, rank.max(0.01))
        })
        .collect())
}
