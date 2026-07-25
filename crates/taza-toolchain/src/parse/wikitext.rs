//! MediaWiki XML 덤프 — 본문 문서에서 낱말 빈도와 이웃 짝을 센다.

use super::Signal;
use super::corpus::{CorpusCounts, DISCARDED_MARK, word_of};
use crate::lang::LanguageProfile;
use crate::source::container;
use std::io::BufRead;
use std::path::Path;

/// MediaWiki XML 덤프(bzip2)의 본문 문서에서 낱말 빈도와 이웃 짝을 센다.
///
/// 문장 코퍼스보다 훨씬 크지만 문어체 쪽으로 기울어 있다 — 구어체 원천과 함께 쓰고
/// weight로 균형을 잡는 것을 전제한 원천이다.
///
/// 덤프가 여러 스트림으로 나뉘어 있으면 **본문을 손질하는 일만** 실을 나눠 맡긴다.
/// 시간의 절반 이상이 압축 해제와 마크업 제거인데 그 일은 서로를 보지 않고, 세는 일은
/// 집계 표를 함께 봐야 한다 — 표를 실마다 두면 표가 실 수만큼 불어나 벌어 놓은 시간을
/// 도로 내놓는다. 그래서 손질은 여럿이, 세기는 하나가 한다.
pub fn parse(path: &Path, language: &str, minimum_count: u64) -> Result<Signal, String> {
    let cased = LanguageProfile::of(language).cased();
    // 한국어 덤프가 짝 6천만 종류를 낸다. 표를 키워 가며 옮겨 담는 일이 최대 메모리를
    // 정하므로 처음부터 그만한 자리를 잡는다.
    let mut corpus = CorpusCounts::expecting_pairs(64_000_000);
    let chunks = container::compressed_chunks(path)?;

    if chunks.len() < 2 {
        let mut reader = container::open(path)?;
        let mut pages = Pages::default();
        let mut buffer = String::new();
        let mut batch = String::new();
        loop {
            buffer.clear();
            let read = reader
                .read_line(&mut buffer)
                .map_err(|error| format!("{} 읽기 실패: {error}", path.display()))?;
            if read == 0 {
                break;
            }
            if let Some(stripped) = pages.read_line(&buffer) {
                batch.clear();
                select_words(stripped, cased, &mut batch);
                corpus.read_words(&batch);
            }
        }
    } else {
        count_chunks(path, &chunks, &mut corpus, cased)?;
    }

    if corpus.is_empty() {
        return Err(format!("{}: 본문 문서를 읽지 못했음", path.display()));
    }
    Ok(corpus.finish(minimum_count, 2))
}

/// 한 번에 넘길 본문의 크기. 어절 하나씩 넘기면 실 사이를 오가는 값이 나눠 맡긴 이득을
/// 먹는다. 문서 사이는 줄바꿈으로 가르므로 묶어 넘겨도 낱말 사슬은 문서를 넘지 않는다.
const BATCH: usize = 1 << 20;

/// 앞질러 손질해 둘 묶음 수. 세는 쪽이 밀리면 손질하는 쪽이 기다리게 해, 손질된 본문이
/// 메모리에 쌓이지 않게 한다.
const READ_AHEAD: usize = 8;

/// 덩이를 나눠 맡아 풀고 손질하고 어절까지 가려낸 뒤, 세는 일만 한 실에 모은다.
///
/// 표를 만지는 일은 갈라 놓을 수 없다 — 낱말 번호가 한 표에서 나와야 하고, 표를 갈래로
/// 나눠 잠가 보면 흔한 낱말이 한 갈래에 몰려 자물쇠에서 잃는 것이 더 크다(실측 21.7초 →
/// 25.1초). 그래서 표 앞의 일은 모두 나눠 맡기고, 표는 한 실이 지킨다.
fn count_chunks(
    path: &Path,
    chunks: &[(u64, u64)],
    corpus: &mut CorpusCounts,
    cased: bool,
) -> Result<(), String> {
    let strippers = std::thread::available_parallelism()
        .map_or(4, |count| count.get().saturating_sub(1).max(1))
        .min(chunks.len());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let (sender, receiver) = std::sync::mpsc::sync_channel::<String>(READ_AHEAD);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..strippers)
            .map(|_| {
                let (next, sender) = (&next, sender.clone());
                scope.spawn(move || -> Result<(), String> {
                    let mut pages = Pages::default();
                    let mut batch = String::with_capacity(BATCH + BATCH / 4);
                    loop {
                        let at = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(&(start, stop)) = chunks.get(at) else {
                            let _ = sender.send(batch);
                            return Ok(());
                        };
                        let decoded = container::decode_chunk(path, start, stop)
                            .map_err(|error| format!("{} 푸는 데 실패: {error}", path.display()))?;
                        for line in decoded.split_inclusive(|byte| *byte == b'\n') {
                            let Some(stripped) = pages.read_line(&String::from_utf8_lossy(line))
                            else {
                                continue;
                            };
                            select_words(stripped, cased, &mut batch);
                            if batch.len() >= BATCH {
                                let filled = std::mem::replace(
                                    &mut batch,
                                    String::with_capacity(BATCH + BATCH / 4),
                                );
                                if sender.send(filled).is_err() {
                                    // 세는 쪽이 사라졌다 — 더 손질할 까닭이 없다
                                    return Ok(());
                                }
                            }
                        }
                    }
                })
            })
            .collect();
        drop(sender);
        for batch in receiver {
            corpus.read_words(&batch);
            corpus.prune_if_large();
        }
        handles.into_iter().try_for_each(|handle| {
            handle
                .join()
                .unwrap_or_else(|_| Err("본문을 손질하다 말았음".to_string()))
        })
    })
}

