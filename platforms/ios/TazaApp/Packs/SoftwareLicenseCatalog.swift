import Foundation

/// 기기에 나가는 라이브러리의 라이선스. 목록은 `taza-licenses`가 의존성 그래프에서
/// 만들어 앱 자원으로 싣는다 — 앱이 자기 표를 들고 있지 않으므로 의존성을 더하면
/// 고지가 저절로 따라온다.
struct SoftwareLicenseCatalog: Decodable {
    /// 라이선스 본문은 크레이트마다 거의 같아 한 번만 싣고 번호로 가리킨다
    let texts: [String]
    let packages: [Package]

    struct Package: Decodable {
        let name: String
        let version: String
        /// SPDX 식별자 — 크레이트가 밝히지 않았으면 비어 있다
        let license: String
        let repository: String
        let texts: [Int]
    }

    static let resourceName = "licenses"

    /// 자원이 없거나 형식이 다르면 빈 목록 — 고지 화면이 비는 것이 앱이 죽는 것보다 낫다.
    static func load(bundle: Bundle = .main) -> SoftwareLicenseCatalog {
        guard let url = bundle.url(forResource: resourceName, withExtension: "json"),
              let data = try? Data(contentsOf: url),
              let catalog = try? JSONDecoder().decode(SoftwareLicenseCatalog.self, from: data)
        else {
            return SoftwareLicenseCatalog(texts: [], packages: [])
        }
        return catalog
    }

    func texts(for package: Package) -> [String] {
        package.texts.compactMap { texts.indices.contains($0) ? texts[$0] : nil }
    }
}
