import UIKit

/// 후보 바에 서는 항목 하나. 갈래도 어떻게 얻은 후보인지도 코어가 정해 내려 주고
/// (FfiCandidateGroup·FfiCandidateKind) 셸은 그대로 묶어 보이기만 한다.
public struct CandidateModel {
    /// 후보 바의 배치 단위. 낱말은 자리를 나눠 갖고, 곁들이는 것은 제 폭만 차지한다.
    public enum Group: Hashable {
        case word
        case emoji
        case symbol
        case emoticon
    }

    /// 어떻게 얻은 후보인가 — 원문을 지키는 자리와 사전이 고친 자리를 눈으로 가른다.
    public enum Kind: Hashable {
        case typed
        case prediction
        case conversion
        case correction
    }

    public let text: String
    public let kind: Kind
    public let group: Group

    public init(text: String, kind: Kind, group: Group) {
        self.text = text
        self.kind = kind
        self.group = group
    }
}

/// 후보 바. 후보가 없을 때도 자리를 지켜 키보드 높이가 흔들리지 않게 한다
/// (순정도 예측 바 자리를 유지한다).
///
/// 낱말과 곁들이는 것(이모지·기호·얼굴 문자)은 **그룹 단위로 묶여** 한 줄에 인라인으로
/// 늘어선다 — 후보 사이와 그룹 사이 모두 순정 후보 바와 같은 1pt 구분선으로 가른다. 낱말 그룹만 남는
/// 폭을 균등하게 나눠 갖고, 곁들이는 것은 제 글자 폭만 차지한다. 그래야 이모지가 떠도
/// 낱말이 서던 자리가 크게 흔들리지 않는다.
public final class CandidateBarView: UIView {
    public var onSelect: ((Int) -> Void)?

    private let scrollView = UIScrollView()
    private let stack = UIStackView()

