import UIKit

/// 후보 바. 후보가 없을 때도 자리를 지켜 키보드 높이가 흔들리지 않게 한다
/// (순정도 예측 바 자리를 유지한다).
public final class CandidateBarView: UIView {
    public var onSelect: ((Int) -> Void)?

    private let stack = UIStackView()

    public init() {
        super.init(frame: .zero)
        stack.axis = .horizontal
        // 후보는 균등 폭, 구분선은 1pt 고정이라 fill로 두고 폭을 직접 맞춘다
        stack.distribution = .fill
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        NSLayoutConstraint.activate([
            stack.topAnchor.constraint(equalTo: topAnchor),
            stack.bottomAnchor.constraint(equalTo: bottomAnchor),
            stack.leadingAnchor.constraint(equalTo: leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: trailingAnchor),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("사용하지 않음")
    }

    public func setCandidates(_ candidates: [String]) {
        stack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        var buttons: [UIButton] = []
        for (index, candidate) in candidates.prefix(3).enumerated() {
            if index > 0 {
                stack.addArrangedSubview(makeSeparator())
            }
            let button = makeButton(title: candidate, index: index)
            stack.addArrangedSubview(button)
            buttons.append(button)
        }
        for button in buttons.dropFirst() {
            button.widthAnchor.constraint(equalTo: buttons[0].widthAnchor).isActive = true
        }
    }

    private func makeButton(title: String, index: Int) -> UIButton {
        var configuration = UIButton.Configuration.plain()
        configuration.attributedTitle = AttributedString(
            title,
            attributes: AttributeContainer([
                .font: TazaTheme.Typography.candidate,
                .foregroundColor: TazaTheme.Color.label,
            ])
        )
        let button = UIButton(
            configuration: configuration,
            primaryAction: UIAction { [weak self] _ in self?.onSelect?(index) }
        )
        button.accessibilityLabel = title
        button.accessibilityHint = "후보 선택"
        return button
    }

    private func makeSeparator() -> UIView {
        let separator = UIView()
        separator.backgroundColor = TazaTheme.Color.separator
        separator.isAccessibilityElement = false
        separator.setContentHuggingPriority(.required, for: .horizontal)
        separator.widthAnchor.constraint(equalToConstant: 1).isActive = true
        // fillEqually 스택에서 구분선만 폭을 고정한다
        separator.setContentCompressionResistancePriority(.required, for: .horizontal)
        return separator
    }
}
