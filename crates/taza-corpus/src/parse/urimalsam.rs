//! 우리말샘 사전 XML.
//!
//! 110만 표제어를 싣지만 그중 상당수가 방언·북한어·옛말이라 그대로 받으면 표준 어휘를
//! 예산에서 밀어낸다. 다행히 사전이 그것을 스스로 구분해 두었으므로(`senseInfo/type`)
//! 무엇을 받을지 레시피가 고를 수 있다.
//!
//! 사전에는 빈도가 없다. 여기서 나오는 것은 "이것이 낱말이다"라는 보증뿐이고, 예산 안에
//! 들어갈 순위는 코퍼스가 정한다.

use super::Signal;
use super::corpus::CorpusCounts;
use crate::source::container;
use std::collections::HashMap;
use std::io::BufRead;
use std::path::Path;

/// 표제어에서 떼어 낼 사전 표기. 붙임표는 형태소 경계를, 어깨점은 발음 구분을 나타내는
/// 표기일 뿐 실제로 치는 글자가 아니다.
const DICTIONARY_MARKS: [char; 2] = ['-', 'ㆍ'];

/// 전문용어 표기의 구 경계. 사전에서 이 자리는 **붙여 써도 되는** 띄어쓰기를 뜻하므로
/// (`규제^당국`) 띄어 쓴 어절과 붙여 쓴 형태를 모두 낸다.
const PHRASE_BOUNDARY: char = '^';

/// 한 항목에서 읽어 둘 것. 표제어는 `wordInfo`에, 갈래는 `senseInfo`에 있어 항목이
/// 끝나야 받을지 판정할 수 있다.
#[derive(Default)]
struct Entry {
    word: Option<String>,
    word_unit: Option<String>,
    sense_type: Option<String>,
    part_of_speech: Option<String>,
}

