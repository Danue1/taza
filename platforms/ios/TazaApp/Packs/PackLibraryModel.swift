import Foundation

/// 언어별 사전 상태와 다운로드 진행을 들고 있는 화면 모델.
/// 다운로드·설치는 컨테이너 앱 전담이라 이 타입은 앱 타깃에만 있다.
@MainActor
final class PackLibraryModel: ObservableObject {
    enum State: Equatable {
        /// 앱에 내장된 기본 언어 — 내려받을 것이 없다
        case bundled(packVersion: UInt32)
        case notInstalled(size: UInt64)
        case installed(packVersion: UInt32, updateAvailable: Bool)
        case installing
        case unavailable(String)
    }

    @Published private(set) var states: [TazaLanguage: State] = [:]
    /// 언어마다 그 사전이 밝힌 원천들 — 설치·삭제로 목록이 달라진다
    @Published private(set) var sources: [TazaLanguage: [FfiPackSource]] = [:]

    private let store: PackStore
    private let installer: PackInstaller?
    private var catalog: PackCatalog?

    init() {
        let store = PackStore()
        self.store = store
        installer = PackDistribution.catalogURL().map { PackInstaller(catalogURL: $0, store: store) }
    }

    func refresh() async {
        for language in TazaLanguage.all {
            states[language] = resolveLocalState(language)
        }
        collectSources()

        guard let installer else {
            markMissingAsUnavailable(PackInstallError.catalogUnavailable)
            return
        }
        do {
            let catalog = try await installer.loadCatalog()
            self.catalog = catalog
            for language in TazaLanguage.all {
                states[language] = resolveState(language, catalog: catalog)
            }
        } catch {
            markMissingAsUnavailable(error)
        }
    }

    func install(_ language: TazaLanguage) async {
        guard let installer, let catalog else { return }
        guard let entry = catalog.packs.first(where: { $0.name == language.packName }) else {
            states[language] = .unavailable(
                PackInstallError.notInCatalog(language.packName).localizedDescription
            )
            return
        }
        states[language] = .installing
        do {
            _ = try await installer.install(entry, for: language)
            states[language] = resolveState(language, catalog: catalog)
            collectSources()
            // 사전이 바뀌면 키보드가 팩을 다시 열어야 한다
            SettingsBroadcast.post(.settings)
        } catch {
            states[language] = .unavailable(error.localizedDescription)
        }
    }

    func remove(_ language: TazaLanguage) {
        try? store.removeInstalledPack(for: language)
        states[language] = resolveLocalState(language)
        collectSources()
        SettingsBroadcast.post(.settings)
    }

    // MARK: - 상태 판정

    private func resolveLocalState(_ language: TazaLanguage) -> State {
        if let installed = installedPack(language) {
            return .installed(packVersion: installed.packVersion, updateAvailable: false)
        }
        if let bundled = store.bundledURL(for: language),
           let pack = try? readInstalledPack(path: bundled.path)
        {
            return .bundled(packVersion: pack.packVersion)
        }
        return .notInstalled(size: 0)
    }

    private func resolveState(_ language: TazaLanguage, catalog: PackCatalog) -> State {
        let entry = catalog.packs.first { $0.name == language.packName }
        if let installed = installedPack(language) {
            let newer = (entry?.packVersion ?? 0) > installed.packVersion
            return .installed(packVersion: installed.packVersion, updateAvailable: newer)
        }
        if case .bundled(let version) = resolveLocalState(language) {
            // 내장 팩보다 새 판이 배포됐으면 내려받아 덮어쓸 수 있다
            if let entry, entry.packVersion > version {
                return .notInstalled(size: entry.archiveSize)
            }
            return .bundled(packVersion: version)
        }
        guard let entry else {
            return .unavailable(
                PackInstallError.notInCatalog(language.packName).localizedDescription
            )
        }
        return .notInstalled(size: entry.archiveSize)
    }

    private func installedPack(_ language: TazaLanguage) -> FfiInstalledPack? {
        guard let url = store.installedURL(for: language) else { return nil }
        return try? readInstalledPack(path: url.path)
    }

    private func markMissingAsUnavailable(_ error: Error) {
        for language in TazaLanguage.all {
            if case .notInstalled = states[language] {
                states[language] = .unavailable(error.localizedDescription)
            }
        }
    }

    /// 고지는 팩 자체에서 읽는다 — 원천이 바뀌면 표시도 함께 바뀐다. 어느 사전이
    /// 무엇을 쓰는지가 고지의 핵심이므로 언어별로 나눠 둔다.
    private func collectSources() {
        sources = Dictionary(
            uniqueKeysWithValues: TazaLanguage.all.compactMap { language in
                guard let url = store.packURL(for: language),
                      let pack = try? readInstalledPack(path: url.path),
                      !pack.sources.isEmpty
                else {
                    return nil
                }
                return (language, pack.sources)
            }
        )
    }
}
