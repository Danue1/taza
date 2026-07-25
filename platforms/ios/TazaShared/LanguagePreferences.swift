import Foundation

/// 셸이 소유하는 언어 목록. 지구본 키는 시스템 키보드끼리의 전환이라, 우리 키보드가
/// 지원하는 언어 사이의 전환·순서·최근 언어는 앱이 직접 관리한다.
public enum TazaLanguage: String, CaseIterable, Sendable {
    case english
    case korean

    /// 언어는 자기 이름으로 표기한다 (순정 관례)
    public var displayName: String {
        switch self {
        case .english: "English"
        case .korean: "한국어"
        }
    }

    /// 언어 키에 찍히는 짧은 표기
    public var keycapLabel: String {
        switch self {
        case .english: "A"
        case .korean: "한"
        }
    }

    public var layoutName: String {
        switch self {
        case .english: "QWERTY"
        case .korean: "두벌식"
        }
    }

    /// 익스텐션 번들에 들어 있는 언어팩 파일 이름
    public var packName: String {
        switch self {
        case .english: "english"
        case .korean: "korean"
        }
    }
}

/// 키보드 익스텐션과 설정 앱이 함께 읽고 쓰는 저장소. 다른 키보드로 갔다 돌아와도
/// (익스텐션 프로세스가 죽었다 살아나도) 마지막에 쓰던 언어로 돌아오게 한다.
public struct LanguagePreferences {
    public static let appGroupIdentifier = "group.io.danuel.taza"

    private enum Key {
        static let enabledLanguages = "enabledLanguages"
        static let lastUsedLanguage = "lastUsedLanguage"
    }

    private let defaults: UserDefaults

    /// App Group을 못 쓰는 상황(설정 누락·시뮬레이터)에서도 키보드가 동작해야 하므로
    /// 표준 저장소로 물러난다 — 이때는 앱과 값을 공유하지 못할 뿐이다.
    public init(defaults: UserDefaults? = nil) {
        self.defaults = defaults
            ?? UserDefaults(suiteName: Self.appGroupIdentifier)
            ?? .standard
    }

    /// 사용 중인 언어 — 순환 순서 그대로. 비어 있으면 전체 언어를 기본값으로 쓴다.
    public var enabledLanguages: [TazaLanguage] {
        get {
            let stored = defaults.array(forKey: Key.enabledLanguages) as? [String] ?? []
            let languages = stored.compactMap(TazaLanguage.init(rawValue:))
            return languages.isEmpty ? TazaLanguage.allCases : languages
        }
        nonmutating set {
            defaults.set(newValue.map(\.rawValue), forKey: Key.enabledLanguages)
        }
    }

    public var lastUsedLanguage: TazaLanguage {
        get {
            let stored = defaults.string(forKey: Key.lastUsedLanguage)
                .flatMap(TazaLanguage.init(rawValue:))
            guard let stored, enabledLanguages.contains(stored) else {
                return enabledLanguages[0]
            }
            return stored
        }
        nonmutating set {
            defaults.set(newValue.rawValue, forKey: Key.lastUsedLanguage)
        }
    }

    /// 언어 키를 탭했을 때 갈 다음 언어
    public func language(after current: TazaLanguage) -> TazaLanguage {
        let languages = enabledLanguages
        guard let index = languages.firstIndex(of: current) else {
            return languages[0]
        }
        return languages[(index + 1) % languages.count]
    }
}

/// 익스텐션에서 컨테이너 앱(설정)을 여는 딥 링크
public enum TazaDeepLink {
    public static let scheme = "taza"
    public static let settings = URL(string: "taza://settings")!
}
