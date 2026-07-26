import UIKit

/// 그리드가 올려 준 좌표를 코어로 넘기는 길. 어떤 키인지는 코어가 판정하고, 셸은
/// 누름·뗌·길게 누르기라는 플랫폼 제스처를 구분하는 데까지만 관여한다.
extension KeyboardViewController {
    func pressBegan(at point: CGPoint) {
        if languageMenu != nil {
            dismissLanguageMenu()
            return
        }
        guard let session = activeSession else { return }
        let key = session.keyAt(x: Float(point.x), y: Float(point.y))
        // 삭제만 누르는 순간 듣는다 — 길게 누르면 그대로 이어 지워야 하기 때문이다.
        // 나머지는 손을 뗄 때 확정한다: 손가락을 굴려 옆 키로 옮겨 뗄 수 있고, 길게 눌러
        // 여는 팝업(변형 문자·언어 목록)과도 뜻이 겹치지 않는다.
        if key.role == .backspace {
            activateKey(at: point)
            return
        }
        if key.role == .shift,
           keyboardPreferences.shiftDoubleTapLock,
           consumeShiftDoubleTap(session: session)
        {
            return
        }
        deferredPressPoint = point
    }

    /// 스페이스바를 좌우로 민 것 — 설정으로 켠 사람만 쓴다. 어느 키에서 시작했는지는
    /// 코어가 판정한다.
    func swiped(at point: CGPoint, toLeft: Bool) {
        deferredPressPoint = nil
        guard keyboardPreferences.spaceSwipeLanguage,
              let session = activeSession,
              session.keyAt(x: Float(point.x), y: Float(point.y)).role == .space
        else {
            return
        }
        let languages = preferences.enabledLanguages
        guard languages.count > 1,
              let index = languages.firstIndex(of: currentLanguage)
        else {
            return
        }
        let step = toLeft ? languages.count - 1 : 1
        switchLanguage(to: languages[(index + step) % languages.count])
    }

    func touchEnded(at point: CGPoint) {
        guard deferredPressPoint != nil else { return }
        deferredPressPoint = nil
        activateKey(at: point)
    }

    func activateKey(at point: CGPoint) {
        guard let session = activeSession else { return }
        playKeyFeedback()
        let result = session.pressAt(
            x: Float(point.x),
            y: Float(point.y),
            context: currentContext()
        )
        apply(effects: result.effects)
        if result.requestsNextLanguage {
            switchLanguage(to: preferences.language(after: currentLanguage))
        }
        // 자동 대문자화가 shift를 올렸을 수 있다 — 코어가 프레임 갱신 여부까지 알려 준다
        if result.layoutChanged {
            refreshFrame()
        }
    }

    /// 좌표를 거치지 않은 입력(삭제 반복·후보 선택 등)을 넣은 뒤 문장 시작 여부를 다시
    /// 본다. `pressAt`은 스스로 하므로 여기 오지 않는다.
    func refreshAutoShift() {
        guard let session = activeSession else { return }
        if session.syncAutoShift(context: currentContext()) {
            refreshFrame()
        }
    }

    func longPressBegan(at point: CGPoint) {
        guard let session = activeSession else { return }
        let key = session.keyAt(x: Float(point.x), y: Float(point.y))
        switch key.role {
        case .languageSwitch:
            showLanguageMenu(near: point)
        case .space:
            session.beginCursorDrag(x: Float(point.x))
            gridView.isSkeleton = true
        case .backspace:
            beginBackspaceRepeat()
        case .character where !key.alternates.isEmpty:
            showAlternatesPopup(options: key.alternates, near: point)
        default:
            // 길게 눌러도 할 일이 없는 키는 평범한 누름으로 남는다 — 손을 떼면 들어간다
            return
        }
        deferredPressPoint = nil
    }

    func longPressChanged(at point: CGPoint) {
        if backspaceRepeatTimer != nil { return }
        if let popup = alternatesPopup {
            let pointInPopup = view.convert(
                CGPoint(x: point.x * gridView.bounds.width, y: 0),
                from: gridView
            )
            popup.updateHighlight(atX: pointInPopup.x - popup.frame.minX)
            return
        }
        if languageMenu == nil, let session = activeSession {
            apply(effects: session.updateCursorDrag(
                x: Float(point.x),
                context: currentContext()
            ))
        }
    }

    func longPressEnded() {
        if backspaceRepeatTimer != nil {
            endBackspaceRepeat()
            return
        }
        if let popup = alternatesPopup {
            let alternate = popup.highlightedOption
            dismissAlternatesPopup()
            commitAlternate(alternate)
            return
        }
        if languageMenu == nil {
            gridView.isSkeleton = false
            activeSession?.endCursorDrag()
        }
    }
}
