//! 언어별 표기 지식. 추출기와 정규화가 언어를 아는 유일한 자리다.
//!
//! 트레이트를 두지 않고 값 하나로 가린다 — 목적은 언어를 갈아 끼울 수 있게 하는 것이
//! 아니라, 언어 지식이 파이프라인 곳곳에 흩어지는 것을 막는 것이다.

pub mod korean;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageProfile {
    /// 대소문자가 있고 어절이 곧 낱말인 스크립트
    Latin,
    /// 대소문자가 없고 조사·어미가 어절에 붙는 교착어
    Korean,
}

impl LanguageProfile {
    pub fn of(language_tag: &str) -> LanguageProfile {
        match language_tag {
            "ko" => LanguageProfile::Korean,
            _ => LanguageProfile::Latin,
        }
    }

    /// 대소문자가 있는 스크립트인가. 있으면 코퍼스에서 소문자 출현만 센다 — 대문자
    /// 출현을 함께 세면 예문 인물 이름이 흔한 낱말을 밀어내고 상위권을 차지한다.
    pub fn cased(self) -> bool {
        matches!(self, LanguageProfile::Latin)
    }

    /// 어절 뒤에 붙는 접사. 팩에 실려 코어가 학습 어휘의 결합형을 알아보는 데 쓰이고,
    /// 승격 판정에서 "이것만으로는 어절이 아니다"를 가리는 데도 쓰인다.
    pub fn affixes(self) -> Vec<String> {
        match self {
            LanguageProfile::Latin => Vec::new(),
            LanguageProfile::Korean => korean::particle_forms(),
        }
    }
}
