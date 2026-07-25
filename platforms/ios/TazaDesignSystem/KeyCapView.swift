import UIKit

/// 키 하나의 겉모습. 터치는 받지 않는다 — 히트 테스트는 코어가 좌표로 판정하므로
/// 터치는 키보드 판이 통째로 받고, 이 뷰는 그리기와 접근성만 맡는다.
public final class KeyCapView: UIView {
    public enum Appearance {
        /// 문자 키 — 밝은 바탕
        case letter
        /// shift·삭제·심볼·엔터 등 — 한 톤 어두운 바탕 (순정 관례)
        case control
        /// 언어 키 — 제어 키 바탕에 굵은 글자
        case language
        /// 스페이스 — 순정은 문자 키와 같은 밝은 바탕에, 현재 언어를 작은 회색
        /// 글자로 오른쪽에 흘려 놓는다
        case space
        /// 검색 필드의 리턴키처럼 필드가 강조하는 키 — 순정은 강조색 바탕에 흰 글자
        case emphasized
        /// 숫자 패드 좌하단처럼 순정이 비워 두는 자리 — 아무것도 그리지 않는다
        case blank

        var usesLetterSurface: Bool {
            self == .letter || self == .space
        }
    }

    public var isPressed: Bool = false {
        didSet { updateColors() }
    }

    public var isActive: Bool = false {
        didSet { updateColors() }
    }

    private let appearance: Appearance
    private let labelView = UILabel()

    public init(
        label: String,
        appearance: Appearance,
        fontSize: CGFloat,
        cornerRadius: CGFloat,
        accessibilityLabel: String,
        accessibilityHint: String?,
        accessibilityValue: String?,
        alternates: [String]
    ) {
        self.appearance = appearance
        super.init(frame: .zero)

        isUserInteractionEnabled = false
        layer.cornerRadius = cornerRadius
        layer.cornerCurve = .continuous
        if appearance != .blank {
            // 순정 키캡은 아래로 1pt 그림자를 깔아 판에서 살짝 떠 보인다
            layer.shadowColor = TazaTheme.Key.shadowColor.cgColor
            layer.shadowOffset = CGSize(width: 0, height: 1)
            layer.shadowRadius = 0
            layer.shadowOpacity = TazaTheme.Key.shadowOpacity
        }

        labelView.text = label
        labelView.adjustsFontSizeToFitWidth = true
        labelView.minimumScaleFactor = 0.6
        labelView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(labelView)

        switch appearance {
        case .space:
            labelView.textAlignment = .right
            labelView.font = TazaTheme.Typography.spaceLabel
            labelView.textColor = TazaTheme.Color.secondaryLabel
        case .language:
            labelView.textAlignment = .center
            labelView.font = TazaTheme.Typography.languageKeyLabel(size: fontSize)
            labelView.textColor = TazaTheme.Color.label
        case .emphasized:
            labelView.textAlignment = .center
            labelView.font = TazaTheme.Typography.keyLabel(size: fontSize)
            labelView.textColor = TazaTheme.Color.emphasizedLabel
        case .letter, .control, .blank:
            labelView.textAlignment = .center
            labelView.font = TazaTheme.Typography.keyLabel(size: fontSize)
            labelView.textColor = TazaTheme.Color.label
        }

        let trailingInset: CGFloat = appearance == .space ? -10 : -2
        NSLayoutConstraint.activate([
            labelView.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 2),
            labelView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: trailingInset),
            labelView.centerYAnchor.constraint(equalTo: centerYAnchor),
        ])

        // 접근성은 계약의 일부 — 라벨·롤·상태를 코어가 준 값 그대로 노출한다.
        // 빈 자리는 누를 수 없으므로 VoiceOver 순회에서도 빠진다.
        isAccessibilityElement = appearance != .blank
        accessibilityTraits = .keyboardKey
        self.accessibilityLabel = accessibilityLabel
        self.accessibilityHint = accessibilityHint
        self.accessibilityValue = accessibilityValue
        if !alternates.isEmpty {
            // VoiceOver는 길게 누르기 제스처 대신 커스텀 액션으로 변형 문자를 고른다
            accessibilityCustomActions = alternates.map { alternate in
                UIAccessibilityCustomAction(name: alternate) { [weak self] _ in
                    self?.onSelectAlternate?(alternate)
                    return true
                }
            }
        }
        updateColors()
    }

    /// VoiceOver 커스텀 액션에서 고른 변형 문자
    public var onSelectAlternate: ((String) -> Void)?

    /// VoiceOver 활성화(더블 탭) — 접근성 경로에서는 확률 판정 없이 이 키가 눌린다
    public var onAccessibilityActivate: (() -> Void)?

    public override func accessibilityActivate() -> Bool {
        onAccessibilityActivate?()
        return true
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("사용하지 않음")
    }

    public override func traitCollectionDidChange(_ previous: UITraitCollection?) {
        super.traitCollectionDidChange(previous)
        updateColors()
    }

    private func updateColors() {
        if appearance == .blank {
            backgroundColor = .clear
            return
        }
        if appearance == .emphasized {
            backgroundColor = isPressed
                ? TazaTheme.Color.accent.withAlphaComponent(0.7)
                : TazaTheme.Color.accent
            return
        }
        backgroundColor = if appearance.usesLetterSurface {
            isPressed ? TazaTheme.Color.keySurfacePressed : TazaTheme.Color.keySurface
        } else if isPressed {
            TazaTheme.Color.controlKeySurfacePressed
        } else if isActive {
            TazaTheme.Color.keySurfaceActive
        } else {
            TazaTheme.Color.controlKeySurface
        }
    }
}
