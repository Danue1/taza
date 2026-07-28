//! 점수 눈금 — 어휘 점수와 문맥 이득이 같은 자로 팩에 실린다.

use taza_engine::pack::lexicon::MAX_FREQUENCY;

/// 실사용 횟수는 로그로 눌러 담는다 — 상위 몇 낱말이 점수 공간을 독점하지 않게.
pub(super) fn logarithmic(count: f64) -> f64 {
    if count <= 0.0 {
        0.0
    } else {
        (1.0 + count).ln()
    }
}

/// 팩에 담는 점수의 눈금. 사전은 접미사가 같은 하위 그래프를 한 노드로 합쳐 저장하는데
/// (DAWG), 합칠 수 있는지는 끝 노드의 점수까지 같은지로 가린다. 점수를 65535단계로
/// 실으면 끝 노드가 거의 다 달라 공유가 막힌다 — 교착어처럼 접미사를 나눠 갖는 표제어가
/// 많을수록 손해가 크다.
///
/// 눈금을 이만큼 굵히면 한국어팩이 2188KB → 1542KB, 배포 아카이브가 1155KB → 715KB로
/// 줄면서 랭킹 지표는 그대로다(top1 0.972 / top3 0.999 / MRR 0.985 / 절약률 0.484,
/// 오교정률 0.000 모두 동일). 더 굵히면(1024, 4096) 더 줄지만 그때부터는 지표가 밀린다.
const SCORE_STEP: u32 = 256;

pub(super) fn quantize(score: f64, highest: f64) -> u32 {
    let ratio = if highest > 0.0 {
        (score / highest).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let value = 1 + (ratio * (MAX_FREQUENCY - 1) as f64).round() as u32;
    ((value + SCORE_STEP / 2) / SCORE_STEP * SCORE_STEP).clamp(1, MAX_FREQUENCY)
}
