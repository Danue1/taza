import Foundation

/// 셸이 소유하는 언어 목록의 항목. 지구본 키는 시스템 키보드끼리의 전환이라, 우리
/// 키보드가 지원하는 언어 사이의 전환·순서·최근 언어는 앱이 직접 관리한다.
///
/// 표시 이름·키캡 표기·배열 이름은 여기에 두지 않는다 — 코어가 팩 선언에서 읽어
/// 알려 주므로(`KeyboardSession.language()`), 언어를 늘리는 일이 아래 목록 한 줄과
/// 팩 배포로 끝난다.
public struct TazaLanguage: Hashable, Sendable {
    /// 언어 태그 — 코어·팩·설정이 언어를 가리키는 유일한 이름
    public let tag: String
    /// 배포 파일 이름 (`english` → `english.tazapack`)
    public let packName: String

    /// 이 빌드가 다루는 언어들. 팩을 새로 만들면 여기에 한 줄을 더한다.
    public static let all: [TazaLanguage] = [
        TazaLanguage(tag: "en", packName: "english"),
        TazaLanguage(tag: "ko", packName: "korean"),
    ]

    public static func named(_ tag: String) -> TazaLanguage? {
        all.first { $0.tag == tag }
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
            let languages = stored.compactMap(TazaLanguage.named)
            return languages.isEmpty ? TazaLanguage.all : languages
        }
        nonmutating set {
            defaults.set(newValue.map(\.tag), forKey: Key.enabledLanguages)
        }
    }

    /// 아직 쓰지 않는 언어 — 설정에서 추가할 수 있는 것들
    public var availableLanguages: [TazaLanguage] {
        let enabled = enabledLanguages
        return TazaLanguage.all.filter { !enabled.contains($0) }
    }

    /// 새 언어는 순환 순서의 끝에 붙는다 — 쓰던 순서가 흔들리지 않게 한다.
    public func enable(_ language: TazaLanguage) {
        var languages = enabledLanguages
        guard !languages.contains(language) else { return }
        languages.append(language)
        enabledLanguages = languages
    }

    /// 마지막 한 언어는 지울 수 없다 — 지우면 키보드가 그릴 배열이 남지 않는다.
    public func disable(_ language: TazaLanguage) {
        var languages = enabledLanguages
        guard languages.count > 1, let index = languages.firstIndex(of: language) else { return }
        languages.remove(at: index)
        enabledLanguages = languages
    }

    public var lastUsedLanguage: TazaLanguage {
        get {
            let stored = defaults.string(forKey: Key.lastUsedLanguage)
                .flatMap(TazaLanguage.named)
            guard let stored, enabledLanguages.contains(stored) else {
                return enabledLanguages[0]
            }
            return stored
        }
        nonmutating set {
            defaults.set(newValue.tag, forKey: Key.lastUsedLanguage)
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

/// 언어 선언을 코어에서 읽는다 — 설치된 팩이 있으면 팩이 밝힌 값, 없으면 내장 값이다.
/// 표시 이름·키캡 표기·배열 이름을 셸이 따로 표로 갖지 않게 하는 통로다.
public func tazaLanguageDescriptor(
    for language: TazaLanguage,
    packURL: URL? = nil
) -> FfiLanguageDescriptor? {
    guard let session = try? KeyboardSession(languageTag: language.tag) else {
        return nil
    }
    if let packURL {
        try? session.loadPack(path: packURL.path)
    }
    return session.language()
}
