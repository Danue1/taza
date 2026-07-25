import UIKit

/// 글자를 길게 눌렀을 때 키 위에 뜨는 변형 문자 줄. 손가락을 좌우로 끌어 고르고
/// 떼면 확정된다 (순정 관례).
public final class AlternatesPopupView: UIView {
    private let optionViews: [UILabel]

    public let options: [String]
    public private(set) var highlightedIndex: Int

    public init(options: [String], metrics: TazaTheme.Metrics) {
        self.options = options
        self.highlightedIndex = 0
        self.optionViews = options.map { option in
            let label = UILabel()
            label.text = option
            label.textAlignment = .center
            label.font = TazaTheme.Typography.keyLabel(size: metrics.letterFontSize)
            label.textColor = TazaTheme.Color.label
            label.layer.cornerRadius = TazaTheme.Key.cornerRadius
            label.layer.cornerCurve = .continuous
            label.layer.masksToBounds = true
            label.isAccessibilityElement = false
            return label
        }
        super.init(frame: .zero)

        backgroundColor = TazaTheme.Color.popupSurface
        layer.cornerRadius = TazaTheme.Popup.cornerRadius
        layer.cornerCurve = .continuous
        layer.shadowColor = UIColor.black.cgColor
        layer.shadowOpacity = TazaTheme.Popup.shadowOpacity
        layer.shadowRadius = TazaTheme.Popup.shadowRadius
        layer.shadowOffset = CGSize(width: 0, height: 4)

        let stack = UIStackView(arrangedSubviews: optionViews)
        stack.axis = .horizontal
        stack.distribution = .fillEqually
        stack.spacing = 2
        stack.isLayoutMarginsRelativeArrangement = true
        stack.layoutMargins = UIEdgeInsets(top: 4, left: 4, bottom: 4, right: 4)
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        NSLayoutConstraint.activate([
            stack.topAnchor.constraint(equalTo: topAnchor),
            stack.bottomAnchor.constraint(equalTo: bottomAnchor),
            stack.leadingAnchor.constraint(equalTo: leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: trailingAnchor),
        ])
        updateHighlight()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("사용하지 않음")
    }

    /// 팝업의 자연 크기 — 옵션 하나가 키 하나 크기가 되도록 잡는다
    public func preferredSize(keySize: CGSize) -> CGSize {
        CGSize(
            width: keySize.width * CGFloat(options.count) + 8,
            height: keySize.height + 8
        )
    }

    /// 팝업 좌표계 기준 x로 강조 항목을 갱신한다
    public func updateHighlight(atX x: CGFloat) {
        guard !options.isEmpty, bounds.width > 0 else { return }
        let slot = Int(x / (bounds.width / CGFloat(options.count)))
        highlightedIndex = min(max(slot, 0), options.count - 1)
        updateHighlight()
    }

    public var highlightedOption: String {
        options[highlightedIndex]
    }

    private func updateHighlight() {
        for (index, view) in optionViews.enumerated() {
            let selected = index == highlightedIndex
            view.backgroundColor = selected ? TazaTheme.Color.accent : .clear
            view.textColor = selected ? .white : TazaTheme.Color.label
        }
    }
}
