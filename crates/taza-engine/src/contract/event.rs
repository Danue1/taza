//! 경계를 오가는 것 — 셸이 보내는 입력과 셸이 받아 번역하는 명령.

use super::candidate::Candidate;
use super::composer::ComposingText;
use crate::keyboard::KeySignal;

/// 셸이 코어로 보내는 입력. 터치 좌표를 키로 판정하는 일은 코어가 하므로
/// (`Engine::press_at`) 셸이 이 값을 직접 만드는 경로는 물리 키보드·접근성뿐이다.
#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    /// 터치가 만든 키 신호 — 확률 판정은 코어의 히트 테스트가 한다. 물리 키보드·접근성
    /// 경로처럼 어느 키인지 확실할 때는 `KeySignal::certain`으로 만든다.
    Key(KeySignal),
    /// 한 번에 여러 글자를 넣는 키(`.com` 등). 진행 중 조합은 먼저 확정된다.
    Text(String),
    /// 같은 멀티탭 키를 이어 누름(천지인의 ㄱ→ㅋ→ㄲ) — 직전 글자를 갈아 끼운다.
    /// 몇 번째 누름인지는 코어가 판정하므로 셸은 이 이벤트를 만들지 않는다.
    Retap(char),
    Backspace,
    Separator(char),
    CandidateSelected(usize),
    CursorMoved,
    /// 스페이스바를 길게 눌러 끄는 커서 이동. 값은 논리적 이동 칸수(부호 = 방향)로,
    /// 코어가 포인터 이동량에서 산출한다.
    CursorDrag(i32),
    FocusLost,
}

/// 셸이 플랫폼 API로 번역하는 선언적 명령. 셸은 번역만 하고 판단하지 않는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// 활성 composing 구간이 있으면 그것을 치환하며 확정한다
    /// (iOS insertText / Android commitText의 공통 의미론)
    CommitText(String),
    SetComposing(ComposingText),
    /// composing 구간의 텍스트를 제거하고 composing 상태를 끝낸다.
    /// 주의: iOS unmarkText / Android finishComposingText는 "확정"이므로 그대로 쓰면
    /// 안 된다 — 빈 문자열로 치환 후 종료해야 한다 (셸 계약).
    ClearComposing,
    /// 코드포인트 수. iOS deleteBackward는 count 미보장이므로 셸은 적용 후 문맥 재동기화 필요
    DeleteBackward(usize),
    UpdateCandidates(Vec<Candidate>),
    /// 커서를 논리적으로 옮긴다(부호 = 방향, 단위 = 코드포인트).
    /// RTL에서도 의미는 "논리적 이동"으로 고정 — 시각적 방향은 플랫폼이 해석한다.
    MoveCursor(i32),
    /// 밀리초 뒤에 `Engine::timer_fired`를 부르라는 요청. 앞선 타이머는 갈아 끼운다 —
    /// 멀티탭 주기가 이어질 때마다 시한이 새로 시작해야 하기 때문이다. 끄는 명령은 없다:
    /// 주기가 이미 끝난 뒤에 울린 타이머는 코어에서 아무 일도 하지 않으므로, 셸이
    /// "지금 꺼도 되는가"를 판단할 일이 없다.
    SetTimer(u32),
}
