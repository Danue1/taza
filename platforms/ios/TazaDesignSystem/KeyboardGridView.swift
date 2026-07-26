import UIKit

/// 코어가 준 키 하나를 그리기 위한 표현. FFI 타입을 셸이 이 모델로 옮겨 담아
/// 디자인 시스템이 엔진에 의존하지 않게 한다(설정 앱도 같은 컴포넌트를 쓴다).
public struct KeyModel {
    public let label: String
    public let appearance: KeyCapView.Appearance
    /// 코어가 정한 이 키 라벨의 글꼴 크기(pt)
    public let fontSize: CGFloat
    /// 키보드 영역 기준 정규화 좌표
    public let bounds: CGRect
    public let accessibilityLabel: String
    public let accessibilityValue: String?
    public let accessibilityHint: String?
    public let alternates: [String]
    public let isActive: Bool
    /// shift 오른쪽·backspace 왼쪽처럼 글자 열과 갈라지는 자리에 더할 여백(pt)
    public let leadingExtraGap: CGFloat
    public let trailingExtraGap: CGFloat

    public init(
        label: String,
        appearance: KeyCapView.Appearance,
        fontSize: CGFloat,
        bounds: CGRect,
        accessibilityLabel: String,
        accessibilityValue: String? = nil,
        accessibilityHint: String? = nil,
        alternates: [String] = [],
        isActive: Bool = false,
        leadingExtraGap: CGFloat = 0,
        trailingExtraGap: CGFloat = 0
    ) {
        self.label = label
        self.appearance = appearance
        self.fontSize = fontSize
        self.bounds = bounds
        self.accessibilityLabel = accessibilityLabel
        self.accessibilityValue = accessibilityValue
        self.accessibilityHint = accessibilityHint
        self.alternates = alternates
        self.isActive = isActive
        self.leadingExtraGap = leadingExtraGap
        self.trailingExtraGap = trailingExtraGap
    }
}

public struct KeyboardFrameModel {
    public let rows: [[KeyModel]]

    public init(rows: [[KeyModel]]) {
        self.rows = rows
    }
}

/// 키보드 판. 키는 개별 뷰로 그리되(접근성 때문에) 터치는 판이 통째로 받아
/// 정규화 좌표로 코어에 넘긴다 — 어느 키인지 판정하는 것은 여전히 코어다.
public final class KeyboardGridView: UIView {
    public var onPress: ((CGPoint) -> Void)?
    public var onLongPressBegan: ((CGPoint) -> Void)?
    public var onLongPressChanged: ((CGPoint) -> Void)?
    public var onLongPressEnded: ((CGPoint) -> Void)?
    public var onTouchEnded: ((CGPoint) -> Void)?
    public var onAccessibilityActivate: ((CGPoint) -> Void)?
    public var onSelectAlternate: ((CGPoint, String) -> Void)?

    private var keyViews: [(view: KeyCapView, bounds: CGRect, leadingExtraGap: CGFloat, trailingExtraGap: CGFloat)] = []
    private var pressedView: KeyCapView?
    /// 빈 자리 키의 화면 프레임 — 검색면 레일처럼 코어가 비워 둔 자리에 얹는 것이 쓴다
    public private(set) var blankKeyFrame: CGRect = .zero
    private var longPressActive = false

    public init() {
        super.init(frame: .zero)
        backgroundColor = .clear

        let longPress = UILongPressGestureRecognizer(
            target: self,
            action: #selector(handleLongPress(_:))
        )
        longPress.minimumPressDuration = 0.35
        longPress.cancelsTouchesInView = false
        addGestureRecognizer(longPress)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("사용하지 않음")
    }

