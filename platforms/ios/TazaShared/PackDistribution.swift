import Foundation

/// 배포 카탈로그 — 오프라인 파이프라인(`taza-packs`)이 팩과 함께 만들어 두는 목록.
/// 무엇을 어디서 받아 어떤 해시여야 하는지가 여기에만 적혀 있다.
public struct PackCatalog: Codable, Sendable {
    public let formatVersion: UInt16
    public let packs: [PackCatalogEntry]
}

public struct PackCatalogEntry: Codable, Sendable {
    public let name: String
    public let language: String
    public let packVersion: UInt32
    public let wordCount: Int
    /// 카탈로그 URL 기준 상대 경로
    public let archiveFile: String
    public let archiveSize: UInt64
    public let archiveSha256: String
    public let packSize: UInt64
    public let packSha256: String
    /// 원천 목록 — 팩 메타데이터와 같은 탭 구분 형태. 저작자 표시도 이 줄 안에 있다
    /// (이름과 문구를 따로 두면 짝이 위치로만 맞아 하나만 비어도 전부 어긋난다).
    public let sources: String

    public var taza: TazaLanguage? {
        TazaLanguage.all.first { $0.packName == name }
    }
}

/// 팩 배포처. 앱 번들의 `TazaPackCatalogURL`에 적어 두고, 개발 중에는 로컬
/// `file://` 카탈로그를 가리켜도 그대로 돈다.
public enum PackDistribution {
    public static let catalogURLKey = "TazaPackCatalogURL"

    public static func catalogURL(bundle: Bundle = .main) -> URL? {
        guard let text = bundle.object(forInfoDictionaryKey: catalogURLKey) as? String,
              !text.isEmpty
        else {
            return nil
        }
        return URL(string: text)
    }
}

public enum PackInstallError: Error, LocalizedError {
    case catalogUnavailable
    case notInCatalog(String)
    case installDirectoryUnavailable
    case formatTooNew(UInt16)
    case sizeMismatch(expected: UInt64, actual: UInt64)

    public var errorDescription: String? {
        switch self {
        case .catalogUnavailable:
            NSLocalizedString("언어팩 배포처가 설정되지 않았습니다.", comment: "카탈로그 URL 없음")
        case .notInCatalog(let name):
            String(
                format: NSLocalizedString("카탈로그에 %@ 팩이 없습니다.", comment: "카탈로그에 없는 팩"),
                name
            )
        case .installDirectoryUnavailable:
            NSLocalizedString(
                "App Group을 쓸 수 없어 언어팩을 설치할 자리가 없습니다.",
                comment: "App Group 미활성"
            )
        case .formatTooNew(let version):
            String(
                format: NSLocalizedString(
                    "이 앱보다 새로운 팩 포맷(%lld)입니다. 앱을 업데이트하세요.",
                    comment: "앱이 못 읽는 팩 포맷"
                ),
                Int(version)
            )
        case .sizeMismatch(let expected, let actual):
            "받은 크기가 카탈로그와 다릅니다(기대 \(expected), 실제 \(actual))."
        }
    }
}

/// 카탈로그를 읽고 팩을 내려받아 설치한다. 해시 검증과 원자적 교체는 코어(FFI)가 하고,
/// 이 타입은 네트워크와 파일 자리만 맡는다.
public actor PackInstaller {
    private let catalogURL: URL
    private let store: PackStore
    private let session: URLSession

    public init(catalogURL: URL, store: PackStore = PackStore(), session: URLSession = .shared) {
        self.catalogURL = catalogURL
        self.store = store
        self.session = session
    }

    public func loadCatalog() async throws -> PackCatalog {
        let (data, _) = try await session.data(from: catalogURL)
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        let catalog = try decoder.decode(PackCatalog.self, from: data)
        // 팩 포맷이 앱보다 새로우면 받아도 열 수 없다 — 받기 전에 멈춘다.
        guard catalog.formatVersion <= supportedPackFormatVersion() else {
            throw PackInstallError.formatTooNew(catalog.formatVersion)
        }
        return catalog
    }

    public func install(_ entry: PackCatalogEntry, for language: TazaLanguage) async throws
        -> FfiInstalledPack
    {
        guard let destination = store.installURL(for: language) else {
            throw PackInstallError.installDirectoryUnavailable
        }
        let archiveURL = URL(string: entry.archiveFile, relativeTo: catalogURL) ?? catalogURL
        let (downloaded, _) = try await session.download(from: archiveURL)
        defer { try? FileManager.default.removeItem(at: downloaded) }

        let size = (try? FileManager.default.attributesOfItem(atPath: downloaded.path)[.size])
            .flatMap { ($0 as? NSNumber)?.uint64Value } ?? 0
        guard size == entry.archiveSize else {
            throw PackInstallError.sizeMismatch(expected: entry.archiveSize, actual: size)
        }
        return try installPackArchive(
            archivePath: downloaded.path,
            destinationPath: destination.path,
            expectedArchiveSha256: entry.archiveSha256,
            expectedPackSha256: entry.packSha256
        )
    }

    /// 이미 설치된 팩의 판 번호 — 카탈로그와 비교해 갱신이 필요한지 판단한다.
    public func installedPack(for language: TazaLanguage) -> FfiInstalledPack? {
        guard let url = store.installedURL(for: language) else { return nil }
        return try? readInstalledPack(path: url.path)
    }
}
