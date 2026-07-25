//! MediaWiki XML 덤프 — 본문 문서에서 낱말 빈도와 이웃 짝을 센다.

use super::Signal;
use super::corpus::{CorpusCounts, DISCARDED_MARK};
use crate::lang::LanguageProfile;
use bzip2::read::BzDecoder;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// MediaWiki XML 덤프(bzip2)의 본문 문서에서 낱말 빈도와 이웃 짝을 센다.
///
/// 문장 코퍼스보다 훨씬 크지만 문어체 쪽으로 기울어 있다 — 구어체 원천과 함께 쓰고
/// weight로 균형을 잡는 것을 전제한 원천이다.
pub fn parse(path: &Path, language: &str, minimum_count: u64) -> Result<Signal, String> {
    let file = std::fs::File::open(path)
        .map_err(|error| format!("{} 열기 실패: {error}", path.display()))?;
    let reader = BufReader::new(BzDecoder::new(file));
    let cased = LanguageProfile::of(language).cased();

    let mut corpus = CorpusCounts::default();
    let mut article = false;
    let mut wikitext = String::new();
    let mut collecting = false;
    for line in reader.lines() {
        let line = line.map_err(|error| format!("{} 읽기 실패: {error}", path.display()))?;
        let trimmed = line.trim_start();
        if trimmed.starts_with("<page>") {
            article = false;
            collecting = false;
            wikitext.clear();
        }
        // 본문 이름공간(0)만 — 분류·틀·토론 문서는 사람이 치는 글이 아니다
        if let Some(rest) = trimmed.strip_prefix("<ns>") {
            article = rest.starts_with("0<");
        }
        // 넘겨주기 문서는 본문이 아니라 한 줄짜리 지시문이다 — 그대로 세면
        // "넘겨주기"가 흔한 낱말 자리를 차지한다
        if trimmed.starts_with("<redirect") {
            article = false;
        }
        if !article {
            continue;
        }
        if !collecting {
            let Some(opening) = line.find("<text") else {
                continue;
            };
            let Some(close) = line[opening..].find('>') else {
                continue;
            };
            // `<text … />`는 빈 문서다
            if line[opening..opening + close].ends_with('/') {
                continue;
            }
            collecting = true;
            wikitext.push_str(&line[opening + close + 1..]);
        } else {
            wikitext.push('\n');
            wikitext.push_str(&line);
        }
        let Some(end) = wikitext.find("</text>") else {
            continue;
        };
        wikitext.truncate(end);
        // 문장 경계에서 끊어 센다 — 통째로 넣으면 마침표를 건너뛴 짝이 문맥으로 둔갑한다
        for sentence in strip_wikitext(&wikitext).split(['.', '!', '?', '\n']) {
            corpus.read_sentence(sentence, cased);
        }
        collecting = false;
        article = false;
        wikitext.clear();
        corpus.prune_if_large();
    }
    if corpus.counts.is_empty() {
        return Err(format!("{}: 본문 문서를 읽지 못했음", path.display()));
    }
    Ok(corpus.finish(minimum_count))
}

/// 통째로 버릴 구간의 (여는 표지, 닫는 표지). 어느 것도 사람이 쓴 문장이 아니라
/// 문서를 짜는 장치라서, 그대로 세면 "웹인용"·"분류"·"섬네일" 같은 말이 상위권을
/// 차지한다. 문서 이름공간 접두는 한국어판·영어판 양쪽 표기를 함께 적는다.
const DISCARDED_MARKUP: [(&str, &str); 6] = [
    ("{|", "|}"),
    ("[[파일:", "]]"),
    ("[[File:", "]]"),
    ("[[분류:", "]]"),
    ("[[Category:", "]]"),
    ("[[Image:", "]]"),
];

/// wikitext에서 본문 글만 남긴다. 마크업 구간은 통째로 버리고 링크는 표시 문자열만
/// 남긴다. 버린 자리에는 표지를 넣어 낱말 사슬을 끊는다 — 마크업을 사이에 두고
/// 떨어져 있던 낱말이 이웃으로 둔갑하지 않게.
fn strip_wikitext(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut stripped = String::with_capacity(text.len());
    let mut index = 0usize;
    while index < bytes.len() {
        let rest = &text[index..];
        // `== 외부 링크 ==` 같은 절 제목은 문서마다 똑같이 되풀이되는 정형구다
        if (index == 0 || bytes[index - 1] == b'\n') && rest.starts_with("==") {
            let line_end = rest.find('\n').unwrap_or(rest.len());
            stripped.push_str("\n\n");
            index += line_end;
            continue;
        }
        if let Some(skipped) = skip_nested(rest, "{{", "}}")
            .or_else(|| {
                DISCARDED_MARKUP
                    .iter()
                    .find_map(|&(open, close)| skip_nested(rest, open, close))
            })
            .or_else(|| {
                skip_until(rest, "<ref", "</ref>").or_else(|| skip_until(rest, "<ref", "/>"))
            })
            .or_else(|| skip_until(rest, "<!--", "-->"))
        {
            stripped.push(DISCARDED_MARK);
            index += skipped;
            continue;
        }
        // `[[대상|표시]]`에서 대상은 문서 이름이라 본문 흐름의 낱말이 아니다. 세로줄이
        // 없으면 대상이 곧 화면에 보이는 글이므로 그대로 남긴다 — 대괄호를 남겨 두면
        // 뒤에 붙은 조사까지 딸린 어절이 통째로 버려진다.
        if rest.starts_with("[[")
            && let Some(close) = rest.find("]]")
        {
            let inner = &rest[2..close];
            stripped.push_str(inner.rsplit_once('|').map_or(inner, |(_, shown)| shown));
            index += close + 2;
            continue;
        }
        let character = rest.chars().next().unwrap_or('\n');
        stripped.push(character);
        index += character.len_utf8();
    }
    stripped
}

