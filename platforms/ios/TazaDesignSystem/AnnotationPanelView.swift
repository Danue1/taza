import UIKit

/// 이모지·기호·얼굴 문자를 한자리에서 훑는 판. 순정 이모지 화면처럼 가로로 넘기고,
/// 묶음 이름은 항목 위에 붙어 따라오며, 레일은 코어가 비워 둔 하단 자리에 앉아
/// 문자 복귀·삭제 키와 한 줄을 이룬다.
///
/// 항목 폭은 갈래마다 다르다 — 이모지·기호는 한 글자라 좁게 서지만, 글자로 그린 얼굴은
/// 폭이 넓어 자리를 많이 쓴다. 갈래가 섞이는 묶음은 가장 넓은 쪽을 따른다.
public final class AnnotationPanelView: UIView {
    public var onSelect: ((CandidateModel.Group, String) -> Void)?

    private enum Metrics {
        /// 항목 한 칸 — 글자 크기(`panelItem`)가 겨우 들어갈 만큼만 잡는다. 칸이 작을수록
        /// 한 화면에 더 많이 서고, 이 값을 더 줄이면 이모지가 잘린다.
        static let itemHeight: CGFloat = 32
        static let emojiWidth: CGFloat = 36
        static let symbolWidth: CGFloat = 36
        static let emoticonWidth: CGFloat = 104
        /// 그룹 사이 틈 — 어디서 묶음이 갈리는지 눈으로 잡히도록 넉넉히 둔다
        static let groupSpacing: CGFloat = 30
        /// 레일 표식의 아이콘 크기와 지금 자리를 알리는 원의 지름
        static let railIconSize: CGFloat = 17
        static let railMarkerSize: CGFloat = 32
    }

    private var model = AnnotationPanelModel(groups: [])
    private let collectionView: UICollectionView
    private let rail = UIStackView()
    /// 레일을 끄는 동안 같은 묶음으로 거듭 스크롤하지 않도록 마지막 목적지를 기억한다
    private var scrolledSection: Int?

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
        addSubview(collectionView)

        rail.axis = .horizontal
        rail.distribution = .fillEqually
        rail.alignment = .fill
        addSubview(rail)
        // 표식을 누른 채 옆으로 끌면 손가락 아래 묶음으로 곧장 옮겨 간다(순정 관례).
        // 최소 시간을 0으로 두어 탭도 같은 길을 지난다.
        let drag = UILongPressGestureRecognizer(target: self, action: #selector(dragRail(_:)))
        drag.minimumPressDuration = 0
        rail.addGestureRecognizer(drag)

    }

    /// 레일이 앉을 자리 — 코어가 하단 행에 비워 둔 칸(문자 복귀 키와 삭제 키 사이)이다.
    /// 셸이 자리를 지어내지 않도록 프레임을 밖에서 받는다.
    public var railFrame: CGRect = .zero {
        didSet { setNeedsLayout() }
    }

    public override func layoutSubviews() {
        super.layoutSubviews()
        rail.frame = railFrame
        collectionView.frame = CGRect(
            x: 0,
            y: 0,
            width: bounds.width,
            height: railFrame.isEmpty ? bounds.height : railFrame.minY
        )
        updateRailPosition()
    }