    public func setFrame(_ model: KeyboardFrameModel) {
        keyViews.forEach { $0.view.removeFromSuperview() }
        keyViews = []
        for row in model.rows {
            for key in row {
                let view = KeyCapView(
                    label: key.label,
                    appearance: key.appearance,
                    fontSize: key.fontSize,
                    cornerRadius: TazaTheme.Key.cornerRadius,
                    accessibilityLabel: key.accessibilityLabel,
                    accessibilityHint: key.accessibilityHint,
                    accessibilityValue: key.accessibilityValue,
                    alternates: key.alternates
                )
                view.isActive = key.isActive
                let center = CGPoint(x: key.bounds.midX, y: key.bounds.midY)
                view.onAccessibilityActivate = { [weak self] in
                    self?.onAccessibilityActivate?(center)
                }
                view.onSelectAlternate = { [weak self] alternate in
                    self?.onSelectAlternate?(center, alternate)
                }
                view.isSkeleton = isSkeleton
                addSubview(view)
                keyViews.append((view, key.bounds, key.leadingExtraGap, key.trailingExtraGap))
            }
        }
        setNeedsLayout()
    }

    /// 스페이스바로 커서를 끄는 동안 판 전체를 물린다 — 지금 글자를 받지 않는다는 신호다
    public var isSkeleton: Bool = false {
        didSet {
            UIView.animate(withDuration: 0.28) {
                self.keyViews.forEach { $0.view.isSkeleton = self.isSkeleton }
            }
        }
    }

    public override func layoutSubviews() {
        super.layoutSubviews()
        let horizontalGap = TazaTheme.Key.horizontalGap
        for (view, normalized, leadingExtraGap, trailingExtraGap) in keyViews {
            // 세로 간격은 키 높이를 따라간다 — 행 높이가 폼팩터·배열마다 달라도
            // 키가 차지하는 비율은 순정과 같게 유지된다
            let rowHeight = normalized.height * bounds.height
            let verticalGap = rowHeight * TazaTheme.Key.verticalGapRatio
            view.frame = CGRect(
                x: normalized.minX * bounds.width + horizontalGap / 2 + leadingExtraGap,
                y: normalized.minY * bounds.height + verticalGap / 2,
                width: normalized.width * bounds.width - horizontalGap - leadingExtraGap - trailingExtraGap,
                height: rowHeight - verticalGap
            )
        }
        blankKeyFrame = keyViews
            .first { $0.view.appearance == .blank }
            .map(\.view.frame) ?? .zero
    }

    /// 화면 좌표 → 코어가 쓰는 정규화 좌표
    public func normalizedPoint(_ point: CGPoint) -> CGPoint {
        CGPoint(x: point.x / bounds.width, y: point.y / bounds.height)
    }

    /// 정규화 좌표에 있는 키의 화면 프레임 — 팝업을 키 위에 붙일 때 쓴다
    public func keyFrame(at normalized: CGPoint) -> CGRect? {
        keyViews
            .first { $0.bounds.contains(normalized) }
            .map(\.view.frame)
    }

    public override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let touch = touches.first else { return }
        let point = touch.location(in: self)
        highlight(at: point)
        onPress?(normalizedPoint(point))
    }

    public override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let touch = touches.first else { return }
        clearHighlight()
        if !longPressActive {
            onTouchEnded?(normalizedPoint(touch.location(in: self)))
        }
    }

    public override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        clearHighlight()
    }

    @objc private func handleLongPress(_ recognizer: UILongPressGestureRecognizer) {
        let point = normalizedPoint(recognizer.location(in: self))
        switch recognizer.state {
        case .began:
            longPressActive = true
            onLongPressBegan?(point)
        case .changed:
            onLongPressChanged?(point)
        case .ended, .cancelled, .failed:
            clearHighlight()
            onLongPressEnded?(point)
            longPressActive = false
        default:
            break
        }
    }

    private func highlight(at point: CGPoint) {
        pressedView?.isPressed = false
        pressedView = keyViews.first { $0.view.frame.contains(point) }?.view
        pressedView?.isPressed = true
    }

    private func clearHighlight() {
        pressedView?.isPressed = false
        pressedView = nil
    }
}
