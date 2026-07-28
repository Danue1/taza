//! 원천 형식별 파서 — 파일 안의 글에서 낱말·문맥 신호를 뽑는다.
//!
//! 파서는 압축을 열지 않고(`source::container`가 이미 풀어 넘긴다), 원천을 어디서
//! 구했는지도 모른다(`source::acquire`의 몫이다). 형식 하나가 파일 하나이므로 원천이
//! 늘어나는 일이 파일을 더하는 일이 된다.

mod annotation_list;
mod cldr;
mod corpus;
mod emoji_test;
mod mecab;
mod nikl;
mod scowl;
mod tatoeba;
mod urimalsam;
mod wikitext;
mod word_list;

use crate::declaration::Extraction;
use std::path::Path;
use taza_engine::contract::{CandidateGroup, EmojiCategory};

/// 낱말에 곁들일 것 하나 — 이모지·기호·얼굴 문자. 낱말은 아직 표시 형태이며, 팩에 담길
/// 때 lexicon과 같은 조회 키로 인코딩된다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    pub word: String,
    pub group: CandidateGroup,
    pub text: String,
}

/// 한 원천에서 뽑은 신호.
///
/// 사전이 보증한 것(`attested`)과 코퍼스가 본 것(`observed`)은 다른 종류의 증거이므로
/// 따로 담는다. 한 필드에 겸하게 하면 원천이 둘 중 하나만 될 수 있는데, 전문용어 목록처럼
/// *표제어를 보증하면서 동시에 어절 빈도와 결합을 아는* 원천을 그러면 표현할 수 없다.
#[derive(Debug, Default)]
pub struct Signal {
    /// 사전이 표제어임을 보증한 낱말 — (낱말, 흔함 등급 0 초과 1 이하)
    pub attested: Vec<(String, f64)>,
    /// 코퍼스에서 관측된 낱말 — (낱말, 관측 횟수)
    pub observed: Vec<(String, u64)>,
    /// (앞말 번호, 뒷말 번호, 관측 횟수) — 문맥을 아는 원천만 낸다. 번호는 `observed`의
    /// 자리 번호다. 낱말을 글자로 실으면 짝 하나가 문자열 두 벌을 더 들고 다니는데,
    /// 덤프 규모에서는 그 두 벌이 신호의 대부분을 차지한다.
    pub bigrams: Vec<(u32, u32, u64)>,
    /// 활용형이 뻗어 나오는 어간 — 형태소 사전만 낸다. 코퍼스에서 관측된 활용형을
    /// 표제어로 받아들일지 가리는 조건이 된다.
    pub stems: Vec<String>,
    /// 낱말에 곁들일 것 — 주석 원천만 낸다.
    pub annotations: Vec<Annotation>,
    /// 검색면에 이모지를 세우는 차례 — (묶음, 이모지). 차례를 밝힌 원천만 낸다.
    pub emoji_order: Vec<(EmojiCategory, String)>,
    /// 어절 뒤에 붙어 활용형을 만드는 접사. 팩에 실려 코어가 학습한 어휘의 결합형을
    /// 제안하는 데 쓴다 — 사전을 넓히는 것과 같은 목록이어야 둘이 어긋나지 않는다.
    pub affixes: Vec<String>,
}

impl Signal {
    fn attested_only(attested: Vec<(String, f64)>) -> Self {
        Signal {
            attested,
            ..Signal::default()
        }
    }
}

/// 형식별 파서 판 번호. **파서의 동작을 바꾸면 그 형식의 번호를 반드시 올린다** —
/// 올리지 않으면 낡은 추출 결과 캐시가 조용히 쓰여, 고친 것이 팩에 반영되지 않는다.
///
/// 형식마다 따로 세는 이유는 파서 하나를 손봤을 때 나머지 원천까지 다시 훑을 까닭이
/// 없기 때문이다 — 위키백과 덤프 하나가 몇 분이고, 다른 원천은 그 사이 아무것도 달라진
/// 것이 없다. 다만 `corpus`처럼 여러 형식이 함께 쓰는 코드를 고쳤다면 그것을 쓰는 형식의
/// 번호를 모두 올려야 한다.
pub fn parser_version(extraction: &Extraction) -> u32 {
    match extraction {
        Extraction::Scowl { .. } => 3,
        Extraction::Tatoeba { .. } => 3,
        Extraction::Wikipedia { .. } => 4,
        Extraction::MecabKoDic { .. } => 3,
        Extraction::NiklCorpus { .. } => 3,
        Extraction::Urimalsam { .. } => 3,
        Extraction::CldrAnnotations => 3,
        Extraction::EmojiTest => 1,
        Extraction::AnnotationList { .. } => 3,
        Extraction::WordList { .. } => 3,
    }
}

pub fn parse(extraction: &Extraction, path: &Path, language: &str) -> Result<Signal, String> {
    match extraction {
        Extraction::Scowl {
            dialects,
            categories,
            maximum_size,
        } => scowl::parse(path, dialects, categories, *maximum_size).map(Signal::attested_only),
        Extraction::Tatoeba { minimum_count } => tatoeba::parse(path, language, *minimum_count),
        Extraction::Wikipedia { minimum_count } => wikitext::parse(path, language, *minimum_count),
        Extraction::MecabKoDic {
            files,
            verb_stem_files,
            inflection_files,
            particle_expansion_nouns,
        } => mecab::parse(
            path,
            files,
            verb_stem_files,
            inflection_files,
            *particle_expansion_nouns,
        ),
        Extraction::NiklCorpus { minimum_count } => nikl::parse(path, *minimum_count),
        Extraction::Urimalsam {
            sense_types,
            word_units,
            excluded_parts_of_speech,
            rank,
        } => urimalsam::parse(
            path,
            *rank,
            sense_types,
            word_units,
            excluded_parts_of_speech,
        ),
        Extraction::CldrAnnotations => cldr::parse(path),
        Extraction::EmojiTest => emoji_test::parse(path),
        Extraction::AnnotationList { group } => annotation_list::parse(path, (*group).into()),
        Extraction::WordList {
            rank,
            minimum_count,
        } => word_list::parse(path, *rank, *minimum_count),
    }
}
