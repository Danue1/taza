//! 추출 결과 캐시.
//!
//! 파이프라인 시간의 거의 전부가 원천을 훑는 데 든다 — 위키백과 덤프 하나가 8분이고,
//! 말뭉치가 늘수록 그만큼 쌓인다. 정규화 상수 하나를 고치려고 원천을 다시 훑는 일이
//! 없도록, 원천과 파서가 그대로면 지난번 결과를 그대로 쓴다.
//!
//! 캐시가 낡는 경우는 셋뿐이고 모두 키에 들어간다: 원천 파일이 바뀌거나(해시), 파서
//! 설정이 바뀌거나(추출 선언), 파서 자체가 바뀌었을 때(`parse::parser_version`).

use crate::parse::{Annotation, Signal, parser_version};
use crate::recipe::Extraction;
use crate::source::acquire::hex_digest;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use taza_engine::contract::{CandidateGroup, EmojiCategory};

/// 캐시 파일 형식의 판. 형식을 바꾸면 올린다 — 낡은 파일은 읽히지 않고 버려진다.
const FORMAT: u8 = 4;
const MAGIC: &[u8; 6] = b"TZSIG\0";

/// 캐시 압축 수준. 캐시는 오래 두는 것이 아니라 다음 실행까지만 사는 것이므로,
/// 압축률보다 쓰는 속도가 중요하다.
const COMPRESSION: i32 = 3;

pub fn path(
    directory: &Path,
    source_digest: &str,
    extraction: &Extraction,
    language: &str,
) -> PathBuf {
    // 추출 선언은 Debug 표현으로 지문을 뜬다 — 필드가 늘거나 값이 바뀌면 표현이 달라져
    // 캐시가 저절로 무효가 된다.
    let fingerprint = hex_digest(
        format!(
            "{source_digest}\n{extraction:?}\n{language}\n{}",
            parser_version(extraction)
        )
        .as_bytes(),
    );
    directory.join(format!("{}.tzsig", &fingerprint[..32]))
}

/// 캐시를 읽는다. 없거나 깨졌으면 `None` — 캐시 문제로 빌드가 멈추지는 않는다.
pub fn load(path: &Path) -> Option<Signal> {
    let file = std::fs::File::open(path).ok()?;
    let mut reader = zstd::Decoder::new(file).ok()?;
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).ok()?;
    decode(&bytes)
}

pub fn store(path: &Path, signal: &Signal) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("{} 만들기 실패: {error}", parent.display()))?;
    }
    let file = std::fs::File::create(path)
        .map_err(|error| format!("{} 만들기 실패: {error}", path.display()))?;
    let mut writer = zstd::Encoder::new(file, COMPRESSION)
        .map_err(|error| format!("{} 압축 실패: {error}", path.display()))?;
    writer
        .write_all(&encode(signal))
        .map_err(|error| format!("{} 쓰기 실패: {error}", path.display()))?;
    writer
        .finish()
        .map_err(|error| format!("{} 마무리 실패: {error}", path.display()))?;
    Ok(())
}

/// 신호는 항목이 수백만이라 JSON으로 담으면 쓰고 읽는 데만 몇십 초가 든다 — 캐시로
/// 아끼려던 시간을 되돌려 주는 셈이다. 길이를 앞세운 이진 형식으로 곧장 담는다.
fn encode(signal: &Signal) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(MAGIC);
    bytes.push(FORMAT);
    write_length(&mut bytes, signal.attested.len());
    for (word, rank) in &signal.attested {
        write_text(&mut bytes, word);
        bytes.extend_from_slice(&rank.to_le_bytes());
    }
    write_length(&mut bytes, signal.observed.len());
    for (word, count) in &signal.observed {
        write_text(&mut bytes, word);
        bytes.extend_from_slice(&count.to_le_bytes());
    }
    write_length(&mut bytes, signal.bigrams.len());
    for (left, right, count) in &signal.bigrams {
        bytes.extend_from_slice(&left.to_le_bytes());
        bytes.extend_from_slice(&right.to_le_bytes());
        bytes.extend_from_slice(&count.to_le_bytes());
    }
    write_length(&mut bytes, signal.annotations.len());
    for annotation in &signal.annotations {
        write_text(&mut bytes, &annotation.word);
        bytes.push(annotation.group.tag().unwrap_or_default());
        write_text(&mut bytes, &annotation.text);
    }
    for list in [&signal.stems, &signal.affixes] {
        write_length(&mut bytes, list.len());
        for text in list {
            write_text(&mut bytes, text);
        }
    }
    write_length(&mut bytes, signal.emoji_order.len());
    for (category, emoji) in &signal.emoji_order {
        bytes.push(category.tag());
        write_text(&mut bytes, emoji);
    }
    bytes
}

