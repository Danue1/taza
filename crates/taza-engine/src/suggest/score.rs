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
/// 점수 공간의 1/8이므로 소형 사전에서는 사실상 완성 우선이고, 어휘가 큰 실팩에서는
/// 흔한 낱말로의 교정이 희귀한 완성을 앞선다.
const EDIT_PENALTY: i64 = (MAX_FREQUENCY / 8) as i64;

/// 사전 빈도·개인화·언어모델·편집거리를 하나의 점수로 합친다.
pub(crate) fn combine(
    frequency: u32,
    personalization: u32,
    language_model: u32,
    distance: u32,
) -> i64 {
    i64::from(frequency) + i64::from(personalization) + i64::from(language_model)
        - i64::from(distance) * EDIT_PENALTY
}
