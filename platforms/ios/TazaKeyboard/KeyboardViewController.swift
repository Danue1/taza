import UIKit

/// 무분기 셸 — 코어가 내려준 KeyboardFrame을 그리고, 터치 좌표를 코어에 넘기고,
/// 반환된 Effect를 textDocumentProxy로 번역만 한다. 입력 의미론 판단은 하지 않는다.
///
/// 셸이 갖는 분기는 플랫폼 관습 화이트리스트뿐이다: 언어 목록·순서(서드파티 키보드는
/// 자체 관리), 키 역할로만 갈리는 길게 누르기, 딥 링크로 설정 앱 열기.
///
/// 이 파일은 셸의 뼈대만 갖는다 — 생명주기와 뷰 조립, 그리고 코어가 정한 치수를 제약에
/// 옮기는 일. 나머지는 갈래별 파일의 extension이 나눠 갖는다.
final class KeyboardViewController: UIInputViewController {
    let preferences = LanguagePreferences()
    let typing = TypingPreferences()
    let keyboardPreferences = KeyboardPreferences()
    let learning = LearningStore()
    let shortcuts = ShortcutStore()
    var sessions: [TazaLanguage: KeyboardSession] = [:]
    var currentLanguage: TazaLanguage = TazaLanguage.all[0]

    var metrics = TazaTheme.Metrics.placeholder
    /// 코어에 이미 알린 표시 환경 — 같은 값을 다시 밀어 넣어 배치가 도는 것을 막는다
    var appliedFormFactor: FfiFormFactor?
    var appliedWidth: CGFloat = 0
    /// 코어에 이미 알린 글자 배율 — 시스템 글자 크기를 바꾸면 달라진다
    var appliedTextScale: Float = 0
    /// 코어에 이미 알린 필드 성격 — 같은 값을 다시 밀어 넣어 배치가 도는 것을 막는다
    var appliedField: FfiFieldKind?
    var gridView: KeyboardGridView!
    var candidateBar: CandidateBarView!
    /// 통합 검색면 — 이모지 키로 들어가는 레이어에서만 보인다
    var panelView: AnnotationPanelView!
    var heightConstraint: NSLayoutConstraint!
    var candidateBarHeightConstraint: NSLayoutConstraint!
    var panelHeightConstraint: NSLayoutConstraint!

    var languageMenu: PopupMenuView?
    var alternatesPopup: AlternatesPopupView?
    /// 손을 뗄 때 확정할 자리 — 삭제를 뺀 모든 키가 이 길을 지난다. 빠르게 치면 앞
    /// 손가락이 떨어지기 전에 다음 손가락이 닿으므로 손가락마다 하나씩 둔다.
    var deferredPresses: [KeyboardGridView.TouchIdentifier: CGPoint] = [:]
    /// 백스페이스를 누르고 있는 동안 이어 지우는 틱 — 누를 때 한 번 지운 뒤, 길게 누르기가
    /// 걸리면 이 간격으로 계속 지운다. 오래 누를수록 간격이 줄어 빨라진다(순정 관례).
    var backspaceRepeatTimer: Timer?
    var backspaceRepeatInterval: TimeInterval = 0
    /// shift를 두 번 눌러 고정하는 관례(순정) — 마지막으로 shift를 누른 시각
    var lastShiftPressedAt: Date?
    /// 멀티탭 주기가 끝나기를 기다리는 타이머(천지인). 코어가 시한을 요청할 때마다
    /// 갈아 끼운다 — 이어 누르면 시한이 새로 시작해야 하기 때문이다. 무엇이 몇 번째인지는
    /// 코어가 알고 있고, 셸은 시각만 잰다.
    var multitapTimer: Timer?
    /// 설정 앱이 값을 고쳤다는 신호를 듣는 자리 — 키보드가 떠 있는 동안에도 바뀐다
    var settingsObserver: SettingsBroadcast.Observer?