/// 손질한 본문에서 셀 어절만 골라 담는다. 표를 만지는 일은 한 실이 해야 하지만 어절을
/// 가려내는 일은 그렇지 않으므로, 손질하는 실이 여기까지 해 둔다. 버린 어절과 문장
/// 경계는 줄바꿈으로 남겨 낱말 사슬을 끊는다.
fn select_words(stripped: &str, cased: bool, batch: &mut String) {
    for sentence in stripped.split(['.', '!', '?', '\n']) {
        for chunk in sentence.split_whitespace() {
            match word_of(chunk)
                .filter(|word| !cased || !word.chars().next().is_some_and(char::is_uppercase))
            {
                Some(word) => {
                    batch.push_str(word);
                    batch.push(' ');
                }
                None => batch.push('\n'),
            }
        }
        batch.push('\n');
    }
}

/// 본문이 끝나는 자리를 알리는 표지
const CLOSING: &str = "</text>";

/// 덤프의 줄을 차례로 받아 본문 문서를 손질해 내는 상태 기계. 문서 하나가 여러 줄에
/// 걸쳐 있어 줄 하나만 보고는 손질할 수 없다.
#[derive(Default)]
struct Pages {
    article: bool,
    collecting: bool,
    wikitext: String,
    /// 이미 닫는 표지를 찾아본 데까지의 길이
    scanned: usize,
    /// 손질한 본문을 담아 두는 자리 — 문서마다 새로 만들지 않는다
    stripped: String,
}

