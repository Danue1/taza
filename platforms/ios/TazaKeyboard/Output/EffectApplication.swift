import UIKit

/// 코어가 돌려준 Effect를 textDocumentProxy 조작으로 옮기는 유일한 통로.
/// 문서에 글자를 넣고 지우는 일은 전부 여기를 지난다.
extension KeyboardViewController {
    func selectCandidate(_ index: Int) {
        guard let session = activeSession else { return }
        playKeyFeedback()
        apply(effects: session.handleEvent(
            event: .candidateSelected(index: UInt32(index)),
            context: currentContext()
        ))
        refreshAutoShift()
    }

    /// 앞선 시한은 갈아 끼운다 — 남겨 두면 이어 누르는 도중에 울려 주기를 일찍 끊는다.
    private func scheduleMultitapTimeout(milliseconds: UInt32) {
        multitapTimer?.invalidate()
        multitapTimer = Timer.scheduledTimer(
            withTimeInterval: TimeInterval(milliseconds) / 1000,
            repeats: false
        ) { [weak self] _ in
            self?.activeSession?.timerFired()
        }
    }

    private func deleteCharacters(_ count: Int) {
        for _ in 0..<count {
            textDocumentProxy.deleteBackward()
        }
    }

    func apply(effects: [FfiEffect]) {
        applyingEffects = true
        defer {
            applyingEffects = false
            // 우리가 남긴 문서 끝 — 다음 알림이 이것과 어긋나면 바깥에서 움직인 것이다
            lastAppliedTail = textDocumentProxy.documentContextBeforeInput ?? ""
        }
        for effect in effects {
            switch effect {
            case .commitText(let text):
                // 코어 의미론: 활성 composing 구간을 치환하며 확정
                deleteCharacters(composingOnScreen.count)
                composingOnScreen = ""
                textDocumentProxy.insertText(text)
            case .setComposing(let text, _):
                let common = zip(composingOnScreen, text)
                    .prefix(while: { $0 == $1 })
                    .count
                deleteCharacters(composingOnScreen.count - common)
                textDocumentProxy.insertText(String(text.dropFirst(common)))
                composingOnScreen = text
            case .clearComposing:
                deleteCharacters(composingOnScreen.count)
                composingOnScreen = ""
            case .deleteBackward(let codePoints):
                deleteCharacters(Int(codePoints))
            case .updateCandidates(let candidates):
                candidateBar.setCandidates(candidates.map(candidateModel))
            case .moveCursor(let offset):
                textDocumentProxy.adjustTextPosition(byCharacterOffset: Int(offset))
            case .setTimer(let milliseconds):
                scheduleMultitapTimeout(milliseconds: milliseconds)
            }
        }
    }

    /// textDidChange는 우리 편집 뒤에도 호출된다. 문서 끝이 우리가 남긴 그대로면 무시하고,
    /// 어긋났을 때(커서 이동·외부 수정)만 코어 finalize로 동기화한다 — 문맥 재동기화
    /// (reconciliation) 규칙의 셸 구현.
    ///
    /// 조합 창이 없는 언어(라틴)에서도 판단할 것이 있다: 커서가 다른 자리로 옮겨 갔으면
    /// 후보 바에 남은 것은 그 자리의 어절이 아니다. 그래서 기준을 조합 창 하나에 걸지
    /// 않고, 우리가 마지막으로 남긴 문서 끝과 견준다.
    override func textDidChange(_ textInput: UITextInput?) {
        super.textDidChange(textInput)
        // 다른 필드로 초점이 옮겨 갔을 수 있다 — 화면부터 그 필드에 맞춘다
        updateField()
        guard !applyingEffects, let session = activeSession else { return }
        let tail = textDocumentProxy.documentContextBeforeInput ?? ""
        let unchanged = composingOnScreen.isEmpty
            ? lastAppliedTail.map { $0 == tail } ?? true
            : tail.hasSuffix(composingOnScreen)
        if unchanged { return }
        composingOnScreen = ""
        apply(effects: session.handleEvent(event: .cursorMoved, context: currentContext()))
    }
}
