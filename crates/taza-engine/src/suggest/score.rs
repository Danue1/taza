//! 후보 점수 — 모든 항을 하나의 가산 공간에서 더한다.
//!
//! 기준 공간은 팩 lexicon의 정규화 빈도 [1, `MAX_FREQUENCY`]다. 파이프라인이 원천 빈도를
//! 로그 스케일로 옮겨 담으므로 개인화 가중치와 언어모델 가중치도 같은 자릿수로 맞춰
//! 그대로 더한다 — 이 캘리브레이션이 언어 간·모델 간 비교 가능성의 전제다.
//!
//! 거리를 사전식으로 먼저 비교하지 않는 이유는, 그러면 거리 0의 희귀어가 거리 1의
//! 흔한 낱말을 언제나 이기기 때문이다. 벌점으로 두면 어휘 규모에 따라 자연히 갈린다.

use crate::pack::lexicon::MAX_FREQUENCY;

/// 편집 1회의 벌점. 교정이 완성을 앞서려면 빈도 차이가 이만큼 나야 한다.
///
/// 완성 품질과 교정 정확도의 교환점이다. 낮추면 흔한 낱말로의 교정이 완성을 밀어내
/// 타이핑 절약이 줄고, 높이면 오타를 놓친다. 실팩(영어 480낱말) 실측:
/// MAX/8 → 교정 top3 0.937 / KS 0.330, MAX/4 → 0.930 / 0.375, MAX/3 → 0.926 / 0.385.
/// 완성은 매 글자 일어나고 교정은 오타에만 일어나므로 KS 쪽에 무게를 두어 MAX/4로 잡았다.
const EDIT_PENALTY: i64 = (MAX_FREQUENCY / 4) as i64;

/// 편집 비용의 눈금. 편집 1회가 `EDIT_UNIT`이고, 인접 키 오타처럼 그럴듯한 편집은
/// 그보다 싸다 — 정수 한 칸으로는 이 차이를 담을 수 없어 눈금을 잘게 나눈다.
pub(crate) const EDIT_UNIT: u32 = 100;

/// 사전 빈도·개인화·언어모델·편집 비용을 하나의 점수로 합친다.
/// `cost`는 `EDIT_UNIT` 눈금의 편집 비용이다.
pub(crate) fn combine(frequency: u32, personalization: u32, language_model: u32, cost: u32) -> i64 {
    i64::from(frequency) + i64::from(personalization) + i64::from(language_model)
        - i64::from(cost) * EDIT_PENALTY / i64::from(EDIT_UNIT)
}
