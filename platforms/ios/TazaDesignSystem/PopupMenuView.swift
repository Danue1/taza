import UIKit

/// 언어 키를 길게 눌렀을 때 뜨는 목록. 순정 지구본 메뉴처럼 반투명 재질 위에 줄을 세우고,
/// 지금 쓰는 언어는 오른쪽 체크로 알린다. 언어 줄 앞에는 키캡 표기를 배지로 두어 설정 앱의
/// 언어 목록과 같은 모습을 갖는다.
public final class PopupMenuView: UIView {
    public struct Item {
        public let title: String
        /// 배열 이름처럼 제목 아래 붙는 보조 표기
        public let detail: String?
        /// 줄 앞에 세우는 표기 — 언어의 키캡 라벨("한", "A")
        public let badge: String?
        public let isSelected: Bool
        public let accessibilityHint: String?

        public init(
            title: String,
            detail: String? = nil,
            badge: String? = nil,
            isSelected: Bool = false,
            accessibilityHint: String? = nil
        ) {
            self.title = title
            self.detail = detail
            self.badge = badge
            self.isSelected = isSelected
            self.accessibilityHint = accessibilityHint
        }
    }

    public var onSelect: ((Int) -> Void)?

    private let stack = UIStackView()

    public init(items: [Item]) {
        super.init(frame: .zero)

        let material = UIVisualEffectView(effect: UIBlurEffect(style: .systemMaterial))
        material.translatesAutoresizingMaskIntoConstraints = false
        addSubview(material)

        layer.cornerRadius = TazaTheme.Popup.cornerRadius
        layer.cornerCurve = .continuous
        layer.masksToBounds = false
        layer.shadowColor = UIColor.black.cgColor
        layer.shadowOpacity = TazaTheme.Popup.shadowOpacity
        layer.shadowRadius = TazaTheme.Popup.shadowRadius
        layer.shadowOffset = CGSize(width: 0, height: 4)
        material.layer.cornerRadius = TazaTheme.Popup.cornerRadius
        material.layer.cornerCurve = .continuous
        material.clipsToBounds = true

        stack.axis = .vertical
        stack.translatesAutoresizingMaskIntoConstraints = false
        material.contentView.addSubview(stack)
        NSLayoutConstraint.activate([
            material.topAnchor.constraint(equalTo: topAnchor),
            material.bottomAnchor.constraint(equalTo: bottomAnchor),
            material.leadingAnchor.constraint(equalTo: leadingAnchor),
            material.trailingAnchor.constraint(equalTo: trailingAnchor),

            stack.topAnchor.constraint(equalTo: material.contentView.topAnchor, constant: 6),
            stack.bottomAnchor.constraint(equalTo: material.contentView.bottomAnchor, constant: -6),
            stack.leadingAnchor.constraint(equalTo: material.contentView.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: material.contentView.trailingAnchor),
            widthAnchor.constraint(equalToConstant: TazaTheme.Popup.width),
        ])

        for (index, item) in items.enumerated() {
            // 구분선은 갈래가 바뀌는 자리에만 둔다 — 언어 줄끼리는 여백으로 나뉜다
            if index > 0, items[index - 1].badge != nil, item.badge == nil {
                stack.addArrangedSubview(makeSeparator())
            }
            stack.addArrangedSubview(makeRow(item: item, index: index))
        }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("사용하지 않음")
    }

    private func makeSeparator() -> UIView {
        let holder = UIView()
        let line = UIView()
        line.backgroundColor = TazaTheme.Color.separator
        line.translatesAutoresizingMaskIntoConstraints = false
        holder.addSubview(line)
        holder.isAccessibilityElement = false
        NSLayoutConstraint.activate([
            holder.heightAnchor.constraint(equalToConstant: 7),
            line.heightAnchor.constraint(equalToConstant: 1),
            line.centerYAnchor.constraint(equalTo: holder.centerYAnchor),
            line.leadingAnchor.constraint(equalTo: holder.leadingAnchor, constant: 12),
            line.trailingAnchor.constraint(equalTo: holder.trailingAnchor, constant: -12),
        ])
        return holder
    }

