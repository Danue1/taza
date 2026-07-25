import UIKit

/// 무분기 셸 — 코어가 내려준 KeyboardFrame을 그리고, 터치 좌표를 코어에 넘기고,
/// 반환된 Effect를 textDocumentProxy로 번역만 한다. 입력 의미론 판단은 하지 않는다.
///
/// 셸이 갖는 분기는 플랫폼 관습 화이트리스트뿐이다: 언어 목록·순서(서드파티 키보드는
/// 자체 관리), 키 역할로만 갈리는 길게 누르기, 딥 링크로 설정 앱 열기.
final class KeyboardViewController: UIInputViewController {
    private let preferences = LanguagePreferences()
    private let typing = TypingPreferences()
    private let learning = LearningStore()
    private var sessions: [TazaLanguage: KeyboardSession] = [:]
    private var currentLanguage: TazaLanguage = TazaLanguage.all[0]

    private var metrics = TazaTheme.Metrics.placeholder
    /// 코어에 이미 알린 표시 환경 — 같은 값을 다시 밀어 넣어 배치가 도는 것을 막는다
    private var appliedFormFactor: FfiFormFactor?
    private var appliedWidth: CGFloat = 0
    private var gridView: KeyboardGridView!
    private var candidateBar: CandidateBarView!
    private var heightConstraint: NSLayoutConstraint!
    private var candidateBarHeightConstraint: NSLayoutConstraint!

    private var languageMenu: PopupMenuView?
    private var alternatesPopup: AlternatesPopupView?
    /// 스페이스·언어 키는 길게 누르기와 의미가 겹치므로 손을 뗄 때 확정한다(순정 관례)
    private var deferredPressPoint: CGPoint?

    /// 순정 한국어 키보드 관행(iOS 안전 모드): marked text를 쓰지 않고 composing을
    /// 일반 텍스트로 내보낸 뒤 diff로 갱신한다 — 밑줄이 없고, marked text를 제대로
    /// 다루지 못하는 앱에서도 동작이 같다. 화면에 나가 있는 composing을 추적한다.
    private var composingOnScreen = ""
    private var applyingEffects = false

    private var activeSession: KeyboardSession? {
        sessions[currentLanguage]
    }

    // MARK: - 생명주기

    override func viewDidLoad() {
        super.viewDidLoad()

        for language in preferences.enabledLanguages {
            sessions[language] = makeSession(language: language)
        }
        // 다른 키보드를 거쳐 돌아와도(익스텐션이 죽었다 살아나도) 마지막 언어로 시작한다
        currentLanguage = preferences.lastUsedLanguage
        if sessions[currentLanguage] == nil {
            currentLanguage = sessions.keys.first ?? TazaLanguage.all[0]
        }

        buildViews()
        refreshFrame()
    }

