//! mozc 사전 — 읽기·표기·비용·연접.
//!
//! **입력기용으로 만들어진 사전이라는 점이 다른 후보와 갈리는 자리다.** 형태소 분석 사전
//! (ipadic·UniDic 계열)의 비용은 *표기가 이미 주어진* 글에서 경계를 가르도록 학습된
//! 값이다. 분석기는 「きしゃ」를 만날 일이 없다 — 汽車인지 記者인지가 이미 글자에 적혀
//! 있으니까. 입력기의 일은 정확히 그 반대라, 같은 형식이어도 그 값을 그대로 쓰면 동음이의
//! 선택에 쓸 정보가 사실상 없다. mozc의 비용은 변환 방향으로 매겨져 있고, 읽기 열이
//! 이미 히라가나여서 조회 키 공간과 그대로 맞는다.
//!
//! 배포본 하나에서 넷을 함께 읽는다. 넷이 **같은 문맥 id 공간**을 쓰는 것이 이 조합의
//! 조건이다 — 연접 행렬은 id 공간 하나에 묶여 있어서, 다른 사전을 섞으려면 id를 옮겨야
//! 하고 그 순간 학습된 값의 짝이 깨진다.
//!
//! | 파일 | 무엇 | 형식 |
//! |---|---|---|
//! | `dictionary0N.txt` | 주 어휘 | `읽기 · 좌id · 우id · 비용 · 표기` (탭) |
//! | `suffix.txt` | 접미·어미 | 위와 같음 |
//! | `single_kanji.tsv` | 단漢字 음훈 | `읽기 · 한자들` (붙여 쓴 차례가 곧 우선순위) |
//! | `connection_single_column.txt` | 연접 값 | 첫 줄이 크기, 그 뒤로 크기² 개의 값 |
//! | `id.def` | 문맥 id → 품사 | `id 품사,세분류…` |
//!
//! 기호·얼굴 문자(`symbol.tsv`·`emoticon.tsv`)는 변환표가 아니라 **곁들이는 것**으로 간다 —
//! 이 저장소에는 낱말에 곁들이는 통로가 이미 있고(annotation 섹션), 「、」를 변환 후보로
//! 내놓는 것과 후보 바에 곁들이는 것은 다른 일이다.

use super::{Annotation, Conversion, Signal};
use crate::source::container;
use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::path::Path;
use taza_engine::contract::CandidateGroup;

/// 단漢字에 매기는 값의 바닥. 어떤 낱말보다 비싸야 「あい」가 「愛」라는 낱말로 먼저
/// 서고 단漢字는 그 뒤에 줄을 선다.
///
/// 값을 사전 비용대(수천)와 같은 자리에 두었더니 한 글자짜리가 조사를 이겼다 — 「やつが」가
/// 「やつ画」가 되는 식이다. 조사는 격자에서 한 글자이므로 같은 자리를 다투는데, 단漢字는
/// 사람이 **일부러 고를 때만** 쓰는 것이라 첫 변환에서 이길 이유가 없다.
const SINGLE_KANJI_BASE_COST: u16 = 15000;

/// 같은 읽기의 단漢字끼리 벌리는 값 — 사전이 적어 둔 차례가 곧 우선순위다.
const SINGLE_KANJI_STEP: u16 = 10;

/// 단漢字가 빌리는 문맥 id의 품사. 자기 id가 없으므로 가장 흔한 명사 자리를 빌린다 —
/// 0(BOS/EOS)을 주면 문장 끝처럼 이어져 라티스가 앞뒤와 잇지 못한다.
const SINGLE_KANJI_PART_OF_SPEECH: &str = "名詞,一般";

/// 읽고 나면 그만인 파일들 — 배포본에는 이 밖에도 수천 개가 들어 있다.
fn wanted(file_name: &str, dictionary_files: &[String]) -> bool {
    dictionary_files.iter().any(|file| file == file_name)
        || matches!(
            file_name,
            "id.def"
                | "suffix.txt"
                | "single_kanji.tsv"
                | "connection_single_column.txt"
                | "symbol.tsv"
                | "emoticon.tsv"
        )
}