    public init() {
        super.init(frame: .zero)
        stack.axis = .horizontal
        // 그룹은 제 폭을 갖고 낱말 그룹만 늘어난다 — 구분선 1pt를 지키려면 fill이어야 한다
        stack.distribution = .fill
        stack.translatesAutoresizingMaskIntoConstraints = false
        // 후보가 자리보다 많으면 가로로 훑는다 — 변환은 한 문절에 표기가 열 몇 개씩 달리고,
        // 순정도 그것을 접지 않고 옆으로 밀어 둔다. 자리에 다 들어가는 예측 후보(셋)에서는
        // 폭이 남지 않으므로 스크롤이 있는지조차 보이지 않는다.
        scrollView.showsHorizontalScrollIndicator = false
        scrollView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(scrollView)
        scrollView.addSubview(stack)
        NSLayoutConstraint.activate([
            scrollView.topAnchor.constraint(equalTo: topAnchor),
            scrollView.bottomAnchor.constraint(equalTo: bottomAnchor),
            scrollView.leadingAnchor.constraint(equalTo: leadingAnchor),
            scrollView.trailingAnchor.constraint(equalTo: trailingAnchor),
            stack.topAnchor.constraint(equalTo: scrollView.contentLayoutGuide.topAnchor),
            stack.bottomAnchor.constraint(equalTo: scrollView.contentLayoutGuide.bottomAnchor),
            stack.leadingAnchor.constraint(equalTo: scrollView.contentLayoutGuide.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: scrollView.contentLayoutGuide.trailingAnchor),
            stack.heightAnchor.constraint(equalTo: scrollView.frameLayoutGuide.heightAnchor),
            // 후보가 적어도 바는 폭을 다 쓴다 — 자리와 구분선이 그대로 보여야 한다
            stack.widthAnchor.constraint(
                greaterThanOrEqualTo: scrollView.frameLayoutGuide.widthAnchor
            ),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("사용하지 않음")
    }

    public func setCandidates(_ candidates: [CandidateModel]) {
        stack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        var wordSlots: [UIView] = []
        for (position, group) in groups(of: candidates).enumerated() {
            if position > 0 {
                stack.addArrangedSubview(makeSeparator())
            }
            let groupStack = UIStackView()
            groupStack.axis = .horizontal
            groupStack.distribution = .fill
            groupStack.spacing = group.kind == .word ? 0 : Metrics.itemSpacing
            // 낱말 자리는 후보가 모자라도 셋을 지키고(순정 예측 바 — 자리와 구분선이
            // 그대로 있어야 후보가 바뀔 때마다 바가 다시 짜이지 않는다), 더 오면 온 만큼
            // 늘어난다. 변환 후보는 셋으로 접을 수 있는 것이 아니다.
            let slots = group.kind == .word
                ? max(Metrics.wordSlots, group.items.count)
                : group.items.count
            for slot in 0..<slots {
                // 후보 사이도 그룹 사이와 같은 선으로 가른다
                if slot > 0, group.kind == .word {
                    groupStack.addArrangedSubview(makeSeparator())
                }
                let view: UIView = group.items[safe: slot]
                    .map { makeButton($0.candidate, index: $0.index) } ?? UIView()
                groupStack.addArrangedSubview(view)
                if group.kind == .word {
                    wordSlots.append(view)
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
        for slot in wordSlots.dropFirst() {
            slot.widthAnchor.constraint(equalTo: wordSlots[0].widthAnchor).isActive = true
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
        // 낱말 자리는 후보가 하나도 없어도 남는다 — 빈 바에도 셋으로 나뉜 자리가 보인다
        if !groups.contains(where: { $0.kind == .word }) {
            groups.insert(Group(kind: .word, items: []), at: 0)
        }
        return groups
    }

    // MARK: - 그리기

    private enum Metrics {
        /// 낱말 후보가 서는 **최소** 자리 수 — 순정 예측 바와 같다. 후보가 더 오면
        /// 그만큼 늘고 넘치는 만큼 가로로 훑는다.
        static let wordSlots = 3
        static let itemSpacing: CGFloat = 8
        static let groupInset: CGFloat = 12
        /// 낱말은 남는 폭을 나눠 갖는 자리라 좌우 여백을 좁게 잡아 글자 쪽에 폭을 넘긴다
        static let wordInset: CGFloat = 6
    }

    private func makeButton(_ candidate: CandidateModel, index: Int) -> UIButton {
        var configuration = UIButton.Configuration.plain()
        configuration.attributedTitle = AttributedString(
            displayText(candidate),
            attributes: AttributeContainer([
                .font: TazaTheme.Typography.candidate(
                    group: candidate.group,
                    kind: candidate.kind
                ),
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
        // 읽어 주는 것은 실제로 들어갈 글자다 — 따옴표는 눈으로 가르려고 두른 것이라
        // 소리로 옮기면 없는 부호를 읽는다
        button.accessibilityLabel = candidate.text
        button.accessibilityHint = hint(for: candidate)
        return button
    }

    /// 화면에 나갈 꼴. 원문 후보는 따옴표로 감싼다 — 순정도 사전이 아는 낱말과 친 그대로를
    /// 이렇게 가른다. 감싸는 것은 보이는 꼴일 뿐이고, 고르면 코어가 쥔 원문이 그대로 들어간다.
    private func displayText(_ candidate: CandidateModel) -> String {
        guard candidate.kind == .typed else { return candidate.text }
        return "\u{201C}\(candidate.text)\u{201D}"
    }

    private func hint(for candidate: CandidateModel) -> String {
        if candidate.kind == .typed {
            return NSLocalizedString("친 대로 두기", comment: "후보 바 접근성 힌트")
        }
        return hint(for: candidate.group)
    }

    private func hint(for group: CandidateModel.Group) -> String {
        switch group {
        case .word: NSLocalizedString("후보 선택", comment: "후보 바 접근성 힌트")
        case .emoji: NSLocalizedString("이모지 선택", comment: "후보 바 접근성 힌트")
        case .symbol: NSLocalizedString("기호 선택", comment: "후보 바 접근성 힌트")
        case .emoticon: NSLocalizedString("얼굴 문자 선택", comment: "후보 바 접근성 힌트")
        }
    }

    /// 후보를 가르는 선 — 순정은 바 높이를 다 쓰지 않고 가운데 절반만 긋는다
    private func makeSeparator() -> UIView {
        let holder = UIView()
        holder.isAccessibilityElement = false
        holder.setContentHuggingPriority(.required, for: .horizontal)
        holder.setContentCompressionResistancePriority(.required, for: .horizontal)
        holder.widthAnchor.constraint(equalToConstant: 1).isActive = true
        let separator = UIView()
        separator.backgroundColor = TazaTheme.Color.separator
        separator.translatesAutoresizingMaskIntoConstraints = false
        holder.addSubview(separator)
        NSLayoutConstraint.activate([
            separator.widthAnchor.constraint(equalToConstant: 1),
            separator.centerXAnchor.constraint(equalTo: holder.centerXAnchor),
            separator.centerYAnchor.constraint(equalTo: holder.centerYAnchor),
            separator.heightAnchor.constraint(equalTo: holder.heightAnchor, multiplier: 0.5),
        ])
        return holder
    }

}

private extension Array {
    subscript(safe index: Int) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}