    /// 설정 앱에서 바뀐 값은 다음 표시 때 반영된다 — 익스텐션이 살아 있는 채로
    /// 설정을 다녀오는 경우까지 덮는다. 학습도 같은 이유로 매번 저장소를 따른다:
    /// 앱에서 재설정을 눌렀으면 스냅샷이 비어 있고, 그때는 코어의 학습도 비운다.
    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        let core = typing.core
        for (language, session) in sessions {
            session.setPreferences(preferences: core)
            let snapshot = learning.snapshot(for: language)
            if snapshot.isEmpty {
                session.resetPersonalization()
            } else {
                session.restorePersonalization(lines: snapshot)
            }
        }
    }

    /// 익스텐션은 키보드가 내려갈 때마다 사라질 수 있으므로 배운 것을 여기서 남긴다.
    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        for (language, session) in sessions {
            learning.save(session.personalizationSnapshot(), for: language)
        }
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        updateMetrics()
    }

    /// 코어 빌드에 포함되지 않은 언어는 nil — 해당 언어는 전환 대상에서 빠진다.
    private func makeSession(language: TazaLanguage) -> KeyboardSession? {
        guard let session = try? KeyboardSession(languageTag: language.tag) else {
            return nil
        }
        // 내려받아 설치된 팩이 있으면 그쪽을, 없으면 내장 팩을 mmap한다.
        // 사전이 아직 없는 언어는 팩 없이도 동작한다(제안·자동교정만 비활성).
        let store = PackStore(bundle: Bundle(for: Self.self))
        if let packURL = store.packURL(for: language) {
            try? session.loadPack(path: packURL.path)
        }
        return session
    }

    // MARK: - 화면 구성

    private func buildViews() {
        view.backgroundColor = TazaTheme.Color.keyboardBackground

        let bar = CandidateBarView()
        bar.onSelect = { [weak self] index in self?.selectCandidate(index) }
        bar.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(bar)
        candidateBar = bar

        let grid = KeyboardGridView(metrics: metrics)
        grid.onPress = { [weak self] point in self?.pressBegan(at: point) }
        grid.onTouchEnded = { [weak self] _ in self?.touchEnded() }
        grid.onLongPressBegan = { [weak self] point in self?.longPressBegan(at: point) }
        grid.onLongPressChanged = { [weak self] point in self?.longPressChanged(at: point) }
        grid.onLongPressEnded = { [weak self] _ in self?.longPressEnded() }
        grid.onAccessibilityActivate = { [weak self] point in self?.activateKey(at: point) }
        grid.onSelectAlternate = { [weak self] _, alternate in
            self?.commitAlternate(alternate, replacingTypedCharacter: false)
        }
        grid.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(grid)
        gridView = grid

        heightConstraint = view.heightAnchor.constraint(equalToConstant: metrics.totalHeight)
        heightConstraint.priority = .init(999)
        heightConstraint.isActive = true
        candidateBarHeightConstraint = bar.heightAnchor.constraint(
            equalToConstant: metrics.candidateBarHeight
        )

        NSLayoutConstraint.activate([
            bar.topAnchor.constraint(equalTo: view.topAnchor),
            bar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            bar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            candidateBarHeightConstraint,

            grid.topAnchor.constraint(equalTo: bar.bottomAnchor),
            grid.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            grid.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            grid.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
    }

    /// 플랫폼 trait을 코어 폼팩터로 옮긴다 — 번역일 뿐이고, 이 갈래로 어떤 치수를
    /// 쓸지는 코어가 정한다.
    private func formFactor() -> FfiFormFactor {
        if traitCollection.horizontalSizeClass == .regular,
           traitCollection.verticalSizeClass == .regular
        {
            return .tablet
        }
        return traitCollection.verticalSizeClass == .compact ? .phoneLandscape : .phonePortrait
    }

    /// 표시 환경이 바뀌면 코어에 알리고, 코어가 새로 정한 치수로 다시 그린다.
    private func updateMetrics() {
        let width = view.bounds.width
        let factor = formFactor()
        guard width > 0, width != appliedWidth || factor != appliedFormFactor else { return }
        appliedWidth = width
        appliedFormFactor = factor
        // 전환 대상 언어들도 같은 화면에 그려지므로 세션 전체에 알린다
        for session in sessions.values {
            session.setMetrics(formFactor: factor, widthPoints: Float(width))
        }
        refreshFrame()
    }

    private func refreshFrame() {
        guard let frame = activeSession?.keyboardFrame() else { return }
        metrics = TazaTheme.Metrics(
            candidateBarHeight: CGFloat(frame.metrics.candidateBarHeight),
            gridHeight: CGFloat(frame.metrics.gridHeight),
            letterFontSize: CGFloat(frame.metrics.letterFontSize),
            controlFontSize: CGFloat(frame.metrics.controlFontSize)
        )
        candidateBarHeightConstraint.constant = metrics.candidateBarHeight
        heightConstraint.constant = metrics.totalHeight
        gridView.update(metrics: metrics)
        gridView.setFrame(model(from: frame))
    }

    private func model(from frame: FfiKeyboardFrame) -> KeyboardFrameModel {
        KeyboardFrameModel(
            rows: frame.rows.map { row in
                row.map { key in
                    KeyModel(
                        label: key.label,
                        appearance: appearance(for: key.role),
                        bounds: CGRect(
                            x: CGFloat(key.bounds.x),
                            y: CGFloat(key.bounds.y),
                            width: CGFloat(key.bounds.width),
                            height: CGFloat(key.bounds.height)
                        ),
                        accessibilityLabel: key.accessibilityLabel,
                        accessibilityValue: key.role == .languageSwitch
                            ? activeSession?.language().displayName
                            : nil,
                        accessibilityHint: hint(for: key),
                        alternates: key.alternates,
                        isActive: key.shiftActive
                    )
                }
            }
        )
    }

    private func appearance(for role: FfiKeyRole) -> KeyCapView.Appearance {
        switch role {
        case .character: .letter
        case .languageSwitch: .language
        case .space: .space
        default: .control
        }
    }

    private func hint(for key: FfiFrameKey) -> String? {
        switch key.role {
        case .languageSwitch: "길게 눌러 언어와 설정 선택"
        case .space: "길게 눌러 커서 이동"
        case .character: key.alternates.isEmpty ? nil : "길게 눌러 변형 문자 선택"
        default: nil
        }
    }

    // MARK: - 입력 문맥

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

    // MARK: - 터치

    private func pressBegan(at point: CGPoint) {
        if languageMenu != nil {
            dismissLanguageMenu()
            return
        }
        guard let session = activeSession else { return }
        let key = session.keyAt(x: Float(point.x), y: Float(point.y))
        if key.role == .space || key.role == .languageSwitch {
            deferredPressPoint = point
            return
        }
        activateKey(at: point)
    }

    private func touchEnded() {
        guard let deferred = deferredPressPoint else { return }
        deferredPressPoint = nil
        activateKey(at: deferred)
    }

    private func activateKey(at point: CGPoint) {
        guard let session = activeSession else { return }
        let result = session.pressAt(
            x: Float(point.x),
            y: Float(point.y),
            context: currentContext()
        )
        apply(effects: result.effects)
        if result.requestsNextLanguage {
            switchLanguage(to: preferences.language(after: currentLanguage))
        }
        if result.layoutChanged {
            refreshFrame()
        }
    }

    private func longPressBegan(at point: CGPoint) {
        deferredPressPoint = nil
        guard let session = activeSession else { return }
        let key = session.keyAt(x: Float(point.x), y: Float(point.y))
        switch key.role {
        case .languageSwitch:
            showLanguageMenu(near: point)
        case .space:
            session.beginCursorDrag(x: Float(point.x))
        case .character where !key.alternates.isEmpty:
            showAlternatesPopup(options: key.alternates, near: point)
        default:
            break
        }
    }

    private func longPressChanged(at point: CGPoint) {
        if let popup = alternatesPopup {
            let pointInPopup = view.convert(
                CGPoint(x: point.x * gridView.bounds.width, y: 0),
                from: gridView
            )
            popup.updateHighlight(atX: pointInPopup.x - popup.frame.minX)
            return
        }
        if languageMenu == nil, let session = activeSession {
            apply(effects: session.updateCursorDrag(
                x: Float(point.x),
                context: currentContext()
            ))
        }
    }

    private func longPressEnded() {
        if let popup = alternatesPopup {
            let alternate = popup.highlightedOption
            dismissAlternatesPopup()
            commitAlternate(alternate, replacingTypedCharacter: true)
            return
        }
        if languageMenu == nil {
            activeSession?.endCursorDrag()
        }
    }

    // MARK: - 변형 문자 팝업

    private func showAlternatesPopup(options: [String], near point: CGPoint) {
        guard let keyFrame = gridView.keyFrame(at: point) else { return }
        let popup = AlternatesPopupView(options: options, metrics: metrics)
        let size = popup.preferredSize(keySize: keyFrame.size)
        let originX = min(
            max(keyFrame.midX - size.width / 2, 4),
            max(view.bounds.width - size.width - 4, 4)
        )
        popup.frame = CGRect(
            x: originX,
            y: gridView.frame.minY + keyFrame.minY - size.height - 4,
            width: size.width,
            height: size.height
        )
        popup.layoutIfNeeded()
        popup.updateHighlight(atX: keyFrame.midX - originX)
        view.addSubview(popup)
        alternatesPopup = popup
    }

    private func dismissAlternatesPopup() {
        alternatesPopup?.removeFromSuperview()
        alternatesPopup = nil
    }

    /// 변형 문자는 이미 들어간 글자를 대체한다 — 순정도 누르는 즉시 기본 글자가 들어가고
    /// 팝업에서 고르면 그 자리를 바꾼다. 지우는 일도 코어 경로(Backspace)를 통과시킨다.
    private func commitAlternate(_ alternate: String, replacingTypedCharacter: Bool) {
        guard let session = activeSession else { return }
        if replacingTypedCharacter {
            apply(effects: session.handleEvent(event: .backspace, context: currentContext()))
        }
        apply(effects: session.selectAlternate(alternate: alternate, context: currentContext()))
    }

    // MARK: - 언어 메뉴

    private func showLanguageMenu(near point: CGPoint) {
        let languages = preferences.enabledLanguages.filter { sessions[$0] != nil }
        var items = [
            PopupMenuView.Item(
                title: "설정",
                detail: "Taza 키보드 앱 열기",
                accessibilityHint: "키보드 설정 앱을 엽니다"
            )
        ]
        // 표시 이름·배열 이름은 각 언어의 코어 세션이 팩 선언에서 읽어 알려 준다
        items += languages.map { language in
            let descriptor = sessions[language]?.language()
            return PopupMenuView.Item(
                title: descriptor?.displayName ?? language.tag,
                detail: descriptor?.layoutName ?? "",
                isSelected: language == currentLanguage
            )
        }

        let menu = PopupMenuView(items: items)
        menu.onSelect = { [weak self] index in
            guard let self else { return }
            dismissLanguageMenu()
            if index == 0 {
                openSettingsApplication()
            } else {
                switchLanguage(to: languages[index - 1])
            }
        }
        menu.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(menu)

        let anchorX = point.x * view.bounds.width
        let leading = min(
            max(anchorX - TazaTheme.Popup.width / 2, 6),
            max(view.bounds.width - TazaTheme.Popup.width - 6, 6)
        )
        // 메뉴 하단을 눌린 키의 윗변에 맞춘다 — 셸이 행 높이를 따로 알 필요가 없다
        let keyTop = gridView.frame.minY + (gridView.keyFrame(at: point)?.minY ?? 0)
        NSLayoutConstraint.activate([
            menu.bottomAnchor.constraint(
                equalTo: view.bottomAnchor,
                constant: keyTop - view.bounds.height
            ),
            menu.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: leading),
        ])
        languageMenu = menu
        UIAccessibility.post(notification: .layoutChanged, argument: menu)
    }

    private func dismissLanguageMenu() {
        languageMenu?.removeFromSuperview()
        languageMenu = nil
    }

    private func switchLanguage(to language: TazaLanguage) {
        guard language != currentLanguage, sessions[language] != nil else { return }
        // 전환 전에 진행 중 composing을 현재 언어 규칙으로 확정한다
        if let session = activeSession {
            apply(effects: session.handleEvent(event: .focusLost, context: currentContext()))
        }
        currentLanguage = language
        preferences.lastUsedLanguage = language
        candidateBar.setCandidates([])
        refreshFrame()
    }

    /// 익스텐션은 UIApplication에 직접 닿을 수 없어 응답자 사슬을 타고 open을 부른다.
    private func openSettingsApplication() {
        var responder: UIResponder? = self
        while let current = responder {
            if let application = current as? UIApplication {
                application.open(TazaDeepLink.settings)
                return
            }
            responder = current.next
        }
    }

    // MARK: - Effect 적용

    private func selectCandidate(_ index: Int) {
        guard let session = activeSession else { return }
        apply(effects: session.handleEvent(
            event: .candidateSelected(index: UInt32(index)),
            context: currentContext()
        ))
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
                candidateBar.setCandidates(candidates.map(\.text))
            case .moveCursor(let offset):
                textDocumentProxy.adjustTextPosition(byCharacterOffset: Int(offset))
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
}
