//! 문장 코퍼스의 집계 — 낱말 빈도와 이웃한 낱말 짝.
//!
//! 원천이 문장을 어떻게 담고 있든(TSV 한 줄, XML 안의 wikitext, JSON 필드) 세는 방식은
//! 같으므로 여기 모은다. 어절을 어떻게 끊는지가 사전 품질을 좌우하는 자리다.

use super::Signal;
use std::collections::HashMap;

/// 집계 표 하나가 이 항목 수를 넘으면 한 번짜리를 쳐낸다.
const PRUNE_THRESHOLD: usize = 8_000_000;

/// 버린 마크업 자리에 남기는 표지. 글자도 숫자도 아니므로 이 표지가 닿은 어절은
/// 토큰화에서 통째로 버려진다.
///
/// 줄바꿈으로 표시하면 어절에 붙어 있던 마크업이 조사만 홀로 남긴다 — `{{일본어|…}}라고`가
/// "라고"라는 낱말을 만든다. 교착어에서 이 파편은 어떤 실제 낱말보다도 흔하다. 어절
/// 사이에 있던 마크업은 앞뒤 어절을 그대로 두고 사슬만 끊는다.
pub const DISCARDED_MARK: char = char::REPLACEMENT_CHARACTER;

/// 공백으로 끊은 조각에서 낱말을 꺼낸다. 양끝의 구두점·괄호는 떼어 내지만, 안쪽에
/// 글자가 아닌 것이 남아 있으면 그 어절을 통째로 버린다.
///
/// 안쪽에서 잘라 이으면 없던 낱말이 생긴다 — "2026년에"를 숫자에서 자르면 "년에"가,
/// "Windows는"을 라틴에서 자르면 "는"이 낱말로 둔갑한다. 한국어처럼 조사가 어절에
/// 붙는 언어에서는 이 파편이 어떤 실제 낱말보다도 흔해져 상위 어휘를 통째로 차지한다.
fn word_of(chunk: &str) -> Option<&str> {
    let is_letter = |character: char| character.is_alphabetic() || character == '\'';
    // 숫자와 버림 표지는 다듬지 않는다 — 떼어 내면 "2026년에"가 "년에"로, 버려진
    // 마크업에 붙어 있던 조사가 "라고"로 남는다. 어절 안에 그런 것이 섞여 있다는
    // 것은 그 어절이 통째로 낱말이 아니라는 뜻이다.
    let trimmed = chunk.trim_matches(|character: char| {
        !(is_letter(character) || character.is_numeric() || character == DISCARDED_MARK)
    });
    (!trimmed.is_empty() && trimmed.chars().all(is_letter)).then_some(trimmed)
}

/// 문장 코퍼스의 집계 — 낱말 빈도와 이웃한 낱말 짝. 원천이 문장을 어떻게 담고
/// 있든(TSV 한 줄, XML 안의 wikitext) 세는 방식은 같으므로 여기 모은다.
#[derive(Default)]
pub struct CorpusCounts {
    pub counts: HashMap<String, u64>,
    pub pairs: HashMap<(String, String), u64>,
}

impl CorpusCounts {
    /// 대소문자가 있는 스크립트에서는 소문자로 나타난 출현만 센다 — 예문 인물 이름("Tom")이
    /// 극단적으로 흔해서 대문자 출현을 함께 세면 이름이 흔한 낱말을 밀어내고 상위권을
    /// 차지한다. 문장 첫머리 출현을 잃는 대신(흔한 낱말은 문장 중간에도 충분히 나타난다)
    /// 고유명사 편향이 사라진다. 걸러진 낱말은 짝의 사슬도 끊는다 — 건너뛰어 이으면
    /// 실제로 이웃하지 않은 낱말이 문맥으로 둔갑한다.
    pub fn read_sentence(&mut self, sentence: &str, cased: bool) {
        // 줄바꿈은 그 자체로 사슬을 끊는다 — 원천에서 버린 구간이 남긴 자리이거나
        // 문단 경계이지, 이웃한 낱말 사이가 아니다.
        for line in sentence.split('\n') {
            self.read_line(line, cased);
        }
    }

    fn read_line(&mut self, line: &str, cased: bool) {
        let mut previous: Option<&str> = None;
        for token in line.split_whitespace().map(word_of) {
            let Some(token) = token else {
                previous = None;
                continue;
            };
            if cased && token.chars().next().is_some_and(char::is_uppercase) {
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
    pub fn prune_if_large(&mut self) {
        if self.counts.len() > PRUNE_THRESHOLD {
            self.counts.retain(|_, count| *count > 1);
        }
        if self.pairs.len() > PRUNE_THRESHOLD {
            self.pairs.retain(|_, count| *count > 1);
        }
    }

    pub fn finish(self, minimum_count: u64) -> Signal {
        Signal {
            observed: self
                .counts
                .into_iter()
                .filter(|(_, count)| *count >= minimum_count)
                .collect(),
            bigrams: self
                .pairs
                .into_iter()
                .map(|((left, right), count)| (left, right, count))
                .collect(),
            ..Signal::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 어절 안쪽에서 자르면 조사·의존명사 파편이 낱말로 둔갑한다 — 교착어에서 이 파편은
    /// 어떤 실제 낱말보다도 흔해서 상위 어휘를 통째로 차지한다.
    #[test]
    fn words_are_whole_eojeol_not_fragments() {
        let mut corpus = CorpusCounts::default();
        corpus.read_sentence("2026년에 Windows는 나왔다.", false);
        assert_eq!(corpus.counts.get("나왔다"), Some(&1));
        // 숫자가 섞인 어절은 통째로 버린다. 스크립트가 섞인 어절은 온전히 남으므로
        // ("Windows는") 표제어 문자 집합 필터가 뒤에서 걸러 낸다 — 어느 쪽이든 조사
        // 파편은 생기지 않는다.
        for fragment in ["년에", "는", "2026년에"] {
            assert_eq!(corpus.counts.get(fragment), None, "파편이 남음: {fragment}");
        }
        // 버린 어절은 사슬을 끊는다
        assert_eq!(corpus.pairs.len(), 1);
        assert!(
            corpus
                .pairs
                .contains_key(&("Windows는".to_string(), "나왔다".to_string()))
        );
    }
}
