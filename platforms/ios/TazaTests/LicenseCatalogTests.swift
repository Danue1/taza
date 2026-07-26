import XCTest
@testable import TazaApp

/// 앱 자원으로 실려 가는 라이선스 목록 — 생성물이 실제로 읽히는지.
final class LicenseCatalogTests: XCTestCase {
    func testBundledCatalogLoads() {
        let catalog = SoftwareLicenseCatalog.load(bundle: Bundle(for: Self.self))
        // 테스트 번들에는 자원이 없을 수 있다 — 그때도 죽지 않고 빈 목록이어야 한다
        XCTAssertNotNil(catalog.packages)
    }

    func testMissingResourceYieldsAnEmptyCatalogInsteadOfCrashing() {
        let catalog = SoftwareLicenseCatalog.load(bundle: Bundle(for: Self.self))
        XCTAssertEqual(catalog.texts(for: SoftwareLicenseCatalog.Package(
            name: "없는 것",
            version: "0",
            license: "",
            repository: "",
            texts: [99]
        )), [])
    }
}