/// 문맥 id가 앞말에 붙는 말인가를 가리는 표. `id.def`의 품사 이름으로 판정한다.
struct PartsOfSpeech {
    dependent: HashSet<u16>,
    /// 단漢字가 빌릴 명사 id
    noun: u16,
}

fn read_parts_of_speech(reader: impl BufRead, dependent_tags: &[String]) -> PartsOfSpeech {
    let mut dependent = HashSet::new();
    let mut noun = 0;
    for line in reader.lines().map_while(Result::ok) {
        let Some((id, description)) = line.split_once(' ') else {
            continue;
        };
        let Ok(id) = id.parse::<u16>() else { continue };
        if description.starts_with(SINGLE_KANJI_PART_OF_SPEECH) && noun == 0 {
            noun = id;
        }
        let fields: Vec<&str> = description.split(',').collect();
        if fields
            .iter()
            .any(|field| dependent_tags.iter().any(|tag| tag == field))
        {
            dependent.insert(id);
        }
    }
    PartsOfSpeech { dependent, noun }
}

/// `읽기 · 좌id · 우id · 비용 · 표기` 다섯 열.
fn read_entries(reader: impl BufRead, into: &mut Vec<Conversion>) {
    for line in reader.lines().map_while(Result::ok) {
        let fields: Vec<&str> = line.split('\t').collect();
        let [reading, left_id, right_id, cost, surface] = fields.as_slice() else {
            continue;
        };
        let (Ok(left_id), Ok(right_id), Ok(cost)) = (
            left_id.parse::<u16>(),
            right_id.parse::<u16>(),
            cost.parse::<i64>(),
        ) else {
            continue;
        };
        if reading.is_empty() || surface.is_empty() {
            continue;
        }
        into.push(Conversion {
            reading: reading.to_string(),
            surface: surface.to_string(),
            left_id,
            right_id,
            // 흔한 말에 음수를 주는 자리가 있다. 아래로 자르되 자리를 옮기지는 않는다 —
            // 연접 값(i16)과 미등록 마디 값이 같은 눈금 위에 있어야 합이 뜻을 갖는다.
            cost: cost.clamp(0, u16::MAX as i64) as u16,
            // 품사는 뒤에서 id로 채운다 — id.def를 먼저 읽었다는 보장이 없다
            dependent: false,
        });
    }
}

/// `읽기 · 한자들` — 한자를 붙여 쓴 차례가 곧 우선순위다.
fn read_single_kanji(lines: &[(String, String)], noun: u16, into: &mut Vec<Conversion>) {
    for (reading, kanji) in lines {
        for (index, character) in kanji.chars().enumerate() {
            let step = SINGLE_KANJI_STEP.saturating_mul(index.min(u16::MAX as usize) as u16);
            into.push(Conversion {
                reading: reading.to_string(),
                surface: character.to_string(),
                left_id: noun,
                right_id: noun,
                cost: SINGLE_KANJI_BASE_COST.saturating_add(step),
                dependent: false,
            });
        }
    }
}

/// 첫 줄이 축의 크기이고 그 뒤로 크기² 개의 값이 한 줄에 하나씩 온다. 바깥 고리가
/// 앞말의 우문맥 id이고 안쪽이 뒷말의 좌문맥 id다 — 사전 자신의 조회 규약(rid, lid)과
/// 같은 차례다.
///
/// **재어서 정한 방향이다.** 두 축의 크기가 같아 뒤바뀌어도 빌드는 그대로 돌고 변환
/// 품질만 조용히 나빠지므로, 사전을 갈 때마다 다시 재야 한다. mozc 평가 셋 564문장에서
/// 이 방향이 문장 0.637 · 글자 0.852이고 뒤집으면 0.532 · 0.792다
/// (`cargo run -p taza-evaluation --example conversion_report`).
fn read_connection(reader: impl BufRead) -> Option<super::Connection> {
    let mut lines = reader.lines().map_while(Result::ok);
    let size: u16 = lines.next()?.trim().parse().ok()?;
    let mut costs = Vec::new();
    for (index, line) in lines.enumerate() {
        let Ok(cost) = line.trim().parse::<i64>() else {
            continue;
        };
        // 값이 0인 칸은 표를 채우는 기본값이라 실어 봐야 자리만 먹는다
        if cost == 0 {
            continue;
        }
        let row = (index / size as usize) as u16;
        let column = (index % size as usize) as u16;
        costs.push((
            row,
            column,
            cost.clamp(i16::MIN as i64, i16::MAX as i64) as i16,
        ));
    }
    Some(super::Connection {
        rows: size,
        columns: size,
        costs,
    })
}