/// 여는 표지에서 시작해 짝이 맞는 닫는 표지까지의 길이. 중첩을 센다.
fn skip_nested(text: &str, open: &str, close: &str) -> Option<usize> {
    if !text.starts_with(open) {
        return None;
    }
    let mut depth = 0usize;
    let mut index = 0usize;
    while index < text.len() {
        if text[index..].starts_with(open) {
            depth += 1;
            index += open.len();
        } else if text[index..].starts_with(close) {
            depth -= 1;
            index += close.len();
            if depth == 0 {
                return Some(index);
            }
        } else {
            index += text[index..].chars().next().map_or(1, char::len_utf8);
        }
    }
    Some(text.len())
}

/// 여는 표지에서 시작해 처음 나오는 닫는 표지까지의 길이. 중첩되지 않는 구간용이다.
fn skip_until(text: &str, open: &str, close: &str) -> Option<usize> {
    if !text.starts_with(open) {
        return None;
    }
    Some(
        text.find(close)
            .map_or(text.len(), |offset| offset + close.len()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_templates_references_and_link_targets() {
        let wikitext = "{{인용|웹인용|확인날짜=2026}}한국은 {{나라}}동아시아에 있다.\
                        <ref name=\"출처\">참고 문헌</ref> 수도는 [[대한민국 서울|서울]]이다.";
        let stripped = strip_wikitext(wikitext);
        assert!(stripped.contains("한국은"));
        assert!(stripped.contains("동아시아에 있다"));
        assert!(stripped.contains("수도는 서울이다"));
        // 틀 인자·참조 본문·링크 대상은 본문 낱말이 아니다
        for dropped in ["웹인용", "확인날짜", "참고 문헌", "대한민국 서울"] {
            assert!(!stripped.contains(dropped), "남아 있음: {dropped}");
        }
    }

    #[test]
    fn nested_templates_are_skipped_whole() {
        let stripped = strip_wikitext("{{바깥{{안쪽}}더}}남는다");
        assert_eq!(stripped.trim_start_matches(DISCARDED_MARK), "남는다");
    }

    /// 마크업을 사이에 두고 떨어져 있던 낱말은 이웃이 아니다 — 버린 자리가 사슬을 끊는다.
    #[test]
    fn discarded_markup_breaks_the_neighbour_chain() {
        let mut corpus = CorpusCounts::default();
        corpus.read_sentence(&strip_wikitext("앞말 {{틀}} 뒷말"), false);
        assert_eq!(corpus.counts.get("앞말"), Some(&1));
        assert_eq!(corpus.counts.get("뒷말"), Some(&1));
        assert!(corpus.pairs.is_empty());
    }

    /// 어절에 딱 붙은 마크업은 그 어절을 통째로 버린다 — 남기면 조사만 홀로 살아남아
    /// 낱말로 둔갑한다.
    #[test]
    fn markup_inside_an_eojeol_discards_it() {
        let mut corpus = CorpusCounts::default();
        corpus.read_sentence(&strip_wikitext("{{일본어|平和}}라고 불렀다"), false);
        assert_eq!(corpus.counts.get("라고"), None);
        assert_eq!(corpus.counts.get("불렀다"), Some(&1));
    }

    /// 세로줄 없는 링크는 대상이 곧 화면에 보이는 글이다. 대괄호를 남기면 뒤에 붙은
    /// 조사까지 딸린 어절이 통째로 버려진다.
    #[test]
    fn bare_links_keep_their_text() {
        let mut corpus = CorpusCounts::default();
        corpus.read_sentence(&strip_wikitext("[[한국어]]는 아름답다"), false);
        assert_eq!(corpus.counts.get("한국어는"), Some(&1));
    }

    #[test]
    fn file_links_are_dropped_whole() {
        let stripped = strip_wikitext("[[파일:서울.jpg|섬네일|설명 글]]본문");
        assert_eq!(stripped.trim_start_matches(DISCARDED_MARK), "본문");
    }

    /// 절 제목과 분류 링크는 문서마다 똑같이 되풀이돼 흔한 낱말 자리를 빼앗는다.
    #[test]
    fn section_headings_and_categories_are_dropped() {
        let stripped = strip_wikitext("본문이다.\n== 외부 링크 ==\n주소\n[[분류:대한민국]]");
        assert!(stripped.contains("본문이다"));
        assert!(stripped.contains("주소"));
        for dropped in ["외부", "링크", "대한민국"] {
            assert!(!stripped.contains(dropped), "남아 있음: {dropped}");
        }
    }
}