pub fn parse(
    path: &Path,
    rank: f64,
    sense_types: &[String],
    word_units: &[String],
    excluded_parts_of_speech: &[String],
) -> Result<Signal, String> {
    let mut attested: HashMap<String, f64> = HashMap::new();
    let mut corpus = CorpusCounts::new();
    container::for_each_member(path, |name, reader| {
        if !name.ends_with(".xml") {
            return Ok(());
        }
        // 같은 이름의 원소가 `senseInfo` 아래에도 `relation_info` 아래에도 나온다
        // (`word`, `type`). 어느 쪽인지는 지금 열려 있는 원소를 따라가야 알 수 있다.
        let mut open: Vec<String> = Vec::new();
        let mut entry = Entry::default();
        for line in reader.lines() {
            let line = line.map_err(|error| format!("{name} 읽기 실패: {error}"))?;
            let Some(element) = element(line.trim()) else {
                continue;
            };
            match element {
                Open("item") => entry = Entry::default(),
                Close("item") => {
                    if let Some(word) =
                        accept(&entry, sense_types, word_units, excluded_parts_of_speech)
                    {
                        record(&word, rank, &mut attested, &mut corpus);
                    }
                }
                Open(name) => open.push(name.to_string()),
                Close(_) => {
                    open.pop();
                }
                Leaf(name, text) => {
                    let parent = open.last().map(String::as_str).unwrap_or("");
                    match (parent, name) {
                        ("wordInfo", "word") => entry.word = Some(text),
                        ("wordInfo", "word_unit") => entry.word_unit = Some(text),
                        ("senseInfo", "type") => entry.sense_type = Some(text),
                        ("senseInfo", "pos") => entry.part_of_speech = Some(text),
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    })?;
    if attested.is_empty() {
        return Err(format!("{}: 조건에 맞는 표제어가 없음", path.display()));
    }
    Ok(Signal {
        attested: attested.into_iter().collect(),
        ..corpus.finish(1, 1)
    })
}

/// 항목이 표제어가 될 자격을 갖췄으면 정제한 표기를 돌려준다.
fn accept(
    entry: &Entry,
    sense_types: &[String],
    word_units: &[String],
    excluded_parts_of_speech: &[String],
) -> Option<String> {
    let matches = |value: &Option<String>, allowed: &[String]| {
        value
            .as_ref()
            .is_some_and(|value| allowed.iter().any(|wanted| wanted == value))
    };
    if !matches(&entry.sense_type, sense_types) || !matches(&entry.word_unit, word_units) {
        return None;
    }
    if entry
        .part_of_speech
        .as_ref()
        .is_some_and(|pos| excluded_parts_of_speech.iter().any(|wanted| wanted == pos))
    {
        return None;
    }
    let word = entry.word.as_ref()?.replace(DICTIONARY_MARKS, "");
    (!word.trim().is_empty()).then_some(word)
}

/// 표제어 하나를 신호에 담는다. 구는 어절로 쪼개 각각 표제어로 내고, 이웃 짝을 문맥으로
/// 낸다 — 용어가 아는 것의 절반은 "이 말 다음에 저 말이 온다"이다.
fn record(word: &str, rank: f64, attested: &mut HashMap<String, f64>, corpus: &mut CorpusCounts) {
    let spaced = word.replace(PHRASE_BOUNDARY, " ");
    let mut headword = |text: &str| {
        let slot = attested.entry(text.to_string()).or_insert(0.0);
        *slot = slot.max(rank);
    };
    for eojeol in spaced.split_whitespace() {
        headword(eojeol);
    }
    // 구 경계는 붙여 써도 되는 자리다 — 오히려 그쪽이 더 흔히 쳐진다("빅테크").
    // 원래 공백으로 나뉘어 있던 구는 붙여 쓸 근거가 없으므로 만들지 않는다.
    if word.contains(PHRASE_BOUNDARY) {
        let joined = word.replace(PHRASE_BOUNDARY, "");
        if !joined.contains(char::is_whitespace) {
            headword(&joined);
        }
    }
    corpus.read_sentence(&spaced, false);
}

/// 읽어 낸 XML 한 줄. 우리말샘 XML은 원소마다 줄이 나뉘어 있어 이만큼으로 충분하다 —
/// 1.9GB를 통째로 트리에 올리지 않으려면 흘려 읽어야 한다.
enum Element {
    Open(&'static str),
    Close(&'static str),
    Leaf(&'static str, String),
}
use Element::{Close, Leaf, Open};

/// 관심 있는 원소 이름만 알아본다 — 그 밖의 원소는 열고 닫는 것만 세면 된다.
const TRACKED: [&str; 6] = ["item", "wordInfo", "senseInfo", "word", "word_unit", "pos"];

fn element(line: &str) -> Option<Element> {
    let inner = line.strip_prefix('<')?;
    if let Some(name) = inner.strip_prefix('/') {
        let name = name.strip_suffix('>')?;
        return Some(Close(interned(name)));
    }
    let (name, rest) = inner.split_once('>')?;
    if rest.is_empty() {
        return Some(Open(interned(name)));
    }
    let text = rest.strip_suffix(&format!("</{name}>"))?;
    let text = text
        .strip_prefix("<![CDATA[")
        .and_then(|text| text.strip_suffix("]]>"))
        .unwrap_or(text);
    Some(Leaf(interned(name), text.to_string()))
}

/// 원소 이름을 정적 문자열로 바꾼다. 관심 밖 이름은 빈 문자열이 되어 어느 갈래에도
/// 걸리지 않으면서 여는·닫는 짝은 그대로 세어진다.
fn interned(name: &str) -> &'static str {
    // `type`은 `senseInfo` 아래와 `relation_info` 아래 양쪽에 나오므로 부모로 가린다
    TRACKED
        .into_iter()
        .chain(["type"])
        .find(|tracked| *tracked == name)
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<channel>
<item>
<wordInfo>
<word><![CDATA[흙-무덤]]></word>
<word_unit>어휘</word_unit>
</wordInfo>
<senseInfo>
<pos>명사</pos>
<type>일반어</type>
<relation_info>
<word><![CDATA[토총001]]></word>
<type>비슷한말</type>
</relation_info>
</senseInfo>
</item>
<item>
<wordInfo>
<word><![CDATA[흙무데기]]></word>
<word_unit>어휘</word_unit>
</wordInfo>
<senseInfo>
<pos>명사</pos>
<type>북한어</type>
</senseInfo>
</item>
<item>
<wordInfo>
<word><![CDATA[규제^당국]]></word>
<word_unit>구</word_unit>
</wordInfo>
<senseInfo>
<pos>명사</pos>
<type>일반어</type>
</senseInfo>
</item>
</channel>"#;

    fn parse_sample(name: &str, sense_types: &[&str], word_units: &[&str]) -> Signal {
        let directory = std::env::temp_dir().join("taza-urimalsam-test");
        std::fs::create_dir_all(&directory).unwrap();
        // 테스트는 나란히 도므로 파일을 나눠 갖는다
        let path = directory.join(format!("{name}.xml"));
        std::fs::write(&path, SAMPLE).unwrap();
        let owned = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let signal = parse(&path, 0.05, &owned(sense_types), &owned(word_units), &[]).unwrap();
        std::fs::remove_file(&path).unwrap();
        signal
    }

    /// 방언·북한어·옛말은 표준 어휘를 예산에서 밀어낸다 — 사전이 구분해 둔 것을 쓴다.
    #[test]
    fn only_the_requested_sense_types_become_headwords() {
        let signal = parse_sample("sense", &["일반어"], &["어휘", "구"]);
        let attested: HashMap<String, f64> = signal.attested.into_iter().collect();
        assert_eq!(attested.get("흙무덤"), Some(&0.05));
        assert_eq!(attested.get("흙무데기"), None);
    }

    /// `type`과 `word`는 `senseInfo` 아래에도 `relation_info` 아래에도 나온다.
    /// 부모를 보지 않으면 "비슷한말"을 갈래로, 관련어를 표제어로 잘못 읽는다.
    #[test]
    fn related_words_are_not_mistaken_for_headwords() {
        let signal = parse_sample("relation", &["일반어"], &["어휘", "구"]);
        let attested: HashMap<String, f64> = signal.attested.into_iter().collect();
        assert_eq!(attested.get("토총001"), None);
    }

    /// 구는 어절로 쪼개고, 붙여 쓴 형태와 이웃 짝도 함께 낸다.
    #[test]
    fn phrases_yield_eojeol_joined_form_and_context() {
        let signal = parse_sample("phrase", &["일반어"], &["어휘", "구"]);
        let attested: HashMap<&str, f64> = signal
            .attested
            .iter()
            .map(|(word, rank)| (word.as_str(), *rank))
            .collect();
        for expected in ["규제", "당국", "규제당국"] {
            assert_eq!(attested.get(expected), Some(&0.05), "빠짐: {expected}");
        }
        let (left, right) = (place(&signal, "규제"), place(&signal, "당국"));
        assert!(signal.bigrams.contains(&(left, right, 1)));
    }

    /// 짝은 낱말 번호로 실린다 — 번호는 `observed`의 자리 번호다.
    fn place(signal: &Signal, word: &str) -> u32 {
        signal
            .observed
            .iter()
            .position(|(observed, _)| observed == word)
            .expect("관측 목록에 없음") as u32
    }

    #[test]
    fn word_units_outside_the_list_are_skipped() {
        let signal = parse_sample("unit", &["일반어"], &["어휘"]);
        let attested: HashMap<String, f64> = signal.attested.into_iter().collect();
        assert_eq!(attested.get("규제당국"), None);
        assert_eq!(attested.get("흙무덤"), Some(&0.05));
    }
}
