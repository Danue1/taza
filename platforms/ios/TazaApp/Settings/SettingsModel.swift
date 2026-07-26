import SwiftUI

/// 설정 화면이 보는 값의 주인. 값 자체는 App Group의 UserDefaults에 있고, 이 타입은
/// 화면이 항목마다 @State 사본을 두지 않도록 Binding과 갱신 신호만 만든다.
@MainActor
final class SettingsModel: ObservableObject {
    /// 시스템 키보드 목록에 우리 익스텐션이 들어 있는가 — 아직 켜지 않은 사용자에게만
    /// 설정으로 가는 길을 보여 준다.
    @Published private(set) var isKeyboardEnabled = true

    private let languagePreferences = LanguagePreferences()
    private let typingPreferences = TypingPreferences()
    private let learning = LearningStore()

    /// 표시 이름·키캡 표기·배열 이름은 코어가 팩 선언에서 읽어 준다
    private var descriptors: [String: FfiLanguageDescriptor] = [:]

    init() {
        refreshKeyboardActivation()
        let store = PackStore()
        descriptors = Dictionary(
            uniqueKeysWithValues: TazaLanguage.all.compactMap { language in
                tazaLanguageDescriptor(for: language, packURL: store.packURL(for: language))
                    .map { (language.tag, $0) }
            }
        )
    }

    // MARK: - 키보드 활성 여부

    /// 설정을 다녀오면 값이 달라질 수 있어 앱이 앞으로 나올 때마다 다시 본다.
    /// 목록을 읽을 수 없는 환경에서는 켜져 있다고 보아 안내를 띄우지 않는다.
    func refreshKeyboardActivation() {
        guard let identifier = Self.keyboardExtensionIdentifier,
              let keyboards = UserDefaults.standard.object(forKey: "AppleKeyboards") as? [String]
        else {
            isKeyboardEnabled = true
            return
        }
        isKeyboardEnabled = keyboards.contains(identifier)
    }

    /// 익스텐션 번들 식별자는 앱 번들 안의 appex에서 읽는다 — 프로젝트 설정과 코드가
    /// 같은 문자열을 따로 들고 있지 않게 한다.
    private static let keyboardExtensionIdentifier: String? = {
        guard let plugins = Bundle.main.builtInPlugInsURL,
              let contents = try? FileManager.default.contentsOfDirectory(
                  at: plugins,
                  includingPropertiesForKeys: nil
              ),
              let appex = contents.first(where: { $0.pathExtension == "appex" })
        else {
            return nil
        }
        return Bundle(url: appex)?.bundleIdentifier
    }()

    // MARK: - 언어 목록

    var enabledLanguages: [TazaLanguage] { languagePreferences.enabledLanguages }
    var availableLanguages: [TazaLanguage] { languagePreferences.availableLanguages }
    /// 마지막 한 언어는 지울 수 없다 — 지우면 키보드가 그릴 배열이 남지 않는다
    var canDisableLanguage: Bool { enabledLanguages.count > 1 }

    func displayName(_ language: TazaLanguage) -> String {
        descriptors[language.tag]?.displayName ?? language.tag
    }

    func layoutName(_ language: TazaLanguage) -> String {
        descriptors[language.tag]?.layoutName ?? ""
    }

    func enable(_ language: TazaLanguage) {
        languagePreferences.enable(language)
        objectWillChange.send()
    }

    func disable(_ language: TazaLanguage) {
        guard canDisableLanguage else { return }
        languagePreferences.disable(language)
        // 언어에 매달린 값도 함께 지운다 — 다시 추가하면 공통값에서 시작한다
        typingPreferences.removeValues(for: language)
        objectWillChange.send()
    }

    /// 목록 순서가 곧 언어 키를 탭했을 때의 순환 순서다
    func moveLanguages(from source: IndexSet, to destination: Int) {
        var languages = enabledLanguages
        languages.move(fromOffsets: source, toOffset: destination)
        languagePreferences.enabledLanguages = languages
        objectWillChange.send()
    }

    /// 테스트 칸에 들어갈 때 그 언어로 키보드가 열리게 한다. 어느 키보드를 띄울지는
    /// 시스템(지구본)이 정하므로, 우리 키보드가 열릴 때의 언어만 미리 맞춰 둔다.
    func prepareKeyboard(for language: TazaLanguage) {
        languagePreferences.lastUsedLanguage = language
    }

    // MARK: - 입력 보조

    func commonValue(_ option: TypingOption) -> Binding<Bool> {
        Binding(
            get: { self.typingPreferences.common(option) },
            set: { self.typingPreferences.setCommon(option, $0); self.objectWillChange.send() }
        )
    }

    func choice(_ option: TypingOption, for language: TazaLanguage) -> Binding<TypingChoice> {
        Binding(
            get: {
                switch self.typingPreferences.languageValue(option, for: language) {
                case .none: .common
                case .some(true): .on
                case .some(false): .off
                }
            },
            set: {
                let value: Bool?
                switch $0 {
                case .common: value = nil
                case .on: value = true
                case .off: value = false
                }
                self.typingPreferences.setLanguageValue(option, for: language, value)
                self.objectWillChange.send()
            }
        )
    }

    /// "공통"을 골랐을 때 그 항목이 실제로 어떻게 동작하는지 — 공통값을 글로 병기한다
    func commonValueDescription(_ option: TypingOption) -> String {
        typingPreferences.common(option)
            ? NSLocalizedString("켬", comment: "설정이 켜져 있음")
            : NSLocalizedString("끔", comment: "설정이 꺼져 있음")
    }

    func resetLearning() {
        learning.removeAll()
        objectWillChange.send()
    }
}
