import UIKit

/// 키보드 판. 키는 개별 뷰로 그리되(접근성 때문에) 터치는 판이 통째로 받아
/// 정규화 좌표로 코어에 넘긴다 — 어느 키인지 판정하는 것은 여전히 코어다.
public final class KeyboardGridView: UIView {
    /// 지금 닿아 있는 손가락 하나를 가리키는 표 — 손가락이 떨어질 때까지 값이 유지된다.
    /// 두 손가락이 겹쳐 눌리는 것이 빠른 타이핑의 정상이므로 누름·뗌은 이 표로 짝을 짓는다.
    public typealias TouchIdentifier = ObjectIdentifier

    public var onPress: ((CGPoint, TouchIdentifier) -> Void)?
    public var onLongPressBegan: ((CGPoint) -> Void)?
    public var onLongPressChanged: ((CGPoint) -> Void)?
    public var onLongPressEnded: ((CGPoint) -> Void)?
    public var onTouchEnded: ((CGPoint, TouchIdentifier) -> Void)?
    /// 시스템이 터치를 거둬 갔다 — 대기 중이던 누름을 그대로 두면 손을 뗀 적 없는 키가 남는다
    public var onTouchCancelled: ((TouchIdentifier) -> Void)?
    public var onAccessibilityActivate: ((CGPoint) -> Void)?
    public var onSelectAlternate: ((CGPoint, String) -> Void)?
    /// 좌우로 민 제스처 — 시작 자리(정규화)와 방향을 넘긴다. 어느 키에서 시작했는지는
    /// 코어가 판정하므로 셸은 좌표만 나른다.
    public var onSwipe: ((CGPoint, Bool) -> Void)?

    /// 글자 키를 누르는 동안 그 글자를 키 위에 크게 띄운다(순정 관례).
    public var showsKeyPreview = true
    /// 판을 통째로 줄여 그릴 때의 배율. 간격·모서리·글꼴처럼 pt로 정해진 값에 곱한다 —
    /// 설정 앱의 배열 미리보기가 실제 키보드와 같은 그림을 작게 그리기 위한 것이고,
    /// 키보드 자신은 1.0으로 둔다.
    public var contentScale: CGFloat = 1 {
        didSet {
            guard contentScale != oldValue, let model = currentModel else { return }
            setFrame(model)
        }
    }

    /// 키에 테두리를 그린다. 값이 바뀌면 다음 `setFrame`부터 반영된다.
    public var showsKeyBorders = false {
        didSet {
            guard showsKeyBorders != oldValue, let model = currentModel else { return }
            setFrame(model)
        }
    }

    private var keyViews: [(view: KeyCapView, bounds: CGRect, rowHeight: CGFloat, leadingExtraGap: CGFloat, trailingExtraGap: CGFloat)] = []
    /// 손가락마다 지금 물고 있는 키 — 롤오버(앞 손가락이 떨어지기 전에 다음 손가락이
    /// 닿는 것)에서 두 키가 함께 눌린 것으로 보여야 한다
    private var pressedViews: [TouchIdentifier: KeyCapView] = [:]
    /// 닿아 있는 손가락이 시작한 자리 — 길게 누르기·민 제스처가 어느 손가락의 것인지 가른다
    private var touchOrigins: [TouchIdentifier: CGPoint] = [:]
    /// 길게 누르기가 낚아챈 손가락 — 그 손가락만 손을 떼도 글자가 되지 않는다
    private var longPressTouch: TouchIdentifier?
    /// 빈 자리 키의 화면 프레임 — 검색면 레일처럼 코어가 비워 둔 자리에 얹는 것이 쓴다
    public private(set) var blankKeyFrame: CGRect = .zero
    private var currentModel: KeyboardFrameModel?
    private let previewView = KeyPreviewView()
    /// 민 제스처가 어디서 시작했는지 — 제스처 인식기는 시작 자리를 알려 주지 않고,
    /// 인식되는 시점에는 그 터치가 이미 거둬졌을 수 있어 따로 남긴다
    private var lastTouchOrigin: CGPoint?

