import UIKit

/// 코어가 준 키 하나를 그리기 위한 표현. FFI 타입을 셸이 이 모델로 옮겨 담아
/// 디자인 시스템이 엔진에 의존하지 않게 한다(설정 앱도 같은 컴포넌트를 쓴다).
public struct KeyModel {
    public let label: String
    public let appearance: KeyCapView.Appearance
    /// 키보드 영역 기준 정규화 좌표
    public let bounds: CGRect
    public let accessibilityLabel: String
    public let accessibilityValue: String?
    public let accessibilityHint: String?
    public let alternates: [String]
    public let isActive: Bool

    public init(
        label: String,
        appearance: KeyCapView.Appearance,
        bounds: CGRect,
        accessibilityLabel: String,
        accessibilityValue: String? = nil,
        accessibilityHint: String? = nil,
        alternates: [String] = [],
        isActive: Bool = false
    ) {
        self.label = label
        self.appearance = appearance
        self.bounds = bounds
        self.accessibilityLabel = accessibilityLabel
        self.accessibilityValue = accessibilityValue
        self.accessibilityHint = accessibilityHint
        self.alternates = alternates
        self.isActive = isActive
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

    private var metrics: TazaTheme.Metrics
    private var keyViews: [(view: KeyCapView, bounds: CGRect)] = []
    private var pressedView: KeyCapView?
    private var longPressActive = false

    public init(metrics: TazaTheme.Metrics) {
        self.metrics = metrics
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

    public func update(metrics: TazaTheme.Metrics) {
        self.metrics = metrics
    }

    public func setFrame(_ model: KeyboardFrameModel) {
        keyViews.forEach { $0.view.removeFromSuperview() }
        keyViews = []
        for row in model.rows {
            for key in row {
                let view = KeyCapView(
                    label: key.label,
                    appearance: key.appearance,
                    fontSize: key.appearance == .letter
                        ? metrics.letterFontSize
                        : metrics.controlFontSize,
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
                addSubview(view)
                keyViews.append((view, key.bounds))
            }
        }
        setNeedsLayout()
    }

    public override func layoutSubviews() {
        super.layoutSubviews()
        let horizontalGap = TazaTheme.Key.horizontalGap
        for (view, normalized) in keyViews {
            // 세로 간격은 키 높이를 따라간다 — 행 높이가 폼팩터·배열마다 달라도
            // 키가 차지하는 비율은 순정과 같게 유지된다
            let rowHeight = normalized.height * bounds.height
            let verticalGap = rowHeight * TazaTheme.Key.verticalGapRatio
            view.frame = CGRect(
                x: normalized.minX * bounds.width + horizontalGap / 2,
                y: normalized.minY * bounds.height + verticalGap / 2,
                width: normalized.width * bounds.width - horizontalGap,
                height: rowHeight - verticalGap
            )
        }
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
