import UIKit

/// 무분기 셸 — 코어가 내려준 KeyboardFrame을 그리고, 터치 좌표를 코어에 넘기고,
/// 반환된 Effect를 textDocumentProxy로 번역만 한다. 입력 의미론 판단은 하지 않는다.
/// 언어 전환은 플랫폼 관례 소관(화이트리스트)이라 셸이 세션을 교체한다.
final class KeyboardViewController: UIInputViewController {
    private static let candidateBarHeight: CGFloat = 40
    private static let keyRowHeight: CGFloat = 45

    private var englishSession: KeyboardSession?
    private var koreanSession: KeyboardSession?
    private var koreanActive = false

    private var activeSession: KeyboardSession? {
        koreanActive ? koreanSession : englishSession
    }

    private var canvas: KeyboardCanvasView?
    private var candidateBar: UIStackView?
    private var languageToggleButton: UIButton?

    /// 순정 한국어 키보드 관행(iOS 안전 모드): marked text를 쓰지 않고 composing을
    /// 일반 텍스트로 내보낸 뒤 diff로 갱신한다 — 밑줄이 없고, marked text를 제대로
    /// 다루지 못하는 앱에서도 동작이 같다. 화면에 나가 있는 composing을 추적한다.
    private var composingOnScreen = ""
    private var applyingEffects = false

    override func viewDidLoad() {
        super.viewDidLoad()

        englishSession = makeSession(language: .english, packName: "english")
        koreanSession = makeSession(language: .korean, packName: "korean")

        let keyboardHeight = Self.candidateBarHeight + Self.keyRowHeight * 4
        let heightConstraint = view.heightAnchor.constraint(equalToConstant: keyboardHeight)
        heightConstraint.priority = .init(999)
        heightConstraint.isActive = true

        buildViews()
        refreshFrame()
    }

    private func makeSession(language: FfiLanguage, packName: String) -> KeyboardSession {
        let session = KeyboardSession(language: language)
        if let packPath = Bundle(for: Self.self).path(forResource: packName, ofType: "tazapack") {
            try? session.loadPack(path: packPath)
        }
        return session
    }

    /// 플랫폼 inputmode(keyboardType·isSecureTextEntry)를 코어 FieldKind로 매핑
    private func currentFieldKind() -> FfiFieldKind {
        if textDocumentProxy.isSecureTextEntry == true {
            return .password
        }
        switch textDocumentProxy.keyboardType {
        case .emailAddress: return .email
        case .URL, .webSearch: return .url
        case .numberPad, .numbersAndPunctuation, .decimalPad, .asciiCapableNumberPad:
            return .number
        case .phonePad, .namePhonePad: return .phone
        default: return .text
        }
    }

    private func currentContext() -> FfiEditorContext {
        FfiEditorContext(
            textBeforeCursor: textDocumentProxy.documentContextBeforeInput,
            incognito: textDocumentProxy.isSecureTextEntry == true,
            field: currentFieldKind()
        )
    }

