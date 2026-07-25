import UIKit

/// 언어 키를 길게 눌렀을 때 뜨는 목록. 항목은 설정 항목과 언어 항목이 같은 줄
/// 스타일을 공유한다 — 설정 앱의 목록과 같은 토큰을 쓴다.
public final class PopupMenuView: UIView {
    public struct Item {
        public let title: String
        /// 배열 이름처럼 오른쪽에 붙는 보조 표기
        public let detail: String?
        public let isSelected: Bool
        public let isEnabled: Bool
        public let accessibilityHint: String?

        public init(
            title: String,
            detail: String? = nil,
            isSelected: Bool = false,
            isEnabled: Bool = true,
            accessibilityHint: String? = nil
        ) {
            self.title = title
            self.detail = detail
            self.isSelected = isSelected
            self.isEnabled = isEnabled
            self.accessibilityHint = accessibilityHint
        }
    }

    public var onSelect: ((Int) -> Void)?

    private let stack = UIStackView()

    public init(items: [Item]) {
        super.init(frame: .zero)

        backgroundColor = TazaTheme.Color.popupSurface
        layer.cornerRadius = TazaTheme.Popup.cornerRadius
        layer.cornerCurve = .continuous
        layer.shadowColor = UIColor.black.cgColor
        layer.shadowOpacity = TazaTheme.Popup.shadowOpacity
        layer.shadowRadius = TazaTheme.Popup.shadowRadius
        layer.shadowOffset = CGSize(width: 0, height: 4)

        stack.axis = .vertical
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        NSLayoutConstraint.activate([
            stack.topAnchor.constraint(equalTo: topAnchor),
            stack.bottomAnchor.constraint(equalTo: bottomAnchor),
            stack.leadingAnchor.constraint(equalTo: leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: trailingAnchor),
            widthAnchor.constraint(equalToConstant: TazaTheme.Popup.width),
        ])

        for (index, item) in items.enumerated() {
            if index > 0 {
                let separator = UIView()
                separator.backgroundColor = TazaTheme.Color.separator
                separator.isAccessibilityElement = false
                separator.heightAnchor.constraint(equalToConstant: 1).isActive = true
                stack.addArrangedSubview(separator)
            }
            stack.addArrangedSubview(makeRow(item: item, index: index))
        }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("사용하지 않음")
    }

    private func makeRow(item: Item, index: Int) -> UIView {
        let row = UIButton(
            configuration: .plain(),
            primaryAction: UIAction { [weak self] _ in self?.onSelect?(index) }
        )
        var configuration = UIButton.Configuration.plain()
        configuration.contentInsets = NSDirectionalEdgeInsets(
            top: 0, leading: 14, bottom: 0, trailing: 14
        )
        configuration.attributedTitle = AttributedString(
            item.title,
            attributes: AttributeContainer([
                .font: item.isSelected
                    ? TazaTheme.Typography.popupItemSelected
                    : TazaTheme.Typography.popupItem,
                .foregroundColor: item.isEnabled
                    ? TazaTheme.Color.label
                    : TazaTheme.Color.secondaryLabel,
            ])
        )
        if let detail = item.detail {
            configuration.attributedSubtitle = AttributedString(
                detail,
                attributes: AttributeContainer([
                    .font: UIFont.systemFont(ofSize: 13),
                    .foregroundColor: TazaTheme.Color.secondaryLabel,
                ])
            )
        }
        configuration.titleAlignment = .leading
        row.configuration = configuration
        row.contentHorizontalAlignment = .leading
        row.isEnabled = item.isEnabled
        row.backgroundColor = item.isSelected ? TazaTheme.Color.selection : .clear
        row.heightAnchor.constraint(
            greaterThanOrEqualToConstant: TazaTheme.Popup.itemHeight
        ).isActive = true
        row.accessibilityLabel = item.title
        row.accessibilityHint = item.accessibilityHint
        if item.isSelected {
            row.accessibilityTraits.insert(.selected)
        }
        return row
    }
}