    /// 판이 덮고 있어도 자기 것이 없는 자리는 아래 키가 받는다 — 검색면은 키 행 위까지
    /// 자리를 차지하지만 그 줄의 키를 가로채지는 않는다.
    public override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        let hit = super.hitTest(point, with: event)
        return hit === self ? nil : hit
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("사용하지 않음")
    }

    public func setPanel(_ model: AnnotationPanelModel) {
        self.model = model
        scrolledSection = nil
        collectionView.reloadData()
        collectionView.layoutIfNeeded()
        updateRailPosition()
        rebuildRail()
    }

    // MARK: - 배치

    private func makeLayout() -> UICollectionViewLayout {
        let configuration = UICollectionViewCompositionalLayoutConfiguration()
        configuration.scrollDirection = .horizontal
        return UICollectionViewCompositionalLayout(
            sectionProvider: { [weak self] section, environment in
                let width = self?.itemWidth(inSection: section) ?? Metrics.emojiWidth
                // 한 줄(세로 묶음)에 몇 개가 서는지는 판 높이가 정한다 — 채울 수 있는 만큼
                // 세로로 채우고 나머지가 옆으로 이어진다
                let rows = max(
                    Int(environment.container.contentSize.height / Metrics.itemHeight),
                    1
                )
                let item = NSCollectionLayoutItem(
                    layoutSize: NSCollectionLayoutSize(
                        widthDimension: .absolute(width),
                        heightDimension: .absolute(Metrics.itemHeight)
                    )
                )
                let group = NSCollectionLayoutGroup.vertical(
                    layoutSize: NSCollectionLayoutSize(
                        widthDimension: .absolute(width),
                        heightDimension: .absolute(Metrics.itemHeight * CGFloat(rows))
                    ),
                    repeatingSubitem: item,
                    count: rows
                )
                let layoutSection = NSCollectionLayoutSection(group: group)
                layoutSection.contentInsets = NSDirectionalEdgeInsets(
                    top: 0,
                    leading: 0,
                    bottom: 0,
                    trailing: Metrics.groupSpacing
                )
                return layoutSection
            },
            configuration: configuration
        )
    }

    /// 갈래가 정하는 항목 폭. 섞인 그룹은 가장 넓은 갈래(얼굴 문자)에 맞춘다 — 넓은 것이
    /// 잘리는 것보다 좁은 것이 헐렁한 편이 낫다.
    private func itemWidth(inSection section: Int) -> CGFloat {
        guard let group = model.groups[safe: section] else { return Metrics.emojiWidth }
        switch group.group {
        case .emoji: return Metrics.emojiWidth
        case .symbol: return Metrics.symbolWidth
        case .emoticon: return Metrics.emoticonWidth
        case .word, .none:
            return group.items.contains { $0.group == .emoticon }
                ? Metrics.emoticonWidth
                : Metrics.emojiWidth
        }
    }

    private func rebuildRail() {
        rail.arrangedSubviews.forEach { $0.removeFromSuperview() }
        for group in model.groups {
            // 자리를 알리는 바탕은 늘 정원이어야 하므로, 늘어나는 칸 안에 크기를 고정한
            // 원을 따로 둔다 — 칸을 곧장 칠하면 묶음 수에 따라 타원이 된다.
            let slot = UIView()
            let marker = UIImageView(
                image: UIImage(
                    systemName: railSymbol(for: group),
                    withConfiguration: UIImage.SymbolConfiguration(
                        pointSize: Metrics.railIconSize,
                        weight: .regular
                    )
                )
            )
            marker.tintColor = TazaTheme.Color.label
            marker.contentMode = .center
            marker.layer.cornerRadius = Metrics.railMarkerSize / 2
            marker.translatesAutoresizingMaskIntoConstraints = false
            slot.addSubview(marker)
            NSLayoutConstraint.activate([
                marker.widthAnchor.constraint(equalToConstant: Metrics.railMarkerSize),
                marker.heightAnchor.constraint(equalToConstant: Metrics.railMarkerSize),
                marker.centerXAnchor.constraint(equalTo: slot.centerXAnchor),
                marker.centerYAnchor.constraint(equalTo: slot.centerYAnchor),
            ])
            slot.isAccessibilityElement = true
            slot.accessibilityLabel = group.label
            slot.accessibilityTraits = .button
            rail.addArrangedSubview(slot)
        }
    }

    /// 묶음을 알리는 기호 — 빌트인 키보드가 세우는 것과 같은 갈래의 그림이다.
    private func railSymbol(for group: AnnotationPanelModel.Group) -> String {
        switch group.category {
        case .smileysAndPeople: "face.smiling"
        case .animalsAndNature: "pawprint"
        case .foodAndDrink: "fork.knife"
        case .activities: "soccerball"
        case .travelAndPlaces: "car"
        case .objects: "lightbulb"
        case .symbols: "number"
        case .flags: "flag"
        case .none:
            switch group.group {
            // 갈래가 섞이는 묶음은 자주 쓰는 것 — 순정과 같은 시계
            case .none: "clock"
            // 글자로 그린 얼굴과 문자 기호는 이모지 묶음과 다른 그림으로 갈라 둔다
            case .emoticon: "face.dashed"
            default: "textformat"
            }
        }
    }

    /// 손가락 아래 표식이 가리키는 묶음으로 옮긴다 — 누르는 순간부터 끌 때까지 이어진다.
    @objc private func dragRail(_ recognizer: UILongPressGestureRecognizer) {
        guard !model.groups.isEmpty, rail.bounds.width > 0 else { return }
        switch recognizer.state {
        case .began, .changed:
            let x = recognizer.location(in: rail).x
            let slot = Int(x / (rail.bounds.width / CGFloat(model.groups.count)))
            scroll(to: min(max(slot, 0), model.groups.count - 1))
        default:
            break
        }
    }

    /// 왼쪽 가장자리에 걸린 항목이 속한 묶음을 레일에 알린다 — 지금 어디를 보고 있는지가
    /// 표식 하나로 드러난다.
    private func updateRailPosition() {
        let probe = CGPoint(
            x: collectionView.contentOffset.x + 4,
            y: collectionView.contentOffset.y + Metrics.itemHeight / 2
        )
        guard let indexPath = collectionView.indexPathForItem(at: probe) else { return }
        highlightRail(section: indexPath.section)
    }

    /// 지금 보고 있는 묶음의 표식에만 바탕을 깔아 자리를 알린다(순정 관례).
    private func highlightRail(section: Int) {
        for (index, slot) in rail.arrangedSubviews.enumerated() {
            guard let marker = slot.subviews.first else { continue }
            let current = index == section
            marker.backgroundColor = current ? TazaTheme.Color.railMarker : .clear
            marker.tintColor = current
                ? TazaTheme.Color.label
                : TazaTheme.Color.secondaryLabel
        }
    }

    private func scroll(to section: Int) {
        guard model.groups[safe: section]?.items.isEmpty == false,
              scrolledSection != section
        else {
            return
        }
        scrolledSection = section
        collectionView.scrollToItem(
            at: IndexPath(item: 0, section: section),
            at: .left,
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

}

extension AnnotationPanelView: UICollectionViewDelegate {
    public func scrollViewDidScroll(_ scrollView: UIScrollView) {
        updateRailPosition()
    }

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
