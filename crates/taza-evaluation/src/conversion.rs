//! 변환 정확도 — 읽기 하나를 표기로 옮긴 것이 정답과 같은가.
//!
//! 오타 합성 게이트(`synthesis`)가 재는 것은 **친 것을 얼마나 잘 알아듣는가**이고 이쪽이
//! 재는 것은 **알아들은 것을 어떻게 적는가**다. 둘은 다른 축이라 한쪽 수치로 다른 쪽을
//! 미룰 수 없다 — 로마자를 한 글자도 틀리지 않게 쳐도 「きしゃ」가 汽車가 될지 記者가 될지는
//! 그 수치와 아무 상관이 없다.
//!
//! 세 눈금으로 본다. 문장을 통째로 맞혔는가(`sentence`), 글자 단위로 얼마나 겹치는가
//! (`character`), 그리고 정답을 첫 후보로 냈는가와 무관하게 **어느 자리에든 냈는가**
//! (`reachable`). 셋을 함께 보는 까닭은 고치는 방법이 다르기 때문이다: 겹침이 높은데
//! 문장이 틀리면 순위 문제이고, 닿지도 못하면 사전이나 격자 문제다.

use taza_engine::convert::Conversion;

/// 평가 쌍 하나 — 읽기와 그 자리에 사람이 적었을 표기.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversionCase {
    pub reading: String,
    pub expected: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ConversionMetrics {
    pub cases: usize,
    /// 통째로 맞힌 비율
    pub sentence: f64,
    /// 글자 단위 겹침의 평균 — 어디까지 갔는지를 본다
    pub character: f64,
    /// 문절마다 정답 조각이 후보 목록 안에 있던 비율. 순위를 고치면 닿을 수 있는 몫이다.
    pub reachable: f64,
}

/// 한 후보 안에서 정답과 겹치는 글자 수 — 순서를 지키는 최장 공통 부분열이다.
fn overlap(left: &str, right: &str) -> usize {
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    let mut previous = vec![0usize; right.len() + 1];
    let mut current = vec![0usize; right.len() + 1];
    for character in &left {
        for (to, other) in right.iter().enumerate() {
            current[to + 1] = match character == other {
                true => previous[to] + 1,
                false => current[to].max(previous[to + 1]),
            };
        }
        std::mem::swap(&mut previous, &mut current);
        current.iter_mut().for_each(|value| *value = 0);
    }
    previous[right.len()]
}

/// 문절마다 고를 수 있었던 표기 가운데 정답의 조각이 있었는가. 문절 경계가 정답과 같다는
/// 보장이 없으므로 **표기가 정답 안에 부분 문자열로 있는가**로 느슨하게 본다 — 순위만
/// 고치면 닿을 수 있는 몫을 세는 눈금이라 경계까지 맞기를 요구하면 뜻이 흐려진다.
fn reachable_ratio(conversion: &Conversion<'_>, reading: &str, expected: &str) -> f64 {
    let segments = conversion.convert(reading);
    if segments.is_empty() {
        return 0.0;
    }
    let found = segments
        .iter()
        .filter(|segment| {
            conversion
                .candidates(&segment.reading)
                .iter()
                .any(|surface| expected.contains(surface.as_str()))
        })
        .count();
    found as f64 / segments.len() as f64
}

pub fn measure(conversion: &Conversion<'_>, cases: &[ConversionCase]) -> ConversionMetrics {
    if cases.is_empty() {
        return ConversionMetrics::default();
    }
    let mut exact = 0usize;
    let mut character = 0.0;
    let mut reachable = 0.0;
    for case in cases {
        let converted: String = conversion
            .convert(&case.reading)
            .iter()
            .map(|segment| segment.surface.as_str())
            .collect();
        if converted == case.expected {
            exact += 1;
        }
        let expected_length = case.expected.chars().count().max(1);
        character += overlap(&converted, &case.expected) as f64 / expected_length as f64;
        reachable += reachable_ratio(conversion, &case.reading, &case.expected);
    }
    let total = cases.len() as f64;
    ConversionMetrics {
        cases: cases.len(),
        sentence: exact as f64 / total,
        character: character / total,
        reachable: reachable / total,
    }
}

/// mozc가 함께 배포하는 평가 셋 — `상태 · 읽기 · 표기 · 명령 · 인자 · 판`.
///
/// **회귀 사례 모음이라 어려운 쪽으로 치우쳐 있다.** 여기서 나온 절대값을 평균적인 글의
/// 변환 정확도로 읽으면 안 되고, 같은 셋 위에서 A와 B를 견주는 데 쓴다.
pub fn parse_mozc_evaluation(text: &str) -> Vec<ConversionCase> {
    text.lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let _status = fields.next()?;
            let reading = fields.next()?;
            let expected = fields.next()?;
            (!reading.is_empty() && !expected.is_empty()).then(|| ConversionCase {
                reading: reading.to_string(),
                expected: expected.to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 겹침은_차례를_지킨다() {
        assert_eq!(overlap("庭には", "庭には"), 3);
        assert_eq!(overlap("庭には", "庭に鶏"), 2);
        // 글자가 다 있어도 차례가 다르면 그만큼만 센다
        assert_eq!(overlap("には庭", "庭には"), 2);
        assert_eq!(overlap("", "庭"), 0);
    }

    #[test]
    fn 평가_셋은_주석과_빈_줄을_건너뛴다() {
        let cases = parse_mozc_evaluation(
            "# status\tinput\toutput\nOK:\tにわ\t庭\tConversion Match\t庭\t1\nOK:\t\t\t\n",
        );
        assert_eq!(
            cases,
            [ConversionCase {
                reading: "にわ".to_string(),
                expected: "庭".to_string(),
            }]
        );
    }
}