    /// 화면에 나가 있는 composing. 이것을 어떻게 앉히는지는 **코어가 정한다**
    /// (`composingDisplay`) — 한국어는 순정 관행대로 marked text 없이 일반 텍스트로
    /// 내보낸 뒤 diff로 갱신하고(밑줄이 없고 비협조 앱에서도 같게 동작한다), 일본어 변환은
    /// 길이가 크게 출렁이고 주목 문절을 밝혀야 하므로 marked text를 쓴다.
    var composingOnScreen = ""
    /// 지금 언어가 요구하는 조합 표시 방식. 언어를 바꿀 때 코어에서 다시 읽는다.
    var composingDisplay: FfiComposingDisplay = .inline
    var applyingEffects = false
    /// 우리가 마지막으로 남긴 커서 앞 텍스트. 조합 창이 없는 언어에서도 커서가 옮겨 간
    /// 것을 알아보려면 견줄 것이 있어야 한다 — 아직 아무것도 넣지 않았으면 nil이다.
    var lastAppliedTail: String?

    // MARK: - 생명주기

    override func viewDidLoad() {
        super.viewDidLoad()

        syncSessions()
        // 다른 키보드를 거쳐 돌아와도(익스텐션이 죽었다 살아나도) 마지막 언어로 시작한다
        currentLanguage = preferences.lastUsedLanguage
        if sessions[currentLanguage] == nil {
            currentLanguage = sessions.keys.first ?? TazaLanguage.all[0]
        }

        buildViews()
        applySettings(includingLearning: true)
        updateField()

        // 설정 화면의 테스트 칸은 키보드를 띄운 채로 값을 바꾸는 자리다 — 다음 표시를
        // 기다리지 않고 그 자리에서 반영한다.
        settingsObserver = SettingsBroadcast.Observer { [weak self] kind in
            self?.applySettings(includingLearning: kind == .learning)
        }
    }

    /// 저장소에 있는 값을 세션과 뷰에 옮긴다. 키보드가 뜰 때와 설정이 바뀌었다는 신호를
    /// 받을 때 같은 길을 지난다.
    ///
    /// `includingLearning`은 앱이 학습 스냅샷을 실제로 고쳤을 때만 참이다. 설정 하나를
    /// 바꿀 때마다 스냅샷을 되씌우면, 저장되지 않은 이번 세션의 학습이 되감긴다 —
    /// 스냅샷은 익스텐션도 쓰는 유일한 값이라 그렇다.
    func applySettings(includingLearning: Bool) {
        syncSessions()
        for (language, session) in sessions {
            // 입력 보조는 언어마다 다를 수 있다 — 세션마다 그 언어의 유효값을 넣는다
            session.setPreferences(preferences: typing.core(for: language))
            session.setShortcuts(shortcuts: shortcuts.entries)
            applyLayoutChoice(to: session, for: language)
            guard includingLearning else { continue }
            let snapshot = learning.snapshot(for: language)
            if snapshot.isEmpty {
                session.resetPersonalization()
            } else {
                session.restorePersonalization(lines: snapshot)
            }
        }
        applyShellPreferences()
        refreshFrame()
    }

    /// 코어를 거치지 않는 설정 — 코어에는 판단할 것이 없으므로 계약을 늘리지 않고
    /// 여기서 뷰에 그대로 옮긴다. 값이 바뀌는 시점은 코어 설정과 같다(다음 표시).
    private func applyShellPreferences() {
        gridView.showsKeyPreview = keyboardPreferences.keyPreview
        gridView.showsKeyBorders = keyboardPreferences.keyBorders
        switch keyboardPreferences.theme {
        case .system: view.overrideUserInterfaceStyle = .unspecified
        case .light: view.overrideUserInterfaceStyle = .light
        case .dark: view.overrideUserInterfaceStyle = .dark
        }
    }

