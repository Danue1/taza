//! 손으로 갖춘 곁들임 목록 — `곁들일것<TAB>낱말,낱말,…`.
//!
//! ```text
//! (^_^)   웃음,미소,기쁨
//! orz     절망,좌절
//! ```
//! (위 보기의 빈칸은 탭 하나다)
//!
//! 얼굴 문자처럼 공개 원천이 없는 갈래를 싣는 통로다. 갈래는 레시피가 밝히므로 이 파서는
//! 파일에서 짝만 읽는다. 어절 하나로 치는 낱말만 받는다 — CLDR 주석과 같은 기준이다.

use super::{Annotation, Signal};
use crate::source::container;
use std::io::BufRead;
use std::path::Path;
use taza_engine::contract::CandidateGroup;

pub fn parse(path: &Path, group: CandidateGroup) -> Result<Signal, String> {
    let mut annotations = Vec::new();
    container::for_each_member(path, |name, reader| {
        for line in reader.lines() {
            let line = line.map_err(|error| format!("{name} 읽기 실패: {error}"))?;
            let Some((text, words)) = entry(line.trim()) else {
                continue;
            };
            for word in words {
                annotations.push(Annotation {
                    word,
                    group,
                    text: text.to_string(),
                });
            }
        }
        Ok(())
    })?;
    if annotations.is_empty() {
        return Err(format!("{}: 곁들임 목록을 읽지 못했음", path.display()));
    }
    Ok(Signal {
        annotations,
        ..Signal::default()
    })
}

fn entry(line: &str) -> Option<(&str, Vec<String>)> {
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let (text, words) = line.split_once('\t')?;
    let words: Vec<String> = words
        .split(',')
        .map(str::trim)
        .filter(|word| !word.is_empty() && !word.contains(char::is_whitespace))
        .map(str::to_string)
        .collect();
    (!text.is_empty() && !words.is_empty()).then_some((text, words))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_text_and_its_words() {
        let (text, words) = entry("(^_^)\t웃음, 미소,기쁨").unwrap();
        assert_eq!(text, "(^_^)");
        assert_eq!(words, vec!["웃음", "미소", "기쁨"]);
    }

    #[test]
    fn comments_and_broken_lines_are_skipped() {
        assert!(entry("# 주석").is_none());
        assert!(entry("").is_none());
        assert!(entry("(^_^)").is_none());
        assert!(entry("\t웃음").is_none());
        // 띄어 쓴 이름은 어절이 아니다
        assert!(entry("orz\t깊은 절망").is_none());
    }
}
