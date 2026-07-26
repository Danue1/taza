import Foundation

/// shift를 두 번 눌러 고정하는 관례(순정).
extension KeyboardViewController {
    /// 두 누름 사이 허용 간격
    private static let doubleTapInterval: TimeInterval = 0.35

    /// shift를 잇달아 두 번 누른 것인지 — 두 번 누름을 알아보는 것은 플랫폼 제스처라
    /// 셸이 하고, 고정을 걸 수 있는 배열인지는 코어가 판정한다(한글은 걸지 않는다).
    func consumeShiftDoubleTap(session: KeyboardSession) -> Bool {
        let now = Date()
        let isDoubleTap = lastShiftPressedAt.map {
            now.timeIntervalSince($0) < Self.doubleTapInterval
        } ?? false
        // 세 번째 누름이 두 번째와 가까워도 다시 두 번 누름이 되지 않도록 자취를 지운다
        lastShiftPressedAt = isDoubleTap ? nil : now
        guard isDoubleTap, session.toggleShiftLock() else { return false }
        refreshFrame()
        return true
    }
}
