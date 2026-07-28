//! mecab-ko-dic 형태소 사전.

use super::Signal;
use crate::lang::korean::{DERIVATIONAL_SUFFIXES, PARTICLES, has_final_consonant, particle_forms};
use crate::source::container;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// 조사가 붙은 어절은 홑 체언보다 한 단계 뒤에 오게 한다 — 같은 비용이면 사전 표제어인
/// 홑 형태가 먼저 제안되는 것이 자연스럽다.
const PARTICLE_COST_PENALTY: i64 = 200;

/// mecab-ko-dic CSV: `표층형,좌문맥,우문맥,비용,품사,의미부류,종성유무,…`. 비용이
/// 낮을수록 흔한 형태소라 비용 구간을 뒤집어 흔함 등급으로 옮긴다. 용언 어간 파일은
/// 종결어미 `다`를 붙여 기본형으로 만든다 — 용언 활용형 생성은 형태소 언어모델 단계의
/// 과제로 남아 있다.
/// 활용형 파일에서 받을 품사 조합의 첫 태그 — 용언만. 조사·서술격조사 결합은 홀로
/// 쓰이는 어절이 아니라 분석 결과일 뿐이다(`가` = JKS+NP).
const INFLECTED_PREDICATES: [&str; 4] = ["VV", "VA", "VX", "VCN"];

pub fn parse(
    path: &Path,
    files: &[String],
    verb_stem_files: &[String],
    inflection_files: &[String],
    particle_expansion_nouns: usize,
) -> Result<Signal, String> {
    let mut costs: HashMap<String, i64> = HashMap::new();
    let mut stems: Vec<String> = Vec::new();
    // 조사를 붙일 후보 체언 — (표층형, 비용)
    let mut nouns: Vec<(String, i64)> = Vec::new();
    let mut archive = container::open_tar_gz(path)?;
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
        let is_inflection = inflection_files.iter().any(|wanted| wanted == file_name);
        if !is_verb_stem && !is_inflection && !files.iter().any(|wanted| wanted == file_name) {
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
            if is_inflection
                && !INFLECTED_PREDICATES
                    .iter()
                    .any(|tag| part_of_speech.starts_with(tag))
            {
                continue;
            }
            let word = if is_verb_stem {
                stems.push(surface.to_string());
                format!("{surface}다")
            } else {
                surface.to_string()
            };
            if !is_verb_stem
                && !is_inflection
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
            // "생각하다"류 파생 용언은 사전에 명사와 접미사로 나뉘어 있어 어간이 없다.
            // 활용형("생각해", "필요한")은 코퍼스에서 받아들이므로 어간만 알려 주면 된다.
            stems.extend(
                DERIVATIONAL_SUFFIXES
                    .iter()
                    .map(|suffix| format!("{noun}{suffix}")),
            );
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
    stems.sort_unstable();
    stems.dedup();
    Ok(Signal {
        attested: costs
            .into_iter()
            .map(|(word, cost)| {
                let rank = 1.0 - (cost - minimum) as f64 / span;
                // 최소 등급이 0이 되면 인벤토리에서 사라지므로 바닥을 남긴다
                (word, rank.max(0.01))
            })
            .collect(),
        stems,
        affixes: particle_forms(),
        ..Signal::default()
    })
}