impl Pages {
    /// 줄 하나를 받아, 문서 한 편이 끝났으면 손질한 본문을 돌려준다.
    fn read_line(&mut self, buffer: &str) -> Option<&str> {
        let line = buffer.trim_end_matches(['\n', '\r']);
        let trimmed = line.trim_start();
        if trimmed.starts_with("<page>") {
            self.article = false;
            self.collecting = false;
            self.wikitext.clear();
            self.scanned = 0;
        }
        // 본문 이름공간(0)만 — 분류·틀·토론 문서는 사람이 치는 글이 아니다
        if let Some(rest) = trimmed.strip_prefix("<ns>") {
            self.article = rest.starts_with("0<");
        }
        // 넘겨주기 문서는 본문이 아니라 한 줄짜리 지시문이다 — 그대로 세면
        // "넘겨주기"가 흔한 낱말 자리를 차지한다
        if trimmed.starts_with("<redirect") {
            self.article = false;
        }
        if !self.article {
            return None;
        }
        if !self.collecting {
            let opening = line.find("<text")?;
            let close = line[opening..].find('>')?;
            // `<text … />`는 빈 문서다
            if line[opening..opening + close].ends_with('/') {
                return None;
            }
            self.collecting = true;
            self.wikitext.push_str(&line[opening + close + 1..]);
        } else {
            self.wikitext.push('\n');
            self.wikitext.push_str(line);
        }
        // 닫는 표지는 방금 붙인 자리에서만 찾는다 — 줄마다 문서를 처음부터 다시 훑으면
        // 긴 문서에서 훑는 양이 길이의 제곱으로 늘어난다. 표지가 줄 경계에 걸쳐 있을 수
        // 있으므로 표지 길이만큼 앞에서 시작한다.
        let scan_from = self.scanned.saturating_sub(CLOSING.len());
        let Some(end) = self.wikitext.as_bytes()[scan_from..]
            .windows(CLOSING.len())
            .position(|window| window == CLOSING.as_bytes())
            .map(|at| at + scan_from)
        else {
            self.scanned = self.wikitext.len();
            return None;
        };
        self.wikitext.truncate(end);
        strip_wikitext(&self.wikitext, &mut self.stripped);
        self.collecting = false;
        self.article = false;
        self.wikitext.clear();
        self.scanned = 0;
        Some(&self.stripped)
    }
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
fn strip_wikitext(text: &str, stripped: &mut String) {
    let bytes = text.as_bytes();
    stripped.clear();
    stripped.reserve(text.len());
    let mut index = 0usize;
    while index < bytes.len() {
        // 마크업이 시작될 수 있는 글자에서만 표지를 맞춰 본다. 본문의 대부분은 그런
        // 글자가 아니므로, 그 사이를 통째로 옮기는 것이 글자마다 표지 목록을 훑는
        // 것보다 훨씬 싸다 — 이 반복문이 파이프라인에서 가장 뜨거운 자리다.
        let plain = plain_run(&bytes[index..]);
        if plain > 0 {
            stripped.push_str(&text[index..index + plain]);
            index += plain;
            continue;
        }
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
}

/// 마크업이 시작될 수 있는 글자 — 이 앞까지는 본문 글이라 통째로 옮기면 된다.
fn is_markup_start(byte: u8) -> bool {
    matches!(byte, b'{' | b'[' | b'<' | b'=' | b'\n')
}

/// 마크업 표지가 나오기 전까지 이어지는 본문 글의 길이.
///
/// 덤프 5기가바이트가 이 함수를 지나가므로 한 글자씩 보고 바로 멈추는 대신, 한 덩이를
/// 통째로 훑어 표지가 하나라도 있는지부터 가른다. 멈추는 자리가 없는 반복문이라
/// 컴파일러가 벡터 명령으로 접을 수 있고, 표지가 든 덩이에서만 한 글자씩 본다.
fn plain_run(bytes: &[u8]) -> usize {
    const BLOCK: usize = 64;
    let mut at = 0;
    while at + BLOCK <= bytes.len() {
        let block = &bytes[at..at + BLOCK];
        let carries_markup = block
            .iter()
            .fold(false, |seen, &byte| seen | is_markup_start(byte));
        if carries_markup {
            break;
        }
        at += BLOCK;
    }
    at + bytes[at..]
        .iter()
        .position(|&byte| is_markup_start(byte))
        .unwrap_or(bytes.len() - at)
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

    fn stripped_text(wikitext: &str) -> String {
        let mut stripped = String::new();
        strip_wikitext(wikitext, &mut stripped);
        stripped
    }

    /// 덩이 단위로 훑는 길과 한 글자씩 보는 길이 같은 자리를 가리키는가.
    #[test]
    fn plain_run_stops_at_the_first_markup() {
        let cases = [
            "본문만 있는 글".repeat(20),
            format!("{}{{{{틀}}}}", "가".repeat(200)),
            format!("{}\n제목", "나".repeat(63)),
            "[[링크]]".to_string(),
            String::new(),
        ];
        for case in cases {
            let expected = case
                .as_bytes()
                .iter()
                .position(|&byte| is_markup_start(byte))
                .unwrap_or(case.len());
            assert_eq!(plain_run(case.as_bytes()), expected, "다름: {case:?}");
        }
    }

    #[test]
    fn strips_templates_references_and_link_targets() {
        let wikitext = "{{인용|웹인용|확인날짜=2026}}한국은 {{나라}}동아시아에 있다.\
                        <ref name=\"출처\">참고 문헌</ref> 수도는 [[대한민국 서울|서울]]이다.";
        let stripped = stripped_text(wikitext);
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
        let stripped = stripped_text("{{바깥{{안쪽}}더}}남는다");
        assert_eq!(stripped.trim_start_matches(DISCARDED_MARK), "남는다");
    }

    /// 마크업을 사이에 두고 떨어져 있던 낱말은 이웃이 아니다 — 버린 자리가 사슬을 끊는다.
    #[test]
    fn discarded_markup_breaks_the_neighbour_chain() {
        let mut corpus = CorpusCounts::new();
        corpus.read_sentence(&stripped_text("앞말 {{틀}} 뒷말"), false);
        assert_eq!(corpus.count("앞말"), Some(1));
        assert_eq!(corpus.count("뒷말"), Some(1));
        assert_eq!(corpus.pair_kinds(), 0);
    }

    /// 어절에 딱 붙은 마크업은 그 어절을 통째로 버린다 — 남기면 조사만 홀로 살아남아
    /// 낱말로 둔갑한다.
    #[test]
    fn markup_inside_an_eojeol_discards_it() {
        let mut corpus = CorpusCounts::new();
        corpus.read_sentence(&stripped_text("{{일본어|平和}}라고 불렀다"), false);
        assert_eq!(corpus.count("라고"), None);
        assert_eq!(corpus.count("불렀다"), Some(1));
    }

    /// 세로줄 없는 링크는 대상이 곧 화면에 보이는 글이다. 대괄호를 남기면 뒤에 붙은
    /// 조사까지 딸린 어절이 통째로 버려진다.
    #[test]
    fn bare_links_keep_their_text() {
        let mut corpus = CorpusCounts::new();
        corpus.read_sentence(&stripped_text("[[한국어]]는 아름답다"), false);
        assert_eq!(corpus.count("한국어는"), Some(1));
    }

    #[test]
    fn file_links_are_dropped_whole() {
        let stripped = stripped_text("[[파일:서울.jpg|섬네일|설명 글]]본문");
        assert_eq!(stripped.trim_start_matches(DISCARDED_MARK), "본문");
    }

    /// 절 제목과 분류 링크는 문서마다 똑같이 되풀이돼 흔한 낱말 자리를 빼앗는다.
    #[test]
    fn section_headings_and_categories_are_dropped() {
        let stripped = stripped_text("본문이다.\n== 외부 링크 ==\n주소\n[[분류:대한민국]]");
        assert!(stripped.contains("본문이다"));
        assert!(stripped.contains("주소"));
        for dropped in ["외부", "링크", "대한민국"] {
            assert!(!stripped.contains(dropped), "남아 있음: {dropped}");
        }
    }
}
