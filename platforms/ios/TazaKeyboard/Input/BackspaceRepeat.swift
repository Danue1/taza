import UIKit

/// 백스페이스를 누르고 있는 동안 이어 지우는 틱. 누를 때 한 번 지운 뒤, 길게 누르기가
/// 걸리면 이 간격으로 계속 지운다 — 오래 누를수록 간격이 줄어 빨라진다(순정 관례).
extension KeyboardViewController {
    /// 틱마다 간격에 곱하는 값 — 열 번쯤 지우면 가장 빠른 속도에 닿는다
    private static let speedUp: TimeInterval = 0.82

    func beginBackspaceRepeat() {
        backspaceRepeatInterval = keyboardPreferences.backspaceSpeed.interval.first
        scheduleBackspaceTick()
    }

    /// 한 번 지울 때마다 다음 틱을 조금 더 짧게 잡는다 — 반복 타이머로는 가속할 수 없다.
    private func scheduleBackspaceTick() {
        backspaceRepeatTimer?.invalidate()
        backspaceRepeatTimer = Timer.scheduledTimer(
            withTimeInterval: backspaceRepeatInterval,
            repeats: false
        ) { [weak self] _ in
            guard let self, let session = activeSession else { return }
            playKeyFeedback()
            apply(effects: session.handleEvent(event: .backspace, context: currentContext()))
            refreshAutoShift()
            backspaceRepeatInterval = max(
                backspaceRepeatInterval * Self.speedUp,
                keyboardPreferences.backspaceSpeed.interval.fastest
            )
            scheduleBackspaceTick()
        }
    }

    func endBackspaceRepeat() {
        backspaceRepeatTimer?.invalidate()
        backspaceRepeatTimer = nil
    }
}
