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
    let learning = LearningStore()
    var sessions: [TazaLanguage: KeyboardSession] = [:]
    var currentLanguage: TazaLanguage = TazaLanguage.all[0]

    var metrics = TazaTheme.Metrics.placeholder
    /// 코어에 이미 알린 표시 환경 — 같은 값을 다시 밀어 넣어 배치가 도는 것을 막는다
    var appliedFormFactor: FfiFormFactor?
    var appliedWidth: CGFloat = 0
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
    /// 손을 뗄 때 확정할 자리 — 삭제를 뺀 모든 키가 이 길을 지난다
    var deferredPressPoint: CGPoint?
    /// 백스페이스를 누르고 있는 동안 이어 지우는 틱 — 누를 때 한 번 지운 뒤, 길게 누르기가
    /// 걸리면 이 간격으로 계속 지운다. 오래 누를수록 간격이 줄어 빨라진다(순정 관례).
    var backspaceRepeatTimer: Timer?
    var backspaceRepeatInterval: TimeInterval = 0
    /// shift를 두 번 눌러 고정하는 관례(순정) — 마지막으로 shift를 누른 시각
    var lastShiftPressedAt: Date?

    /// 순정 한국어 키보드 관행(iOS 안전 모드): marked text를 쓰지 않고 composing을
    /// 일반 텍스트로 내보낸 뒤 diff로 갱신한다 — 밑줄이 없고, marked text를 제대로
    /// 다루지 못하는 앱에서도 동작이 같다. 화면에 나가 있는 composing을 추적한다.
    var composingOnScreen = ""
    var applyingEffects = false

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
        refreshFrame()
        updateField()
    }

    /// 설정 앱에서 바뀐 값은 다음 표시 때 반영된다 — 익스텐션이 살아 있는 채로
    /// 설정을 다녀오는 경우까지 덮는다. 학습도 같은 이유로 매번 저장소를 따른다:
    /// 앱에서 재설정을 눌렀으면 스냅샷이 비어 있고, 그때는 코어의 학습도 비운다.
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
        for (language, session) in sessions {
            // 입력 보조는 언어마다 다를 수 있다 — 세션마다 그 언어의 유효값을 넣는다
            session.setPreferences(preferences: typing.core(for: language))
            let snapshot = learning.snapshot(for: language)
            if snapshot.isEmpty {
                session.resetPersonalization()
            } else {
                session.restorePersonalization(lines: snapshot)
            }
        }
        if currentLanguage != languageBeforeSync {
            candidateBar.setCandidates([])
            refreshFrame()
        }
        updateField()
    }

    /// 익스텐션은 키보드가 내려갈 때마다 사라질 수 있으므로 배운 것을 여기서 남긴다.
    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        endBackspaceRepeat()
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
        grid.onPress = { [weak self] point in self?.pressBegan(at: point) }
        grid.onTouchEnded = { [weak self] point in self?.touchEnded(at: point) }
        grid.onLongPressBegan = { [weak self] point in self?.longPressBegan(at: point) }
        grid.onLongPressChanged = { [weak self] point in self?.longPressChanged(at: point) }
        grid.onLongPressEnded = { [weak self] _ in self?.longPressEnded() }
        grid.onAccessibilityActivate = { [weak self] point in self?.activateKey(at: point) }
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
        guard width > 0, width != appliedWidth || factor != appliedFormFactor else { return }
        appliedWidth = width
        appliedFormFactor = factor
        // 전환 대상 언어들도 같은 화면에 그려지므로 세션 전체에 알린다
        for session in sessions.values {
            session.setMetrics(formFactor: factor, widthPoints: Float(width))
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
        gridView.setFrame(model(from: frame))
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