    /// 키를 눌렀다는 것을 소리로 알린다.
    ///
    /// 햅틱은 두지 않는다: 익스텐션의 진동은 전체 접근 권한을 받아야 울리는데, 이 키보드는
    /// 기기 밖으로 아무것도 보내지 않는 대가로 그 권한을 요구하지 않기로 했다. 켤 수 없는
    /// 스위치를 설정 화면에 남기느니 항목 자체를 두지 않는다.
    func playKeyFeedback() {
        guard keyboardPreferences.keySound else { return }
        UIDevice.current.playInputClick()
    }

    /// 키보드가 뜰 때마다 저장소를 다시 따른다. 떠 있는 동안의 변경은 알림이 나르지만
    /// (`settingsObserver`), 알림을 놓친 경우와 익스텐션이 죽었다 살아난 경우가 남는다 —
    /// 이 자리가 그 바닥이다.
    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        let languageBeforeSync = currentLanguage
        syncSessions()
        // 설정 앱이 테스트 칸에 들어가며 언어를 정해 두었을 수 있다 — 저장된 마지막
        // 언어를 따라간다. 평소에는 우리가 전환할 때마다 갱신하므로 값이 같다.
        let storedLanguage = preferences.lastUsedLanguage
        if storedLanguage != currentLanguage, sessions[storedLanguage] != nil {
            currentLanguage = storedLanguage
        }
        applySettings(includingLearning: true)
        if currentLanguage != languageBeforeSync {
            candidateBar.setCandidates([])
        }
        updateField()
    }

    /// 익스텐션은 키보드가 내려갈 때마다 사라질 수 있으므로 배운 것을 여기서 남긴다.
    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        endBackspaceRepeat()
        // 키보드가 내려간 뒤 울리는 시한은 끊긴 주기를 다시 끊을 뿐이라 붙잡아 둘 것이 없다
        multitapTimer?.invalidate()
        multitapTimer = nil
        for (language, session) in sessions {
            learning.save(session.personalizationSnapshot(), for: language)
        }
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        updateMetrics()
        // 레일은 코어가 하단 행에 비워 둔 칸에 앉는다 — 자리를 셸이 지어내지 않는다
        panelView.railFrame = panelView.isHidden
            ? .zero
            : panelView.convert(gridView.blankKeyFrame, from: gridView)
    }
}

/// 키 클릭음은 시스템이 내준다 — 이 선언이 없으면 `playInputClick()`이 조용히 무시된다.
extension KeyboardViewController: UIInputViewAudioFeedback {
    var enableInputClicksWhenVisible: Bool { true }

    // MARK: - 화면 구성

