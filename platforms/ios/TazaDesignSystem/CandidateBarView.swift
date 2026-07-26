import UIKit

/// 후보 바에 서는 항목 하나. 갈래는 코어가 정해 내려 주고(FfiCandidateGroup) 셸은 그
/// 갈래대로 묶어 보이기만 한다.
public struct CandidateModel {
    /// 후보 바의 배치 단위. 낱말은 자리를 나눠 갖고, 곁들이는 것은 제 폭만 차지한다.
    public enum Group: Hashable {
        case word
        case emoji
        case symbol
        case emoticon
    }

    public let text: String
    public let group: Group

    public init(text: String, group: Group) {
        self.text = text
        self.group = group
    }
}

/// 후보 바. 후보가 없을 때도 자리를 지켜 키보드 높이가 흔들리지 않게 한다
/// (순정도 예측 바 자리를 유지한다).
///
/// 낱말과 곁들이는 것(이모지·기호·얼굴 문자)은 **그룹 단위로 묶여** 한 줄에 인라인으로
/// 늘어선다 — 그룹 사이는 순정 후보 바와 같은 1pt 구분선으로 가른다. 낱말 그룹만 남는
/// 폭을 균등하게 나눠 갖고, 곁들이는 것은 제 글자 폭만 차지한다. 그래야 이모지가 떠도
/// 낱말이 서던 자리가 크게 흔들리지 않는다.
public final class CandidateBarView: UIView {
    public var onSelect: ((Int) -> Void)?

    private let stack = UIStackView()

    public init() {
        super.init(frame: .zero)
        stack.axis = .horizontal
        // 그룹은 제 폭을 갖고 낱말 그룹만 늘어난다 — 구분선 1pt를 지키려면 fill이어야 한다
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

    public func setCandidates(_ candidates: [CandidateModel]) {
        stack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        var wordButtons: [UIButton] = []
        for (position, group) in groups(of: candidates).enumerated() {
            if position > 0 {
                stack.addArrangedSubview(makeSeparator())
            }
            let groupStack = UIStackView()
            groupStack.axis = .horizontal
            groupStack.distribution = .fill
            groupStack.spacing = group.kind == .word ? 0 : Metrics.itemSpacing
            for item in group.items {
                let button = makeButton(item.candidate, index: item.index)
                groupStack.addArrangedSubview(button)
                if group.kind == .word {
                    wordButtons.append(button)
                }
            }
            if group.kind != .word {
                groupStack.isLayoutMarginsRelativeArrangement = true
                groupStack.directionalLayoutMargins = NSDirectionalEdgeInsets(
                    top: 0,
                    leading: Metrics.groupInset,
                    bottom: 0,
                    trailing: Metrics.groupInset
                )
                // 곁들이는 것은 제 폭만 — 남는 폭은 낱말 그룹이 갖는다
                groupStack.setContentHuggingPriority(.required, for: .horizontal)
                groupStack.setContentCompressionResistancePriority(.required, for: .horizontal)
            }
            stack.addArrangedSubview(groupStack)
        }
        for button in wordButtons.dropFirst() {
            button.widthAnchor.constraint(equalTo: wordButtons[0].widthAnchor).isActive = true
        }
    }

    // MARK: - 그룹으로 묶기

    private struct Group {
        let kind: CandidateModel.Group
        let items: [(index: Int, candidate: CandidateModel)]
    }

    /// 코어가 내려 준 순서를 지키며 이어지는 같은 갈래끼리 묶는다 — 순서가 곧 갈래 순서라
    /// 셸이 다시 정렬할 일이 없다. 후보 선택은 원래 자리(index)로 코어에 돌아간다.
    private func groups(of candidates: [CandidateModel]) -> [Group] {
        var groups: [Group] = []
        for (index, candidate) in candidates.enumerated() {
            if let last = groups.last, last.kind == candidate.group {
                groups[groups.count - 1] = Group(
                    kind: last.kind,
                    items: last.items + [(index, candidate)]
                )
            } else {
                groups.append(Group(kind: candidate.group, items: [(index, candidate)]))
            }
        }
        return groups
    }

    // MARK: - 그리기

    private enum Metrics {
        static let itemSpacing: CGFloat = 8
        static let groupInset: CGFloat = 12
        /// 낱말은 남는 폭을 나눠 갖는 자리라 좌우 여백을 좁게 잡아 글자 쪽에 폭을 넘긴다
        static let wordInset: CGFloat = 6
    }

    private func makeButton(_ candidate: CandidateModel, index: Int) -> UIButton {
        var configuration = UIButton.Configuration.plain()
        configuration.attributedTitle = AttributedString(
            candidate.text,
            attributes: AttributeContainer([
                .font: TazaTheme.Typography.candidate(group: candidate.group),
                .foregroundColor: TazaTheme.Color.label,
            ])
        )
        // 후보 바는 한 줄이다 — 폭이 모자라면 줄을 늘리지 말고 꼬리를 자른다
        configuration.titleLineBreakMode = .byTruncatingTail
        let inset = candidate.group == .word ? Metrics.wordInset : 0
        configuration.contentInsets = NSDirectionalEdgeInsets(
            top: 0,
            leading: inset,
            bottom: 0,
            trailing: inset
        )
        let button = UIButton(
            configuration: configuration,
            primaryAction: UIAction { [weak self] _ in self?.onSelect?(index) }
        )
        if candidate.group == .word {
            // 바 전체가 넘칠 땐 낱말이 먼저 줄어든다 — 곁들이는 것은 제 폭을 지킨다
            button.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        }
        button.accessibilityLabel = candidate.text
        button.accessibilityHint = hint(for: candidate.group)
        return button
    }

    private func hint(for group: CandidateModel.Group) -> String {
        switch group {
        case .word: "후보 선택"
        case .emoji: "이모지 선택"
        case .symbol: "기호 선택"
        case .emoticon: "얼굴 문자 선택"
        }
    }

    private func makeSeparator() -> UIView {
        let separator = UIView()
        separator.backgroundColor = TazaTheme.Color.separator
        separator.isAccessibilityElement = false
        separator.setContentHuggingPriority(.required, for: .horizontal)
        separator.widthAnchor.constraint(equalToConstant: 1).isActive = true
        // 늘어나는 그룹 사이에서 구분선만 폭을 고정한다
        separator.setContentCompressionResistancePriority(.required, for: .horizontal)
        return separator
    }
}
