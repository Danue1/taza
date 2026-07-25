import UIKit

/// 통합 검색면에 담기는 것 — 코어가 그룹 단위로 내려 준다(`annotation_panel`).
public struct AnnotationPanelModel {
    public struct Item {
        public let text: String
        public let group: CandidateModel.Group

        public init(text: String, group: CandidateModel.Group) {
            self.text = text
            self.group = group
        }
    }

    public struct Group {
        /// 이 그룹의 갈래. 자주 쓰는 것처럼 갈래가 섞이는 그룹은 nil이다.
        public let group: CandidateModel.Group?
        public let label: String
        public let items: [Item]

        public init(group: CandidateModel.Group?, label: String, items: [Item]) {
            self.group = group
            self.label = label
            self.items = items
        }
    }

    public let groups: [Group]

    public init(groups: [Group]) {
        self.groups = groups
    }
}

/// 이모지·기호·얼굴 문자를 한자리에서 훑는 판. 그룹 헤더와 항목이 한 흐름으로 이어지는
/// 세로 스크롤이고, 오른쪽 레일로 그룹 사이를 건너뛴다.
///
/// 열 수는 갈래마다 다르다 — 이모지·기호는 한 글자라 여러 열이 들어가지만, 글자로 그린
/// 얼굴은 폭이 넓어 몇 열밖에 서지 못한다. 갈래가 섞이는 그룹은 가장 넓은 쪽을 따른다.
public final class AnnotationPanelView: UIView {
    public var onSelect: ((CandidateModel.Group, String) -> Void)?

    private enum Metrics {
        static let headerHeight: CGFloat = 22
        static let railWidth: CGFloat = 26
        static let itemHeight: CGFloat = 42
        static let emojiColumns = 8
        static let symbolColumns = 8
        static let emoticonColumns = 3
    }

    private var model = AnnotationPanelModel(groups: [])
    private let collectionView: UICollectionView
    private let rail = UIStackView()

    public override init(frame: CGRect) {
        collectionView = UICollectionView(
            frame: .zero,
            collectionViewLayout: UICollectionViewFlowLayout()
        )
        super.init(frame: frame)
        backgroundColor = .clear

        collectionView.collectionViewLayout = makeLayout()
        collectionView.backgroundColor = .clear
        collectionView.dataSource = self
        collectionView.delegate = self
        collectionView.register(ItemCell.self, forCellWithReuseIdentifier: ItemCell.identifier)
        collectionView.register(
            HeaderView.self,
            forSupplementaryViewOfKind: UICollectionView.elementKindSectionHeader,
            withReuseIdentifier: HeaderView.identifier
        )
        collectionView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(collectionView)

        rail.axis = .vertical
        rail.distribution = .equalSpacing
        rail.alignment = .center
        rail.translatesAutoresizingMaskIntoConstraints = false
        addSubview(rail)

        NSLayoutConstraint.activate([
            collectionView.topAnchor.constraint(equalTo: topAnchor),
            collectionView.bottomAnchor.constraint(equalTo: bottomAnchor),
            collectionView.leadingAnchor.constraint(equalTo: leadingAnchor),
            collectionView.trailingAnchor.constraint(
                equalTo: rail.leadingAnchor
            ),

            rail.topAnchor.constraint(equalTo: topAnchor, constant: Metrics.headerHeight),
            rail.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -Metrics.itemHeight / 2),
            rail.trailingAnchor.constraint(equalTo: trailingAnchor),
            rail.widthAnchor.constraint(equalToConstant: Metrics.railWidth),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("사용하지 않음")
    }

    public func setPanel(_ model: AnnotationPanelModel) {
        self.model = model
        collectionView.reloadData()
        rebuildRail()
    }

    // MARK: - 배치

    private func makeLayout() -> UICollectionViewLayout {
        UICollectionViewCompositionalLayout { [weak self] section, _ in
            let columns = self?.columns(inSection: section) ?? Metrics.emojiColumns
            let item = NSCollectionLayoutItem(
                layoutSize: NSCollectionLayoutSize(
                    widthDimension: .fractionalWidth(1.0 / CGFloat(columns)),
                    heightDimension: .absolute(Metrics.itemHeight)
                )
            )
            let group = NSCollectionLayoutGroup.horizontal(
                layoutSize: NSCollectionLayoutSize(
                    widthDimension: .fractionalWidth(1.0),
                    heightDimension: .absolute(Metrics.itemHeight)
                ),
                repeatingSubitem: item,
                count: columns
            )
            let layoutSection = NSCollectionLayoutSection(group: group)
            layoutSection.boundarySupplementaryItems = [
                NSCollectionLayoutBoundarySupplementaryItem(
                    layoutSize: NSCollectionLayoutSize(
                        widthDimension: .fractionalWidth(1.0),
                        heightDimension: .absolute(Metrics.headerHeight)
                    ),
                    elementKind: UICollectionView.elementKindSectionHeader,
                    alignment: .top
                ),
            ]
            return layoutSection
        }
    }

    /// 갈래가 정하는 열 수. 섞인 그룹은 가장 넓은 갈래(얼굴 문자)에 맞춘다 — 넓은 것이
    /// 잘리는 것보다 좁은 것이 헐렁한 편이 낫다.
    private func columns(inSection section: Int) -> Int {
        guard let group = model.groups[safe: section] else { return Metrics.emojiColumns }
        switch group.group {
        case .emoji: return Metrics.emojiColumns
        case .symbol: return Metrics.symbolColumns
        case .emoticon: return Metrics.emoticonColumns
        case .word, .none:
            return group.items.contains { $0.group == .emoticon }
                ? Metrics.emoticonColumns
                : Metrics.emojiColumns
        }
    }

    private func rebuildRail() {
        rail.arrangedSubviews.forEach { $0.removeFromSuperview() }
        for (section, group) in model.groups.enumerated() {
            let button = UIButton(
                type: .system,
                primaryAction: UIAction { [weak self] _ in self?.scroll(to: section) }
            )
            button.setAttributedTitle(
                AttributedString(
                    railLabel(for: group),
                    attributes: AttributeContainer([
                        .font: TazaTheme.Typography.panelRail,
                        .foregroundColor: TazaTheme.Color.secondaryLabel,
                    ])
                ).toUIKit(),
                for: .normal
            )
            button.accessibilityLabel = group.label
            rail.addArrangedSubview(button)
        }
    }

    /// 레일에 세우는 표식 — 그룹의 갈래를 한 글자로 알려 준다(섞인 그룹은 시계).
    private func railLabel(for group: AnnotationPanelModel.Group) -> String {
        switch group.group {
        case .emoji: "\u{1F600}"
        case .symbol: "\u{203B}"
        case .emoticon: "^_^"
        case .word, .none: "\u{1F550}"
        }
    }

    private func scroll(to section: Int) {
        guard model.groups[safe: section]?.items.isEmpty == false else { return }
        collectionView.scrollToItem(
            at: IndexPath(item: 0, section: section),
            at: .top,
            animated: true
        )
    }

    // MARK: - 셀

    private final class ItemCell: UICollectionViewCell {
        static let identifier = "item"

        private let label = UILabel()

        override init(frame: CGRect) {
            super.init(frame: frame)
            label.textAlignment = .center
            label.adjustsFontSizeToFitWidth = true
            label.minimumScaleFactor = 0.7
            label.translatesAutoresizingMaskIntoConstraints = false
            contentView.addSubview(label)
            NSLayoutConstraint.activate([
                label.topAnchor.constraint(equalTo: contentView.topAnchor),
                label.bottomAnchor.constraint(equalTo: contentView.bottomAnchor),
                label.leadingAnchor.constraint(equalTo: contentView.leadingAnchor, constant: 2),
                label.trailingAnchor.constraint(equalTo: contentView.trailingAnchor, constant: -2),
            ])
        }

        @available(*, unavailable)
        required init?(coder: NSCoder) {
            fatalError("사용하지 않음")
        }

        func show(_ item: AnnotationPanelModel.Item) {
            label.text = item.text
            label.font = TazaTheme.Typography.panelItem(group: item.group)
            label.textColor = TazaTheme.Color.label
            isAccessibilityElement = true
            accessibilityLabel = item.text
            accessibilityTraits = .button
        }

        override var isHighlighted: Bool {
            didSet {
                contentView.backgroundColor = isHighlighted
                    ? TazaTheme.Color.keySurfacePressed.withAlphaComponent(0.55)
                    : .clear
                contentView.layer.cornerRadius = TazaTheme.Key.cornerRadius
            }
        }
    }

    private final class HeaderView: UICollectionReusableView {
        static let identifier = "header"

        private let label = UILabel()

        override init(frame: CGRect) {
            super.init(frame: frame)
            label.font = TazaTheme.Typography.panelGroupLabel
            label.textColor = TazaTheme.Color.secondaryLabel
            label.translatesAutoresizingMaskIntoConstraints = false
            addSubview(label)
            NSLayoutConstraint.activate([
                label.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 10),
                label.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -10),
                label.centerYAnchor.constraint(equalTo: centerYAnchor),
            ])
        }