    public init() {
        super.init(frame: .zero)
        backgroundColor = .clear
        // 빠르게 치면 앞 손가락이 떨어지기 전에 다음 손가락이 닿는다 — 이 값이 없으면
        // 그 타건은 이벤트조차 오지 않는다
        isMultipleTouchEnabled = true

        previewView.isHidden = true
        addSubview(previewView)

        let longPress = UILongPressGestureRecognizer(
            target: self,
            action: #selector(handleLongPress(_:))
        )
        longPress.minimumPressDuration = 0.35
        longPress.cancelsTouchesInView = false
        addGestureRecognizer(longPress)

        for direction in [UISwipeGestureRecognizer.Direction.left, .right] {
            let swipe = UISwipeGestureRecognizer(target: self, action: #selector(handleSwipe(_:)))
            swipe.direction = direction
            // 민 것을 실제로 쓰는 자리(스페이스바)인지는 코어 판정이 필요하므로 터치를
            // 여기서 거두지 않는다 — 글자 키 위에서 손가락이 미끄러진 것까지 삼키면
            // 그 타건이 사라진다. 무를 것은 판정한 쪽이 무른다.
            swipe.cancelsTouchesInView = false
            swipe.require(toFail: longPress)
            addGestureRecognizer(swipe)
        }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("사용하지 않음")
    }

    public func setFrame(_ model: KeyboardFrameModel) {
        currentModel = model
        detachPressedViews()
        keyViews.forEach { $0.view.removeFromSuperview() }
        keyViews = []
        for row in model.rows {
            // 두 행을 잇는 키가 섞여 있어도 한 행의 높이는 이웃 키가 알려 준다 —
            // 세로 간격을 그 높이로 잡아야 이어진 키의 모서리가 이웃과 나란해진다
            let rowHeight = row.map(\.bounds.height).min() ?? 0
            for key in row {
                let view = KeyCapView(
                    label: key.label,
                    appearance: key.appearance,
                    fontSize: key.fontSize * contentScale,
                    cornerRadius: TazaTheme.Key.cornerRadius * contentScale,
                    accessibilityLabel: key.accessibilityLabel,
                    accessibilityHint: key.accessibilityHint,
                    accessibilityValue: key.accessibilityValue,
                    alternates: key.alternates,
                    showsBorder: showsKeyBorders
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
                keyViews.append((view, key.bounds, rowHeight, key.leadingExtraGap, key.trailingExtraGap))
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
        let horizontalGap = TazaTheme.Key.horizontalGap * contentScale
        for (view, normalized, unitHeight, declaredLeadingGap, declaredTrailingGap) in keyViews {
            let leadingExtraGap = declaredLeadingGap * contentScale
            let trailingExtraGap = declaredTrailingGap * contentScale
            // 세로 간격은 한 행의 높이를 따라간다 — 행 높이가 폼팩터·배열마다 달라도
            // 키가 차지하는 비율은 순정과 같게 유지되고, 두 행을 잇는 키(천지인의 큰
            // 엔터)도 이웃과 같은 간격으로 서서 위아래 모서리가 나란해진다
            let keyHeight = normalized.height * bounds.height
            let verticalGap = unitHeight * bounds.height * TazaTheme.Key.verticalGapRatio
            view.frame = CGRect(
                x: normalized.minX * bounds.width + horizontalGap / 2 + leadingExtraGap,
                y: normalized.minY * bounds.height + verticalGap / 2,
                width: normalized.width * bounds.width - horizontalGap - leadingExtraGap - trailingExtraGap,
                height: keyHeight - verticalGap
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
        for touch in touches {
            let identifier = TouchIdentifier(touch)
            let point = touch.location(in: self)
            touchOrigins[identifier] = point
            lastTouchOrigin = point
            highlight(at: point, for: identifier)
            onPress?(normalizedPoint(point), identifier)
        }
    }

    public override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches {
            let identifier = TouchIdentifier(touch)
            let point = touch.location(in: self)
            release(identifier)
            // 길게 눌러 팝업을 연 손가락은 손을 떼도 글자가 되지 않는다. 그동안 다른
            // 손가락이 친 키까지 잃을 이유는 없으므로 그 하나만 가른다.
            if identifier == longPressTouch {
                continue
            }
            onTouchEnded?(normalizedPoint(point), identifier)
        }
    }

    public override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches {
            let identifier = TouchIdentifier(touch)
            release(identifier)
            onTouchCancelled?(identifier)
        }
    }

    @objc private func handleSwipe(_ recognizer: UISwipeGestureRecognizer) {
        let location = recognizer.location(in: self)
        guard let origin = nearestTouchOrigin(to: location) ?? lastTouchOrigin else { return }
        onSwipe?(normalizedPoint(origin), recognizer.direction == .left)
    }

    @objc private func handleLongPress(_ recognizer: UILongPressGestureRecognizer) {
        let location = recognizer.location(in: self)
        let point = normalizedPoint(location)
        switch recognizer.state {
        case .began:
            longPressTouch = nearestTouch(to: location)
            onLongPressBegan?(point)
        case .changed:
            onLongPressChanged?(point)
        case .ended, .cancelled, .failed:
            longPressTouch.map(release)
            onLongPressEnded?(point)
            longPressTouch = nil
        default:
            break
        }
    }

    /// 제스처 인식기는 어느 손가락의 것인지 알려 주지 않는다 — 시작 자리가 가장 가까운
    /// 손가락을 그 손가락으로 본다.
    private func nearestTouch(to point: CGPoint) -> TouchIdentifier? {
        touchOrigins
            .min { left, right in
                squaredDistance(left.value, point) < squaredDistance(right.value, point)
            }?
            .key
    }

    private func nearestTouchOrigin(to point: CGPoint) -> CGPoint? {
        nearestTouch(to: point).flatMap { touchOrigins[$0] }
    }

    private func squaredDistance(_ left: CGPoint, _ right: CGPoint) -> CGFloat {
        let dx = left.x - right.x
        let dy = left.y - right.y
        return dx * dx + dy * dy
    }

    private func highlight(at point: CGPoint, for touch: TouchIdentifier) {
        let view = keyViews.first { $0.view.frame.contains(point) }?.view
        pressedViews[touch]?.isPressed = false
        pressedViews[touch] = view
        view?.isPressed = true
        showPreview(for: view)
    }

    /// 손가락 하나가 떨어졌다 — 그 손가락이 물고 있던 키만 놓는다.
    private func release(_ touch: TouchIdentifier) {
        touchOrigins.removeValue(forKey: touch)
        pressedViews.removeValue(forKey: touch)?.isPressed = false
        // 아직 눌려 있는 키가 남아 있으면 그쪽을 띄운다
        showPreview(for: pressedViews.values.first)
    }

    /// 판을 새로 지으면 물고 있던 키 뷰는 사라진다 — 그 자취만 놓고, 아직 닿아 있는
    /// 손가락은 그대로 둔다(손을 뗄 때 그 자리의 키가 들어가야 한다).
    private func detachPressedViews() {
        pressedViews.removeAll()
        hidePreview()
    }

    /// 순정처럼 누른 글자만 키 위로 띄운다 — 제어 키에는 띄우지 않는다.
    private func showPreview(for key: KeyCapView?) {
        guard showsKeyPreview, !isSkeleton, let key, let text = key.previewLabel else {
            hidePreview()
            return
        }
        previewView.show(text: text, over: key.frame, in: bounds)
        bringSubviewToFront(previewView)
    }

    private func hidePreview() {
        previewView.isHidden = true
    }
}

/// 누른 글자를 키 위에 크게 띄우는 말풍선. 순정은 손가락에 가린 글자를 이렇게 보여 준다.
private final class KeyPreviewView: UIView {
    private let labelView = UILabel()

    init() {
        super.init(frame: .zero)
        isUserInteractionEnabled = false
        layer.cornerRadius = TazaTheme.KeyPreview.cornerRadius
        layer.cornerCurve = .continuous
        layer.shadowColor = TazaTheme.Key.shadowColor.cgColor
        layer.shadowOffset = CGSize(width: 0, height: 1)
        layer.shadowRadius = 2
        layer.shadowOpacity = 0.5

        labelView.textAlignment = .center
        labelView.adjustsFontSizeToFitWidth = true
        labelView.minimumScaleFactor = 0.5
        labelView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(labelView)
        NSLayoutConstraint.activate([
            labelView.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 4),
            labelView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -4),
            labelView.centerYAnchor.constraint(equalTo: centerYAnchor),
        ])
        updateColors()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("사용하지 않음")
    }

    override func traitCollectionDidChange(_ previous: UITraitCollection?) {
        super.traitCollectionDidChange(previous)
        updateColors()
    }

    private func updateColors() {
        backgroundColor = TazaTheme.Color.keySurface
        labelView.textColor = TazaTheme.Color.label
    }

    /// 키 위에 올리되 판 밖으로 나가지 않게 좌우를 물린다 — 가장자리 키에서도 글자가
    /// 잘리지 않아야 한다.
    func show(text: String, over keyFrame: CGRect, in container: CGRect) {
        labelView.text = text
        labelView.font = TazaTheme.Typography.keyLabel(
            size: keyFrame.height * TazaTheme.KeyPreview.fontScale / 2
        )
        let width = keyFrame.width + TazaTheme.KeyPreview.horizontalOverhang * 2
        let height = TazaTheme.KeyPreview.height
        let x = min(
            max(keyFrame.midX - width / 2, 0),
            max(container.width - width, 0)
        )
        frame = CGRect(
            x: x,
            y: keyFrame.minY - height - TazaTheme.KeyPreview.lift,
            width: width,
            height: height
        )
        // 첫 행에서는 위로 나갈 자리가 없다 — 그때는 띄우지 않는다(순정도 같다)
        isHidden = frame.minY < 0
    }
}
