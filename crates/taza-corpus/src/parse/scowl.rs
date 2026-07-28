//! SCOWL 낱말 목록 배포본.

use crate::source::container;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

/// SCOWL 배포본의 `final/<방언>-<범주>.<크기>`. 크기 등급은 낮을수록 흔한 낱말이라
/// 그대로 뒤집어 흔함 등급으로 쓴다 — 별도 빈도 원천 없이도 랭킹의 골격이 선다.
pub fn parse(
    path: &Path,
    dialects: &[String],
    categories: &[String],
    maximum_size: u32,
) -> Result<Vec<(String, f64)>, String> {
    let mut ranked: HashMap<String, f64> = HashMap::new();
    let mut archive = container::open_tar_gz(path)?;
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
