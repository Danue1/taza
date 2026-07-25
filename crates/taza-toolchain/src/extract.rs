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
    /// 활용형이 뻗어 나오는 어간 — 형태소 사전만 낸다. 코퍼스에서 관측된 활용형을
    /// 표제어로 받아들일지 가리는 조건이 된다.
    pub stems: Vec<String>,
    /// 어절 뒤에 붙어 활용형을 만드는 접사. 팩에 실려 코어가 학습한 어휘의 결합형을
    /// 제안하는 데 쓴다 — 사전을 넓히는 것과 같은 목록이어야 둘이 어긋나지 않는다.
    pub affixes: Vec<String>,
}

impl Extracted {
    fn words_only(words: Vec<(String, f64)>) -> Self {
        Extracted {
            words,
            ..Extracted::default()
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
        Extraction::Wikipedia { minimum_count } => wikipedia(path, language, *minimum_count),
        Extraction::MecabKoDic {
            files,
            verb_stem_files,
            particle_expansion_nouns,
        } => mecab_ko_dic(path, files, verb_stem_files, *particle_expansion_nouns),
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

/// 집계 표 하나가 이 항목 수를 넘으면 한 번짜리를 쳐낸다.
const PRUNE_THRESHOLD: usize = 8_000_000;

/// 문장 코퍼스의 집계 — 낱말 빈도와 이웃한 낱말 짝. 원천이 문장을 어떻게 담고
/// 있든(TSV 한 줄, XML 안의 wikitext) 세는 방식은 같으므로 여기 모은다.
#[derive(Default)]
struct CorpusCounts {
    counts: HashMap<String, u64>,
    pairs: HashMap<(String, String), u64>,
}

impl CorpusCounts {
    /// 대소문자가 있는 스크립트에서는 소문자로 나타난 출현만 센다 — 예문 인물 이름("Tom")이
    /// 극단적으로 흔해서 대문자 출현을 함께 세면 이름이 흔한 낱말을 밀어내고 상위권을
    /// 차지한다. 문장 첫머리 출현을 잃는 대신(흔한 낱말은 문장 중간에도 충분히 나타난다)
    /// 고유명사 편향이 사라진다. 걸러진 낱말은 짝의 사슬도 끊는다 — 건너뛰어 이으면
    /// 실제로 이웃하지 않은 낱말이 문맥으로 둔갑한다.
    fn read_sentence(&mut self, sentence: &str, cased: bool) {
        let mut previous: Option<&str> = None;
        for token in
            sentence.split(|character: char| !(character.is_alphabetic() || character == '\''))
        {
            if token.is_empty() || (cased && token.chars().next().is_some_and(char::is_uppercase)) {
                previous = None;
                continue;
            }
            *self.counts.entry(token.to_string()).or_insert(0) += 1;
            if let Some(left) = previous {
                *self
                    .pairs
                    .entry((left.to_string(), token.to_string()))
                    .or_insert(0) += 1;
            }
            previous = Some(token);
        }
    }

    /// 집계 표가 너무 커지면 한 번만 만난 항목을 쳐낸다. 위키백과 규모의 코퍼스는
    /// 짝의 종류가 수천만이라 통째로 들고 있을 수 없는데, 우리가 쓰는 것은 흔한
    /// 쪽이므로 한 번짜리를 버려도 순위가 거의 달라지지 않는다 — 실제로 흔한 것은
    /// 곧 다시 만나 되살아난다. 최종 횟수는 그만큼 과소평가되지만 순서는 남는다.
    fn prune_if_large(&mut self) {
        if self.counts.len() > PRUNE_THRESHOLD {
            self.counts.retain(|_, count| *count > 1);
        }
        if self.pairs.len() > PRUNE_THRESHOLD {
            self.pairs.retain(|_, count| *count > 1);
        }
    }

    fn finish(self, minimum_count: u64) -> Extracted {
        Extracted {
            words: self
                .counts
                .into_iter()
                .filter(|(_, count)| *count >= minimum_count)
                .map(|(word, count)| (word, count as f64))
                .collect(),
            bigrams: self
                .pairs
                .into_iter()
                .map(|((left, right), count)| (left, right, count))
                .collect(),
            stems: Vec::new(),
            affixes: Vec::new(),
        }
    }
}

/// Tatoeba 문장 익스포트(bzip2 TSV `식별자<TAB>언어<TAB>문장`).
fn tatoeba(path: &Path, language: &str, minimum_count: u64) -> Result<Extracted, String> {
    let file = std::fs::File::open(path)
        .map_err(|error| format!("{} 열기 실패: {error}", path.display()))?;
    let reader = BufReader::new(BzDecoder::new(file));
    let cased = language == "en";

    let mut corpus = CorpusCounts::default();
    for line in reader.lines() {
        let line = line.map_err(|error| format!("{} 읽기 실패: {error}", path.display()))?;
        let Some(sentence) = line.split('\t').nth(2) else {
            continue;
        };
        corpus.read_sentence(sentence, cased);
    }
    if corpus.counts.is_empty() {
        return Err(format!("{}: 문장을 읽지 못했음", path.display()));
    }
    Ok(corpus.finish(minimum_count))
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

/// MediaWiki XML 덤프(bzip2)의 본문 문서에서 낱말 빈도와 이웃 짝을 센다.
///
/// 문장 코퍼스보다 훨씬 크지만 문어체 쪽으로 기울어 있다 — 구어체 원천과 함께 쓰고
/// weight로 균형을 잡는 것을 전제한 원천이다.
fn wikipedia(path: &Path, language: &str, minimum_count: u64) -> Result<Extracted, String> {
    let file = std::fs::File::open(path)
        .map_err(|error| format!("{} 열기 실패: {error}", path.display()))?;
    let reader = BufReader::new(BzDecoder::new(file));
    let cased = language == "en";

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
/// 남긴다. 버린 자리에는 줄바꿈을 넣어 낱말 사슬을 끊는다 — 마크업을 사이에 두고
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
            .or_else(|| skip_until(rest, "<ref", "</ref>").or_else(|| skip_until(rest, "<ref", "/>")))
            .or_else(|| skip_until(rest, "<!--", "-->"))
        {
            // 구분자 둘 — 하나로는 빈 토큰이 생기지 않아 사슬이 끊기지 않는다
            stripped.push_str("\n\n");
            index += skipped;
            continue;
        }
        // `[[대상|표시]]` — 대상은 문서 이름이라 본문 흐름의 낱말이 아니다
        if rest.starts_with("[[")
            && let Some(close) = rest.find("]]")
            && let Some(bar) = rest[..close].rfind('|')
        {
            stripped.push_str(&rest[bar + 1..close]);
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

/// 체언에 붙어 용언을 만드는 접미사. 사전은 이것을 명사와 따로 싣기 때문에
/// "생각하", "걱정되" 같은 어간이 어디에도 없다 — 여기서 되살린다.
const DERIVATIONAL_SUFFIXES: [&str; 2] = ["하", "되"];

/// 이형태를 펼친 조사 목록 — 코어는 결합형을 알아보기만 하면 되므로 앞말의 종성으로
/// 짝을 가릴 필요가 없다.
fn particle_forms() -> Vec<String> {
    let mut forms: Vec<String> = PARTICLES
        .iter()
        .flat_map(|&(after_consonant, after_vowel)| [after_consonant, after_vowel])
        .map(str::to_string)
        .collect();
    forms.sort_unstable();
    forms.dedup();
    forms
}

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
) -> Result<Extracted, String> {
    let mut costs: HashMap<String, i64> = HashMap::new();
    let mut stems: Vec<String> = Vec::new();
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
                stems.push(surface.to_string());
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
    Ok(Extracted {
        words: costs
            .into_iter()
            .map(|(word, cost)| {
                let rank = 1.0 - (cost - minimum) as f64 / span;
                // 최소 등급이 0이 되면 인벤토리에서 사라지므로 바닥을 남긴다
                (word, rank.max(0.01))
            })
            .collect(),
        bigrams: Vec::new(),
        stems,
        affixes: particle_forms(),
    })
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
        assert_eq!(stripped.trim(), "남는다");
    }

    /// 마크업을 사이에 두고 떨어져 있던 낱말은 이웃이 아니다 — 버린 자리가 사슬을 끊는다.
    #[test]
    fn discarded_markup_breaks_the_neighbour_chain() {
        let stripped = strip_wikitext("앞말{{틀}}뒷말");
        let mut corpus = CorpusCounts::default();
        corpus.read_sentence(&stripped, false);
        assert_eq!(corpus.counts.get("앞말"), Some(&1));
        assert_eq!(corpus.counts.get("뒷말"), Some(&1));
        assert!(corpus.pairs.is_empty());
    }

    #[test]
    fn file_links_are_dropped_whole() {
        let stripped = strip_wikitext("[[파일:서울.jpg|섬네일|설명 글]]본문");
        assert_eq!(stripped.trim(), "본문");
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