    private func makeRow(item: Item, index: Int) -> UIView {
        let row = MenuRowView(item: item)
        row.onTap = { [weak self] in self?.onSelect?(index) }
        return row
    }
}

/// 메뉴 한 줄 — 배지·제목·부제·체크가 한 줄에 선다. 누르는 동안 바탕이 밝아진다.
private final class MenuRowView: UIControl {
    var onTap: (() -> Void)?

    private let highlightView = UIView()

    init(item: PopupMenuView.Item) {
        super.init(frame: .zero)

        highlightView.backgroundColor = .clear
        highlightView.layer.cornerRadius = 8
        highlightView.layer.cornerCurve = .continuous
        highlightView.isUserInteractionEnabled = false
        highlightView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(highlightView)

        let title = UILabel()
        title.text = item.title
        title.font = item.isSelected
            ? TazaTheme.Typography.popupItemSelected
            : TazaTheme.Typography.popupItem
        title.textColor = TazaTheme.Color.label

        let texts = UIStackView(arrangedSubviews: [title])
        texts.axis = .vertical
        texts.spacing = 1
        if let detail = item.detail, !detail.isEmpty {
            let subtitle = UILabel()
            subtitle.text = detail
            subtitle.font = TazaTheme.Typography.popupItemDetail
            subtitle.textColor = TazaTheme.Color.secondaryLabel
            texts.addArrangedSubview(subtitle)
        }

        let row = UIStackView()
        row.axis = .horizontal
        row.alignment = .center
        row.spacing = 10
        row.isUserInteractionEnabled = false
        row.translatesAutoresizingMaskIntoConstraints = false
        if let badge = item.badge {
            row.addArrangedSubview(makeBadge(badge))
        }
        row.addArrangedSubview(texts)
        row.addArrangedSubview(UIView())
        if item.isSelected {
            let check = UIImageView(image: UIImage(systemName: "checkmark"))
            check.tintColor = TazaTheme.Color.accent
            check.contentMode = .scaleAspectFit
            check.widthAnchor.constraint(equalToConstant: 15).isActive = true
            row.addArrangedSubview(check)
        }
        addSubview(row)

        NSLayoutConstraint.activate([
            heightAnchor.constraint(greaterThanOrEqualToConstant: TazaTheme.Popup.itemHeight),
            row.topAnchor.constraint(equalTo: topAnchor, constant: 6),
            row.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -6),
            row.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            row.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),

            highlightView.topAnchor.constraint(equalTo: topAnchor, constant: 1),
            highlightView.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -1),
            highlightView.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 6),
            highlightView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -6),
        ])

        addTarget(self, action: #selector(handleTap), for: .touchUpInside)
        isAccessibilityElement = true
        accessibilityLabel = item.title
        accessibilityHint = item.accessibilityHint
        accessibilityTraits = .button
        if item.isSelected {
            accessibilityTraits.insert(.selected)
        }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("사용하지 않음")
    }

    /// 언어 키캡 표기를 그대로 옮긴 배지 — 설정 앱의 언어 목록과 같은 모습이다
    private func makeBadge(_ text: String) -> UIView {
        let label = UILabel()
        label.text = text
        label.textAlignment = .center
        label.font = TazaTheme.Typography.popupBadge
        label.textColor = TazaTheme.Color.accent
        label.backgroundColor = TazaTheme.Color.selection
        label.layer.cornerRadius = 7
        label.layer.cornerCurve = .continuous
        label.layer.masksToBounds = true
        NSLayoutConstraint.activate([
            label.widthAnchor.constraint(equalToConstant: 30),
            label.heightAnchor.constraint(equalToConstant: 30),
        ])
        return label
    }

    @objc private func handleTap() {
        onTap?()
    }

    override var isHighlighted: Bool {
        didSet {
            highlightView.backgroundColor = isHighlighted
                ? TazaTheme.Color.selection
                : .clear
        }
    }
}