        @available(*, unavailable)
        required init?(coder: NSCoder) {
            fatalError("사용하지 않음")
        }

        func show(_ text: String) {
            label.text = text
        }
    }
}

extension AnnotationPanelView: UICollectionViewDataSource {
    public func numberOfSections(in collectionView: UICollectionView) -> Int {
        model.groups.count
    }

    public func collectionView(
        _ collectionView: UICollectionView,
        numberOfItemsInSection section: Int
    ) -> Int {
        model.groups[safe: section]?.items.count ?? 0
    }

    public func collectionView(
        _ collectionView: UICollectionView,
        cellForItemAt indexPath: IndexPath
    ) -> UICollectionViewCell {
        let cell = collectionView.dequeueReusableCell(
            withReuseIdentifier: ItemCell.identifier,
            for: indexPath
        )
        if let item = model.groups[safe: indexPath.section]?.items[safe: indexPath.item],
           let cell = cell as? ItemCell
        {
            cell.show(item)
        }
        return cell
    }

    public func collectionView(
        _ collectionView: UICollectionView,
        viewForSupplementaryElementOfKind kind: String,
        at indexPath: IndexPath
    ) -> UICollectionReusableView {
        let header = collectionView.dequeueReusableSupplementaryView(
            ofKind: kind,
            withReuseIdentifier: HeaderView.identifier,
            for: indexPath
        )
        if let group = model.groups[safe: indexPath.section], let header = header as? HeaderView {
            header.show(group.label)
        }
        return header
    }
}

extension AnnotationPanelView: UICollectionViewDelegate {
    public func collectionView(
        _ collectionView: UICollectionView,
        didSelectItemAt indexPath: IndexPath
    ) {
        guard let item = model.groups[safe: indexPath.section]?.items[safe: indexPath.item] else {
            return
        }
        collectionView.deselectItem(at: indexPath, animated: false)
        onSelect?(item.group, item.text)
    }
}

private extension Array {
    subscript(safe index: Int) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}

private extension AttributedString {
    /// UIButton.setAttributedTitle은 NSAttributedString을 받는다
    func toUIKit() -> NSAttributedString {
        NSAttributedString(self)
    }
}
