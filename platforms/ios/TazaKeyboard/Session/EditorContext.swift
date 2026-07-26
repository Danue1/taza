import UIKit

/// 지금 편집하고 있는 필드가 어떤 성격인지를 코어에 알리는 길.
extension KeyboardViewController {
    /// 플랫폼 inputmode(keyboardType·isSecureTextEntry)를 코어 FieldKind로 매핑.
    /// 갈래별로 순정이 어떤 화면을 쓰는지는 docs/inputmode.md의 실측표에 있다.
    private func currentFieldKind() -> FfiFieldKind {
        if textDocumentProxy.isSecureTextEntry == true {
            return .password
        }
        switch textDocumentProxy.keyboardType {
        case .emailAddress: return .email
        case .URL: return .url
        case .webSearch: return .search
        case .decimalPad: return .decimal
        case .numberPad, .numbersAndPunctuation, .asciiCapableNumberPad:
            return .number
        case .phonePad, .namePhonePad: return .phone
        default: return .text
        }
    }

    /// 편집 대상이 바뀌면 코어에 알리고 다시 그린다. 필드는 배열·리턴키·후보 바 자리를
    /// 바꾸므로(순정 관습) 이벤트가 오기 전에 화면이 먼저 맞아야 한다.
    func updateField() {
        let field = currentFieldKind()
        guard field != appliedField else { return }
        appliedField = field
        // 전환 대상 언어들도 같은 필드를 그리므로 세션 전체에 알린다
        for session in sessions.values {
            session.setField(field: field)
        }
        candidateBar.setCandidates([])
        refreshFrame()
    }

    func currentContext() -> FfiEditorContext {
        FfiEditorContext(
            textBeforeCursor: textDocumentProxy.documentContextBeforeInput,
            incognito: textDocumentProxy.isSecureTextEntry == true,
            field: currentFieldKind()
        )
    }
}
