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

    /// 편집 대상이 스스로 밝힌 성격 전부. 값의 주인이 **앱**이라 사용자 설정과 AND로
    /// 묶인다 — 코드 입력란처럼 앱이 자동 수정을 끄라고 한 자리에서 우리 설정이 이기면
    /// 안 된다. 셸은 플랫폼 값을 옮기기만 하고 결합은 코어가 한다.
    private func currentFieldTraits() -> FfiFieldTraits {
        let proxy = textDocumentProxy
        return FfiFieldTraits(
            kind: currentFieldKind(),
            returnKey: returnKey(proxy.returnKeyType),
            capitalization: capitalization(proxy.autocapitalizationType),
            // `.default`는 "앱이 말이 없다"이므로 막지 않는다
            autocorrect: proxy.autocorrectionType != .no,
            smartPunctuation: proxy.smartQuotesType != .no && proxy.smartDashesType != .no
        )
    }

    private func returnKey(_ type: UIReturnKeyType?) -> FfiReturnKey {
        switch type {
        case .go: .go
        case .google, .yahoo, .search: .search
        case .send: .send
        case .next: .next
        case .done: .done
        case .join: .join
        case .route: .route
        case .emergencyCall, .continue: .continue
        default: .return
        }
    }

    private func capitalization(_ type: UITextAutocapitalizationType?) -> FfiCapitalization {
        switch type {
        case .none: .none
        case .words: .words
        case .allCharacters: .allCharacters
        default: .sentences
        }
    }

    /// 편집 대상이 바뀌면 코어에 알리고 다시 그린다. 필드는 배열·리턴키·후보 바 자리를
    /// 바꾸므로(순정 관습) 이벤트가 오기 전에 화면이 먼저 맞아야 한다.
    func updateField() {
        let traits = currentFieldTraits()
        // 필드가 그대로여도 문맥은 달라졌을 수 있다 — 문장이 시작되는 자리인지는
        // 늘 다시 본다(자동 대문자화). 배열을 새로 만드는 일만 필드가 바뀔 때 한다.
        defer { refreshAutoShift() }
        guard traits.kind != appliedField else { return }
        appliedField = traits.kind
        // 전환 대상 언어들도 같은 필드를 그리므로 세션 전체에 알린다
        for session in sessions.values {
            session.setField(traits: traits)
        }
        candidateBar.setCandidates([])
        // 이메일·URL은 라틴 글자를 넣는 자리다 — 순정처럼 라틴 배열로 연다
        if let session = activeSession, session.fieldPrefersLatin() {
            switchToLatinLanguage()
        }
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
