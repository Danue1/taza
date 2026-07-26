import UIKit

/// 코어가 돌려준 Effect를 textDocumentProxy 조작으로 옮기는 유일한 통로.
/// 문서에 글자를 넣고 지우는 일은 전부 여기를 지난다.
extension KeyboardViewController {
    func selectCandidate(_ index: Int) {
        guard let session = activeSession else { return }
        apply(effects: session.handleEvent(
            event: .candidateSelected(index: UInt32(index)),
            context: currentContext()
        ))
    }

    private func deleteCharacters(_ count: Int) {
        for _ in 0..<count {
            textDocumentProxy.deleteBackward()
        }
    }

    func apply(effects: [FfiEffect]) {
        applyingEffects = true
        defer { applyingEffects = false }
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
            }
        }
    }

    /// textDidChange는 우리 편집 뒤에도 호출된다. 문서 끝이 화면 composing과 일치하면
    /// 우리 상태 그대로이므로 무시하고, 어긋났을 때(커서 이동·외부 수정)만 코어 finalize로
    /// 동기화한다 — 문맥 재동기화(reconciliation) 규칙의 셸 구현.
    override func textDidChange(_ textInput: UITextInput?) {
        super.textDidChange(textInput)
        // 다른 필드로 초점이 옮겨 갔을 수 있다 — 화면부터 그 필드에 맞춘다
        updateField()
        guard !applyingEffects, let session = activeSession else { return }
        if composingOnScreen.isEmpty { return }
        let tail = textDocumentProxy.documentContextBeforeInput ?? ""
        if tail.hasSuffix(composingOnScreen) { return }
        composingOnScreen = ""
        apply(effects: session.handleEvent(event: .cursorMoved, context: currentContext()))
    }
}
