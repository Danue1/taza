import UIKit

/// 코어가 내려준 키 프레임을 디자인 시스템의 그리기 모델로 옮긴다 — 상태 없는 번역이다.
extension KeyboardViewController {
    func model(from frame: FfiKeyboardFrame) -> KeyboardFrameModel {
        KeyboardFrameModel(
            rows: frame.rows.map { row in
                row.map { key in
                    KeyModel(
                        label: key.label,
                        appearance: appearance(for: key),
                        fontSize: CGFloat(key.fontSize),
                        bounds: CGRect(
                            x: CGFloat(key.bounds.x),
                            y: CGFloat(key.bounds.y),
                            width: CGFloat(key.bounds.width),
                            height: CGFloat(key.bounds.height)
                        ),
                        accessibilityLabel: key.accessibilityLabel,
                        accessibilityValue: key.role == .languageSwitch
                            ? activeSession?.language().displayName
                            : nil,
                        accessibilityHint: hint(for: key),
                        alternates: key.alternates,
                        isActive: key.shiftActive,
                        leadingExtraGap: key.role == .backspace ? TazaTheme.Key.edgeExtraGap : 0,
                        trailingExtraGap: key.role == .shift ? TazaTheme.Key.edgeExtraGap : 0
                    )
                }
            }
        )
    }

    private func appearance(for key: FfiFrameKey) -> KeyCapView.Appearance {
        if key.emphasized {
            return .emphasized
        }
        switch key.role {
        case .character: return .letter
        case .languageSwitch: return .language
        case .space: return .space
        case .blank: return .blank
        default: return .control
        }
    }

    private func hint(for key: FfiFrameKey) -> String? {
        switch key.role {
        case .languageSwitch:
            NSLocalizedString("길게 눌러 언어와 설정 선택", comment: "언어 키 길게 누르기")
        case .space:
            NSLocalizedString("길게 눌러 커서 이동", comment: "스페이스 키 길게 누르기")
        case .character:
            key.alternates.isEmpty
                ? nil
                : NSLocalizedString("길게 눌러 변형 문자 선택", comment: "변형 문자 팝업")
        default: nil
        }
    }
}
