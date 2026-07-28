//! Tatoeba 문장 익스포트.

use super::Signal;
use super::corpus::CorpusCounts;
use crate::lang::LanguageProfile;
use crate::source::container;
use std::io::BufRead;
use std::path::Path;

/// Tatoeba 문장 익스포트(bzip2 TSV `식별자<TAB>언어<TAB>문장`).
pub fn parse(path: &Path, language: &str, minimum_count: u64) -> Result<Signal, String> {
    let reader = container::open(path)?;
    let cased = LanguageProfile::of(language).cased();

    let mut corpus = CorpusCounts::new();
    for line in reader.lines() {
        let line = line.map_err(|error| format!("{} 읽기 실패: {error}", path.display()))?;
        let Some(sentence) = line.split('\t').nth(2) else {
            continue;
        };
        corpus.read_sentence(sentence, cased);
    }
    if corpus.is_empty() {
        return Err(format!("{}: 문장을 읽지 못했음", path.display()));
    }
    Ok(corpus.finish(minimum_count, 1))
}
