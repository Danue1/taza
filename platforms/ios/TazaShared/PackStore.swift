import Foundation

/// 언어팩이 놓이는 자리와 그 자리에서 무엇을 읽을지의 규칙.
///
/// 언어팩 다운로드는 컨테이너 앱이 전담하고, 익스텐션은 App Group에 놓인 파일을 mmap으로
/// 읽기만 한다 — 익스텐션은 네트워크를 쓰지 않는다. 기본 언어(영어)는 앱 번들에 내장돼
/// 있으므로 아무것도 내려받지 않은 상태에서도 키보드가 동작한다.
public struct PackStore {
    /// App Group 컨테이너 안에서 팩을 모아 두는 디렉터리
    private static let directoryName = "Packs"

    private let containerURL: URL?
    private let bundle: Bundle

    /// App Group을 못 쓰는 상황(서명 팀 미설정·시뮬레이터)에서는 설치 자리가 없다 —
    /// 이때는 내장 팩만 쓰이고, 앱은 다운로드를 "설정 필요"로 표시한다.
    public init(
        appGroupIdentifier: String = LanguagePreferences.appGroupIdentifier,
        bundle: Bundle = .main
    ) {
        self.containerURL = FileManager.default
            .containerURL(forSecurityApplicationGroupIdentifier: appGroupIdentifier)
        self.bundle = bundle
    }

    public var installDirectory: URL? {
        containerURL?.appendingPathComponent(Self.directoryName, isDirectory: true)
    }

    public func installURL(for language: TazaLanguage) -> URL? {
        installDirectory?.appendingPathComponent("\(language.packName).tazapack")
    }

    public func installedURL(for language: TazaLanguage) -> URL? {
        guard let url = installURL(for: language),
              FileManager.default.fileExists(atPath: url.path)
        else {
            return nil
        }
        return url
    }

    /// 내장 팩은 익스텐션 번들에 실린다(키보드가 읽는 것이므로). 설정 앱도 같은 팩을
    /// 읽어야 배열 목록·언어팩 상태가 키보드와 어긋나지 않으므로, 자기 번들에 없으면
    /// 앱 안에 들어 있는 익스텐션 번들까지 본다.
    public func bundledURL(for language: TazaLanguage) -> URL? {
        if let url = bundle.url(forResource: language.packName, withExtension: "tazapack") {
            return url
        }
        return Self.extensionBundles(in: bundle)
            .compactMap { $0.url(forResource: language.packName, withExtension: "tazapack") }
            .first
    }

    private static func extensionBundles(in bundle: Bundle) -> [Bundle] {
        guard let plugins = bundle.builtInPlugInsURL,
              let contents = try? FileManager.default.contentsOfDirectory(
                  at: plugins,
                  includingPropertiesForKeys: nil
              )
        else {
            return []
        }
        return contents
            .filter { $0.pathExtension == "appex" }
            .compactMap(Bundle.init(url:))
    }

    /// 키보드가 실제로 열 팩 — 내려받은 판이 있으면 그쪽이 이긴다(갱신 배포 경로).
    public func packURL(for language: TazaLanguage) -> URL? {
        installedURL(for: language) ?? bundledURL(for: language)
    }

    public func removeInstalledPack(for language: TazaLanguage) throws {
        guard let url = installedURL(for: language) else { return }
        try FileManager.default.removeItem(at: url)
    }
}
