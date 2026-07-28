//! 줄 단위 낱말·용어 목록.

use super::Signal;
use super::corpus::CorpusCounts;
use crate::source::container;
use std::collections::HashMap;
use std::io::BufRead;
use std::path::Path;

/// 표제어에서 떼어 낼 사전 표기. 붙임표는 형태소 경계를, 어깨점은 발음 구분을 나타내는
/// 표기일 뿐 실제로 치는 글자가 아니다. 구 경계 `^`는 뜻이 달라 따로 다룬다.
const DICTIONARY_MARKS: [char; 2] = ['-', 'ㆍ'];

/// 전문용어 표기의 구 경계. 사전에서 이 자리는 **붙여 써도 되는** 띄어쓰기를 뜻한다
/// (`빅^테크`, `규제^당국`). 그냥 공백으로 바꿔 어절만 내면 사람이 실제로 치는 형태를
/// 놓친다 — "빅테크"를 치는 사람이 "빅"과 "테크"를 따로 치지는 않는다. 띄어 쓴 어절과
/// 붙여 쓴 형태를 모두 표제어로 낸다.
const PHRASE_BOUNDARY: char = '^';

/// 줄 단위 낱말·용어 목록 (`낱말` 또는 `낱말<TAB>빈도`).
///
/// 사전·용어집·신어 자료는 배포 형식이 저마다 다르고 상당수가 손으로 받아야 한다.
/// 원천마다 파서를 늘리는 대신 사람이 한 번 이 꼴로 뽑아 두게 하면, 새 어휘를 들이는
/// 일이 파일 하나를 떨구는 일이 된다.
///
/// 전문용어는 대부분 구(句)다(`규제^당국`). 어절 사전에 통째로 담을 수 없으므로 어절로
/// 쪼개 각각 표제어로 내고, 이웃한 어절 짝을 문맥으로 낸다 — 용어가 아는 것의 절반은
/// "이 말 다음에 저 말이 온다"이고, 버리면 그 절반을 버리는 것이다. 구 경계가 붙여 쓰기를
/// 허용하는 자리라면(`^`) 붙여 쓴 형태도 함께 낸다.
pub fn parse(path: &Path, rank: f64, minimum_count: u64) -> Result<Signal, String> {
    let mut attested: HashMap<String, f64> = HashMap::new();
    let mut corpus = CorpusCounts::new();
    container::for_each_member(path, |name, reader| {
        for line in reader.lines() {
            let line = line.map_err(|error| format!("{name} 읽기 실패: {error}"))?;
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (entry, repetitions) = match line.split_once('\t') {
                Some((entry, count)) => match count.trim().parse::<u64>() {
                    Ok(count) if count < minimum_count => continue,
                    Ok(count) => (entry.trim(), count),
                    Err(_) => (entry.trim(), 1),
                },
                None => (line, 1),
            };
            let marked = entry.replace(DICTIONARY_MARKS, "");
            let cleaned = marked.replace(PHRASE_BOUNDARY, " ");
            if cleaned.trim().is_empty() {
                continue;
            }
            let mut headword = |word: &str| {
                let slot = attested.entry(word.to_string()).or_insert(0.0);
                *slot = slot.max(rank);
            };
            for eojeol in cleaned.split_whitespace() {
                headword(eojeol);
            }
            // 구 경계가 있었다면 붙여 쓴 형태도 옳은 표기다 — 오히려 그쪽이 더 흔히 쳐진다.
            // 원래 공백으로 나뉘어 있던 구는 붙여 쓸 근거가 없으므로 만들지 않는다.
            if marked.contains(PHRASE_BOUNDARY) {
                let joined: String = marked.replace(PHRASE_BOUNDARY, "");
                if !joined.contains(char::is_whitespace) {
                    headword(&joined);
                }
            }
            // 어절 빈도와 결합은 코퍼스와 같은 방식으로 센다. 목록에 여러 번 오른 용어는
            // 그만큼 흔한 것이므로 횟수를 그대로 되풀이해 반영한다.
            for _ in 0..repetitions {
                corpus.read_sentence(&cleaned, false);
            }
        }
        Ok(())
    })?;
    if attested.is_empty() {
        return Err(format!("{}: 낱말이 없음", path.display()));
    }
    Ok(Signal {
        attested: attested.into_iter().collect(),
        ..corpus.finish(1, 1)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_temporary(name: &str, content: &str) -> std::path::PathBuf {
        let directory = std::env::temp_dir().join("taza-extract-test");
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn word_list_reads_bare_and_counted_lines() {
        let path = write_temporary("words.tsv", "# 주석\n밈\n\n브이로그\t40\n오탈자\t1\n");
        let signal = parse(&path, 0.5, 10).unwrap();
        let mut attested = signal.attested;
        attested.sort_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(
            attested,
            vec![("밈".to_string(), 0.5), ("브이로그".to_string(), 0.5)]
        );
        // 목록에 오른 횟수가 곧 관측 횟수다
        let observed: HashMap<String, u64> = signal.observed.into_iter().collect();
        assert_eq!(observed.get("밈"), Some(&1));
        assert_eq!(observed.get("브이로그"), Some(&40));
        assert_eq!(observed.get("오탈자"), None);
        std::fs::remove_file(&path).unwrap();
    }

    /// 전문용어는 대부분 구다 — 어절로 쪼개 표제어로 내고, 이웃 짝은 문맥으로 낸다.
    #[test]
    fn word_list_splits_phrases_into_eojeol_and_context() {
        let path = write_temporary("terms.tsv", "규제^당국\n가-까이\n");
        let signal = parse(&path, 0.4, 1).unwrap();
        let attested: HashMap<String, f64> = signal.attested.into_iter().collect();
        assert_eq!(attested.get("규제"), Some(&0.4));
        assert_eq!(attested.get("당국"), Some(&0.4));
        // 붙임표는 실제로 치는 글자가 아니다
        assert_eq!(attested.get("가까이"), Some(&0.4));
        let place = |word: &str| {
            signal
                .observed
                .iter()
                .position(|(observed, _)| observed == word)
                .expect("관측 목록에 없음") as u32
        };
        assert_eq!(signal.bigrams, vec![(place("규제"), place("당국"), 1)]);
        std::fs::remove_file(&path).unwrap();
    }

    /// 구 경계 `^`는 붙여 써도 되는 자리다. 사람이 실제로 치는 것은 붙여 쓴 쪽인 경우가
    /// 많으므로("빅테크") 어절만 내고 말면 정작 쳐지는 형태가 사전에 없다.
    #[test]
    fn phrase_boundary_also_yields_the_joined_spelling() {
        let path = write_temporary("joined.tsv", "빅^테크\n정보 기술\n");
        let signal = parse(&path, 0.4, 1).unwrap();
        let attested: HashMap<String, f64> = signal.attested.into_iter().collect();
        assert_eq!(attested.get("빅테크"), Some(&0.4));
        assert_eq!(attested.get("빅"), Some(&0.4));
        assert_eq!(attested.get("테크"), Some(&0.4));
        // 원래 공백이던 구는 붙여 쓸 근거가 없다
        assert_eq!(attested.get("정보기술"), None);
        std::fs::remove_file(&path).unwrap();
    }
}