fn decode(bytes: &[u8]) -> Option<Signal> {
    let mut cursor = Cursor { bytes, at: 0 };
    if cursor.take(MAGIC.len())? != MAGIC || cursor.take(1)? != [FORMAT] {
        return None;
    }
    let mut signal = Signal::default();
    for _ in 0..cursor.length()? {
        let word = cursor.text()?;
        signal
            .attested
            .push((word, f64::from_le_bytes(cursor.eight()?)));
    }
    for _ in 0..cursor.length()? {
        let word = cursor.text()?;
        signal
            .observed
            .push((word, u64::from_le_bytes(cursor.eight()?)));
    }
    for _ in 0..cursor.length()? {
        let left = cursor.number()?;
        let right = cursor.number()?;
        signal
            .bigrams
            .push((left, right, u64::from_le_bytes(cursor.eight()?)));
    }
    for _ in 0..cursor.length()? {
        let word = cursor.text()?;
        let group = CandidateGroup::from_tag(cursor.take(1)?[0])?;
        signal.annotations.push(Annotation {
            word,
            group,
            text: cursor.text()?,
        });
    }
    for _ in 0..cursor.length()? {
        signal.stems.push(cursor.text()?);
    }
    for _ in 0..cursor.length()? {
        signal.affixes.push(cursor.text()?);
    }
    for _ in 0..cursor.length()? {
        let category = EmojiCategory::from_tag(cursor.take(1)?[0])?;
        signal.emoji_order.push((category, cursor.text()?));
    }
    Some(signal)
}

fn write_length(bytes: &mut Vec<u8>, length: usize) {
    bytes.extend_from_slice(&(length as u64).to_le_bytes());
}

fn write_text(bytes: &mut Vec<u8>, text: &str) {
    bytes.extend_from_slice(&(text.len() as u32).to_le_bytes());
    bytes.extend_from_slice(text.as_bytes());
}

struct Cursor<'bytes> {
    bytes: &'bytes [u8],
    at: usize,
}

impl<'bytes> Cursor<'bytes> {
    fn take(&mut self, count: usize) -> Option<&'bytes [u8]> {
        let slice = self.bytes.get(self.at..self.at + count)?;
        self.at += count;
        Some(slice)
    }

    fn eight(&mut self) -> Option<[u8; 8]> {
        self.take(8)?.try_into().ok()
    }

    fn length(&mut self) -> Option<usize> {
        Some(u64::from_le_bytes(self.eight()?) as usize)
    }

    fn number(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    fn text(&mut self) -> Option<String> {
        let length = u32::from_le_bytes(self.take(4)?.try_into().ok()?) as usize;
        String::from_utf8(self.take(length)?.to_vec()).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_survives_a_round_trip() {
        let signal = Signal {
            attested: vec![("규제".to_string(), 0.4)],
            observed: vec![("당국".to_string(), 7)],
            bigrams: vec![(0, 1, 3)],
            annotations: vec![Annotation {
                word: "웃음".to_string(),
                group: CandidateGroup::Emoji,
                text: "😀".to_string(),
            }],
            stems: vec!["하".to_string()],
            affixes: vec!["는".to_string()],
            emoji_order: vec![(EmojiCategory::SmileysAndPeople, "😀".to_string())],
        };
        let decoded = decode(&encode(&signal)).unwrap();
        assert_eq!(decoded.attested, signal.attested);
        assert_eq!(decoded.observed, signal.observed);
        assert_eq!(decoded.bigrams, signal.bigrams);
        assert_eq!(decoded.annotations, signal.annotations);
        assert_eq!(decoded.stems, signal.stems);
        assert_eq!(decoded.affixes, signal.affixes);
        assert_eq!(decoded.emoji_order, signal.emoji_order);
    }

    /// 캐시가 낡는 세 경우가 모두 키에 들어가는가 — 원천이 바뀌거나, 파서 설정이
    /// 바뀌거나, 다른 언어로 읽거나. (파서 자체의 변경은 `parse::parser_version`이 맡는다.)
    #[test]
    fn key_changes_with_source_and_settings() {
        let directory = Path::new("/cache");
        let five = Extraction::Wikipedia { minimum_count: 5 };
        let six = Extraction::Wikipedia { minimum_count: 6 };
        let base = path(directory, "abc123", &five, "ko");
        assert_eq!(base, path(directory, "abc123", &five, "ko"));
        for different in [
            path(directory, "def456", &five, "ko"),
            path(directory, "abc123", &six, "ko"),
            path(directory, "abc123", &five, "en"),
        ] {
            assert_ne!(base, different);
        }
    }

    /// 깨진 캐시는 빌드를 멈추지 않고 그냥 없는 것으로 친다.
    #[test]
    fn damaged_cache_reads_as_missing() {
        assert!(decode(b"not a signal").is_none());
        let mut bytes = encode(&Signal::default());
        bytes.truncate(bytes.len() - 1);
        assert!(decode(&bytes).is_none());
    }
}