    private func buildViews() {
        let toggle = UIButton(configuration: .plain(), primaryAction: UIAction { [weak self] _ in
            self?.toggleLanguage()
        })
        toggle.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(toggle)
        languageToggleButton = toggle

        let bar = UIStackView()
        bar.axis = .horizontal
        bar.distribution = .fillEqually
        bar.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(bar)
        candidateBar = bar

        let canvas = KeyboardCanvasView(frame: .zero)
        canvas.onPress = { [weak self] x, y in self?.press(x: x, y: y) }
        canvas.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(canvas)
        self.canvas = canvas

        NSLayoutConstraint.activate([
            toggle.topAnchor.constraint(equalTo: view.topAnchor),
            toggle.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            toggle.widthAnchor.constraint(equalToConstant: 56),
            toggle.heightAnchor.constraint(equalToConstant: Self.candidateBarHeight),

            bar.topAnchor.constraint(equalTo: view.topAnchor),
            bar.leadingAnchor.constraint(equalTo: toggle.trailingAnchor),
            bar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            bar.heightAnchor.constraint(equalToConstant: Self.candidateBarHeight),

            canvas.topAnchor.constraint(equalTo: bar.bottomAnchor),
            canvas.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            canvas.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            canvas.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
        updateLanguageToggleTitle()
    }

    private func updateLanguageToggleTitle() {
        var configuration = UIButton.Configuration.plain()
        configuration.title = koreanActive ? "한" : "A"
        languageToggleButton?.configuration = configuration
    }

    private func toggleLanguage() {
        // 전환 전에 진행 중 composing을 현재 언어 규칙으로 확정한다
        if let session = activeSession {
            apply(effects: session.handleEvent(event: .focusLost, context: currentContext()))
        }
        koreanActive.toggle()
        updateLanguageToggleTitle()
        showCandidates([])
        refreshFrame()
    }

    private func refreshFrame() {
        canvas?.frameData = activeSession?.keyboardFrame()
    }

    private func press(x: Float, y: Float) {
        guard let session = activeSession else { return }
        let result = session.pressAt(x: x, y: y, context: currentContext())
        apply(effects: result.effects)
        if result.layoutChanged {
            refreshFrame()
        }
    }

    private func deleteCharacters(_ count: Int) {
        for _ in 0..<count {
            textDocumentProxy.deleteBackward()
        }
    }

    private func apply(effects: [FfiEffect]) {
        applyingEffects = true
        defer { applyingEffects = false }
        for effect in effects {
            switch effect {
            case .commitText(let text):
                // 코어 의미론: 활성 composing 구간을 치환하며 확정
                deleteCharacters(composingOnScreen.count)
                composingOnScreen = ""
                textDocumentProxy.insertText(text)
            case .setComposing(let text, _):
                let common = zip(composingOnScreen, text)
                    .prefix(while: { $0 == $1 })
                    .count
                deleteCharacters(composingOnScreen.count - common)
                textDocumentProxy.insertText(String(text.dropFirst(common)))
                composingOnScreen = text
            case .clearComposing:
                deleteCharacters(composingOnScreen.count)
                composingOnScreen = ""
            case .deleteBackward(let codePoints):
                deleteCharacters(Int(codePoints))
            case .updateCandidates(let candidates):
                showCandidates(candidates)
            }
        }
    }

    override func textDidChange(_ textInput: UITextInput?) {
        super.textDidChange(textInput)
        // textDidChange는 우리 편집 뒤에도 호출된다. 문서 끝이 화면 composing과
        // 일치하면 우리 상태 그대로이므로 무시하고, 어긋났을 때(커서 이동·외부 수정)만
        // 코어 finalize로 동기화한다 — 문맥 재동기화(reconciliation) 규칙의 셸 구현.
        guard !applyingEffects, let session = activeSession else { return }
        if composingOnScreen.isEmpty { return }
        let tail = textDocumentProxy.documentContextBeforeInput ?? ""
        if tail.hasSuffix(composingOnScreen) { return }
        composingOnScreen = ""
        apply(effects: session.handleEvent(event: .cursorMoved, context: currentContext()))
    }

    private func showCandidates(_ candidates: [FfiCandidate]) {
        guard let bar = candidateBar else { return }
        bar.arrangedSubviews.forEach { $0.removeFromSuperview() }
        for (index, candidate) in candidates.prefix(4).enumerated() {
            var configuration = UIButton.Configuration.plain()
            configuration.title = candidate.text
            let button = UIButton(configuration: configuration, primaryAction: UIAction { [weak self] _ in
                guard let self, let session = self.activeSession else { return }
                let effects = session.handleEvent(
                    event: .candidateSelected(index: UInt32(index)),
                    context: self.currentContext()
                )
                self.apply(effects: effects)
            })
            bar.addArrangedSubview(button)
        }
    }
}

/// 코어 KeyboardFrame의 정규화 bounds를 px로 변환해 그리는 캔버스.
final class KeyboardCanvasView: UIView {
    var frameData: FfiKeyboardFrame? {
        didSet { setNeedsDisplay() }
    }
    var onPress: ((Float, Float) -> Void)?
    private var pressedLocation: CGPoint?

    override init(frame: CGRect) {
        super.init(frame: frame)
        isOpaque = false
        backgroundColor = .clear
        contentMode = .redraw
    }

    required init?(coder: NSCoder) {
        fatalError("사용하지 않음")
    }

    override func draw(_ rect: CGRect) {
        guard let frameData else { return }
        for row in frameData.rows {
            for key in row {
                let keyRect = CGRect(
                    x: CGFloat(key.bounds.x) * bounds.width + 3,
                    y: CGFloat(key.bounds.y) * bounds.height + 4,
                    width: CGFloat(key.bounds.width) * bounds.width - 6,
                    height: CGFloat(key.bounds.height) * bounds.height - 8
                )
                let path = UIBezierPath(roundedRect: keyRect, cornerRadius: 6)
                let pressed = pressedLocation.map { keyRect.contains($0) } ?? false
                let fill: UIColor = if pressed {
                    .systemGray2
                } else if key.shiftActive {
                    .systemBlue.withAlphaComponent(0.4)
                } else {
                    .white
                }
                fill.setFill()
                path.fill()
                let attributes: [NSAttributedString.Key: Any] = [
                    .font: UIFont.systemFont(ofSize: 21),
                    .foregroundColor: UIColor.black,
                ]
                let size = key.label.size(withAttributes: attributes)
                key.label.draw(
                    at: CGPoint(
                        x: keyRect.midX - size.width / 2,
                        y: keyRect.midY - size.height / 2
                    ),
                    withAttributes: attributes
                )
            }
        }
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let touch = touches.first else { return }
        let location = touch.location(in: self)
        pressedLocation = location
        setNeedsDisplay()
        onPress?(
            Float(location.x / bounds.width),
            Float(location.y / bounds.height)
        )
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        pressedLocation = nil
        setNeedsDisplay()
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        pressedLocation = nil
        setNeedsDisplay()
    }
}
