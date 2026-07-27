//! 원천 신호들을 팩에 담을 하나의 점수표로 합친다.
//!
//! 점수는 원천 코퍼스의 절대 빈도가 아니라 [1, `MAX_FREQUENCY`]로 정규화된 값이다.
//! 절대 빈도를 그대로 실으면 (1) 흔한 낱말의 점수가 개인화 가중치를 압도해 학습이
//! 랭킹에 닿지 못하고, (2) 원천을 갈아치울 때마다 랭킹 스케일이 달라져 평가 결과를
//! 비교할 수 없다. 로그 스케일로 옮겨 두 문제를 함께 없앤다.
//!
//! 두 갈래가 같은 원천 신호를 읽고 서로 다른 표를 낸다: `lexicon`은 낱말 점수표를,
//! `bigram`은 언어모델을 만든다. 둘이 나눠 쓰는 것은 이 파일의 `SourceSignal`과
//! 점수 눈금(`scale`)뿐이라 서로를 부르지 않는다.

mod bigram;
mod lexicon;
mod scale;

pub use bigram::{BigramReport, normalize_bigrams};
pub use lexicon::{Contribution, NormalizeReport, normalize};

use crate::recipe::Role;

/// 한 원천이 병합에 내놓는 것 — 역할·가중치와 추출된 신호를 함께 본다.
pub struct SourceSignal<'call> {
    pub role: Role,
    pub weight: f64,
    /// 사전이 보증한 낱말 — (낱말, 흔함 등급). 인벤토리 역할일 때만 표제어 집합에 들어간다.
    pub attested: &'call [(String, f64)],
    /// 코퍼스에서 관측된 낱말 — (낱말, 관측 횟수). 인벤토리 역할에서는 낱말 점수에
    /// 더하지 않고 문맥 이득을 재는 데만 쓴다 — 사전 등재는 실사용 횟수가 아니다.
    pub observed: &'call [(String, u64)],
    /// (앞말 번호, 뒷말 번호, 관측 횟수) — 문맥을 아는 원천만 채운다. 번호는 `observed`의
    /// 자리 번호다.
    pub bigrams: &'call [(u32, u32, u64)],
    /// 활용형이 뻗어 나오는 어간 — 형태소 사전만 채운다
    pub stems: &'call [String],
    /// 어절 뒤에 붙는 접사 — 형태소 사전만 채운다. 이것과 똑같은 낱말은 홀로 쓰이는
    /// 어절이 아니므로 승격 후보에서 뺀다.
    pub affixes: &'call [String],
}

impl SourceSignal<'_> {
    /// 이 원천이 낸 낱말 전부 — 보증한 것과 관측한 것을 가리지 않는다.
    fn words(&self) -> impl Iterator<Item = &str> {
        self.attested
            .iter()
            .map(|(word, _)| word.as_str())
            .chain(self.observed.iter().map(|(word, _)| word.as_str()))
    }
}

/// 두 갈래가 같은 모양의 원천을 세워 두고 시험하므로 여기 한 벌만 둔다.
#[cfg(test)]
pub(super) mod fixture {
    use super::SourceSignal;
    use crate::recipe::{CharacterSet, LexiconEncoding, LexiconRules, Role};

    pub fn rules(max_words: usize) -> LexiconRules {
        LexiconRules {
            encoding: LexiconEncoding::Utf8,
            character_set: CharacterSet::LatinLowercase,
            max_words,
            minimum_word_length: 2,
            accept_inflections: false,
            admission: None,
        }
    }

    /// 표제어를 보증하는 원천
    pub fn inventory<'a>(attested: &'a [(String, f64)]) -> SourceSignal<'a> {
        SourceSignal {
            role: Role::Inventory,
            weight: 1.0,
            attested,
            observed: &[],
            bigrams: &[],
            stems: &[],
            affixes: &[],
        }
    }

    /// 증거만 대는 코퍼스 원천
    pub fn corpus<'a>(role: Role, observed: &'a [(String, u64)]) -> SourceSignal<'a> {
        SourceSignal {
            role,
            weight: 1.0,
            attested: &[],
            observed,
            bigrams: &[],
            stems: &[],
            affixes: &[],
        }
    }
}