/// `문자 · 읽기들(공백 구분) · 분류` — 첫 줄은 열 이름이다.
fn read_annotations(reader: impl BufRead, group: CandidateGroup, into: &mut Vec<Annotation>) {
    for line in reader.lines().map_while(Result::ok).skip(1) {
        let fields: Vec<&str> = line.split('\t').collect();
        let [text, readings, ..] = fields.as_slice() else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        for reading in readings.split_whitespace() {
            into.push(Annotation {
                word: reading.to_string(),
                group,
                text: text.to_string(),
            });
        }
    }
}

/// `품사 · 문자 · 읽기들 · …` — 기호 표는 열 배치가 얼굴 문자와 다르다.
fn read_symbols(reader: impl BufRead, into: &mut Vec<Annotation>) {
    for line in reader.lines().map_while(Result::ok).skip(1) {
        let fields: Vec<&str> = line.split('\t').collect();
        let [_, text, readings, ..] = fields.as_slice() else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        for reading in readings.split_whitespace() {
            // 표에는 문자 자신도 읽기로 실려 있다 — 조회 키는 가나여야 한다
            if reading == *text {
                continue;
            }
            into.push(Annotation {
                word: reading.to_string(),
                group: CandidateGroup::Symbol,
                text: text.to_string(),
            });
        }
    }
}

pub fn parse(
    path: &Path,
    dictionary_files: &[String],
    dependent_tags: &[String],
) -> Result<Signal, String> {
    let mut entries: Vec<Conversion> = Vec::new();
    let mut single_kanji_lines: Vec<(String, String)> = Vec::new();
    let mut parts_of_speech: Option<PartsOfSpeech> = None;
    let mut signal = Signal::default();

    let mut archive = container::open_tar_gz(path)?;
    let members = archive
        .entries()
        .map_err(|error| format!("{} 목록 읽기 실패: {error}", path.display()))?;
    for member in members {
        let member = member.map_err(|error| format!("항목 읽기 실패: {error}"))?;
        let member_path = member
            .path()
            .map_err(|error| format!("항목 경로 읽기 실패: {error}"))?
            .to_path_buf();
        let Some(file_name) = member_path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        // 시험용 사본이 같은 이름으로 들어 있다 — 그쪽을 읽으면 어휘가 백분의 일이 된다
        if member_path
            .components()
            .any(|part| part.as_os_str() == "test")
        {
            continue;
        }
        if !wanted(file_name, dictionary_files) {
            continue;
        }
        let reader = BufReader::new(member);
        match file_name {
            "id.def" => parts_of_speech = Some(read_parts_of_speech(reader, dependent_tags)),
            "connection_single_column.txt" => signal.connection = read_connection(reader),
            "symbol.tsv" => read_symbols(reader, &mut signal.annotations),
            "emoticon.tsv" => {
                read_annotations(reader, CandidateGroup::Emoticon, &mut signal.annotations)
            }
            // 단漢字는 명사 id를 빌리므로 id.def를 다 읽은 뒤에 세운다
            "single_kanji.tsv" => {
                for line in reader.lines().map_while(Result::ok) {
                    if let Some((reading, kanji)) = line.split_once('\t') {
                        single_kanji_lines.push((reading.to_string(), kanji.to_string()));
                    }
                }
            }
            _ => read_entries(reader, &mut entries),
        }
    }

    let parts_of_speech =
        parts_of_speech.ok_or_else(|| format!("{}: id.def를 찾지 못함", path.display()))?;
    for entry in &mut entries {
        entry.dependent = parts_of_speech.dependent.contains(&entry.left_id);
    }
    read_single_kanji(&single_kanji_lines, parts_of_speech.noun, &mut entries);

    if entries.is_empty() {
        return Err(format!("{}: 표제어를 찾지 못함", path.display()));
    }
    signal.conversions = entries;
    Ok(signal)
}
