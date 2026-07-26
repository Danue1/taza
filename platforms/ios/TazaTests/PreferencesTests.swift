import XCTest
@testable import TazaApp

/// 설정 저장소의 규칙 — 2층 구조와 기본값의 주인이 코어라는 점.
final class PreferencesTests: XCTestCase {
    private var defaults: UserDefaults!

    override func setUp() {
        super.setUp()
        // 테스트마다 빈 저장소를 쓴다 — App Group을 건드리면 시뮬레이터 상태가 남는다
        defaults = UserDefaults(suiteName: "io.danuel.taza.tests.\(UUID().uuidString)")
    }

    override func tearDown() {
        defaults = nil
        super.tearDown()
    }

    func testDefaultsComeFromTheCore() {
        let typing = TypingPreferences(defaults: defaults)
        let core = defaultUserPreferences()
        XCTAssertEqual(typing.common(.autoCorrection), core.autoCorrection)
        XCTAssertEqual(typing.common(.autoPairing), core.autoPairing)
        // 셸이 자기 기본값 표를 두지 않는다는 뜻이다
        XCTAssertEqual(typing.common(.autoCapitalization), core.autoCapitalization)
    }

    func testLanguageValueOverridesTheCommonOne() {
        let typing = TypingPreferences(defaults: defaults)
        let korean = TazaLanguage.named("ko")!
        typing.setCommon(.autoCorrection, true)
        XCTAssertTrue(typing.value(.autoCorrection, for: korean))

        typing.setLanguageValue(.autoCorrection, for: korean, false)
        XCTAssertFalse(typing.value(.autoCorrection, for: korean))
        // 공통값은 건드리지 않는다
        XCTAssertTrue(typing.common(.autoCorrection))

        // nil을 넣으면 다시 공통을 따른다
        typing.setLanguageValue(.autoCorrection, for: korean, nil)
        XCTAssertNil(typing.languageValue(.autoCorrection, for: korean))
        XCTAssertTrue(typing.value(.autoCorrection, for: korean))
    }

    func testRemovingALanguageDropsItsOverrides() {
        let typing = TypingPreferences(defaults: defaults)
        let korean = TazaLanguage.named("ko")!
        typing.setLanguageValue(.predictions, for: korean, false)
        typing.removeValues(for: korean)
        XCTAssertNil(typing.languageValue(.predictions, for: korean))
    }

    func testCoreSnapshotMergesBothLayers() {
        let typing = TypingPreferences(defaults: defaults)
        let keyboard = KeyboardPreferences(defaults: defaults)
        let korean = TazaLanguage.named("ko")!
        typing.setLanguageValue(.autoCorrection, for: korean, false)
        keyboard.numberRow = true

        let core = typing.core(for: korean)
        XCTAssertFalse(core.autoCorrection)
        XCTAssertTrue(core.numberRow)
    }

    func testChoicesSurviveAsStringsNotOrdinals() {
        let keyboard = KeyboardPreferences(defaults: defaults)
        keyboard.keyboardHeight = .tall
        // 정수로 남기면 코어에서 갈래 순서가 바뀔 때 뜻이 조용히 달라진다
        XCTAssertEqual(defaults.string(forKey: "keyboardHeight"), "tall")
        XCTAssertEqual(keyboard.keyboardHeight.core, .tall)
    }
}