    private func buildViews() {
        // 판 바탕은 시스템이 입력 뷰에 깔아 주는 것을 그대로 쓴다 — 순정과 같은 톤이 된다
        view.backgroundColor = .clear

        let bar = CandidateBarView()
        bar.onSelect = { [weak self] index in self?.selectCandidate(index) }
        bar.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(bar)
        candidateBar = bar

        let grid = KeyboardGridView()
        grid.onPress = { [weak self] point, touch in self?.pressBegan(at: point, touch: touch) }
        grid.onTouchEnded = { [weak self] point, touch in
            self?.touchEnded(at: point, touch: touch)
        }
        grid.onTouchCancelled = { [weak self] touch in self?.touchCancelled(touch) }
        grid.onLongPressBegan = { [weak self] point in self?.longPressBegan(at: point) }
        grid.onLongPressChanged = { [weak self] point in self?.longPressChanged(at: point) }
        grid.onLongPressEnded = { [weak self] _ in self?.longPressEnded() }
        grid.onAccessibilityActivate = { [weak self] point in self?.activateKey(at: point) }
        grid.onSwipe = { [weak self] point, toLeft in self?.swiped(at: point, toLeft: toLeft) }
        grid.onSelectAlternate = { [weak self] _, alternate in
            self?.commitAlternate(alternate)
        }
        grid.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(grid)
        gridView = grid

        // 검색면은 키 그리드와 같은 자리를 쓴다 — 코어가 그 레이어의 키를 하단 행만 내려
        // 주므로 패널이 위쪽을 덮어도 키와 겹치지 않는다.
        let panel = AnnotationPanelView(frame: .zero)
        panel.onSelect = { [weak self] group, text in self?.selectAnnotation(group, text) }
        panel.isHidden = true
        panel.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(panel)
        panelView = panel

        heightConstraint = view.heightAnchor.constraint(equalToConstant: metrics.totalHeight)
        heightConstraint.priority = .init(999)
        heightConstraint.isActive = true
        candidateBarHeightConstraint = bar.heightAnchor.constraint(
            equalToConstant: metrics.candidateBarHeight
        )
        panelHeightConstraint = panel.heightAnchor.constraint(equalToConstant: 0)

        NSLayoutConstraint.activate([
            bar.topAnchor.constraint(equalTo: view.topAnchor),
            bar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            bar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            candidateBarHeightConstraint,

            grid.topAnchor.constraint(equalTo: bar.bottomAnchor),
            grid.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            grid.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            grid.bottomAnchor.constraint(equalTo: view.bottomAnchor),

            panel.topAnchor.constraint(equalTo: grid.topAnchor),
            panel.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            panel.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            panelHeightConstraint,
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
        // 시스템 글자 크기 — 본문 글꼴이 얼마나 커졌는지로 잰다. 어디까지 반영할지는
        // 코어가 정한다(라벨이 키를 넘지 않도록 위를 막는다).
        let scale = Float(
            UIFontMetrics.default.scaledValue(
                for: 100,
                compatibleWith: traitCollection
            ) / 100
        )
        guard width > 0,
              width != appliedWidth || factor != appliedFormFactor || scale != appliedTextScale
        else {
            return
        }
        appliedWidth = width
        appliedFormFactor = factor
        appliedTextScale = scale
        // 전환 대상 언어들도 같은 화면에 그려지므로 세션 전체에 알린다
        for session in sessions.values {
            session.setMetrics(formFactor: factor, widthPoints: Float(width), textScale: scale)
        }
        refreshFrame()
    }

    func refreshFrame() {
        guard let frame = activeSession?.keyboardFrame() else { return }
        metrics = TazaTheme.Metrics(
            candidateBarHeight: CGFloat(frame.metrics.candidateBarHeight),
            gridHeight: CGFloat(frame.metrics.gridHeight),
            letterFontSize: CGFloat(frame.metrics.letterFontSize)
        )
        candidateBarHeightConstraint.constant = metrics.candidateBarHeight
        heightConstraint.constant = metrics.totalHeight
        gridView.setFrame(
            keyboardFrameModel(frame, languageDisplayName: activeSession?.language().displayName)
        )
        refreshPanel(frame: frame)
    }

    /// 검색면이 있는 레이어인지는 코어가 프레임에 실어 준다 — 셸은 자리를 잡고 내용을
    /// 받아 그리기만 한다.
    private func refreshPanel(frame: FfiKeyboardFrame) {
        let ratio = CGFloat(frame.panelHeightRatio)
        // 검색면은 하단 키 행까지 덮는다 — 레일이 그 행의 빈 칸에 앉아 문자 복귀·삭제
        // 키와 한 줄을 이루기 때문이다. 키 자리의 터치는 패널이 흘려보낸다.
        panelHeightConstraint.constant = ratio > 0 ? metrics.gridHeight : 0
        panelView.isHidden = ratio <= 0
        guard ratio > 0, let session = activeSession else { return }
        // 검색어 없는 첫 화면 — 자주 쓰는 것과 갈래별 목록
        panelView.setPanel(panelModel(from: session.annotationPanel(query: "")))
        candidateBar.setCandidates([])
    }
}
