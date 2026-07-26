import Foundation

/// 언어를 타지 않는 키보드 설정. 두 갈래가 한 저장소에 있다:
///
/// - **코어가 판단하는 것**(높이·숫자 행·변형 문자·커서 감도): `FfiUserPreferences`에
///   실려 세션으로 들어간다. 셸은 값을 나르기만 한다.
/// - **셸이 그리고 느끼게 하는 것**(사운드·햅틱·테마·미리보기): 코어에 판단할 것이
///   없으므로 계약을 늘리지 않고 익스텐션이 직접 읽는다.
///
/// 갈래가 달라도 사용자에게는 같은 설정 화면의 항목들이므로 저장소는 하나로 둔다.
public final class KeyboardPreferences {
    private let defaults: UserDefaults

    public init(defaults: UserDefaults? = nil) {
        self.defaults = defaults
            ?? UserDefaults(suiteName: LanguagePreferences.appGroupIdentifier)
            ?? .standard
    }

    // MARK: - 코어로 가는 값

    public var keyAlternates: Bool {
        get { flag(Key.keyAlternates, default: coreDefaults.keyAlternates) }
        set { defaults.set(newValue, forKey: Key.keyAlternates) }
    }

    public var numberRow: Bool {
        get { flag(Key.numberRow, default: coreDefaults.numberRow) }
        set { defaults.set(newValue, forKey: Key.numberRow) }
    }

    public var candidateBarAlways: Bool {
        get { flag(Key.candidateBarAlways, default: coreDefaults.candidateBarAlways) }
        set { defaults.set(newValue, forKey: Key.candidateBarAlways) }
    }

    public var keyboardHeight: KeyboardHeightChoice {
        get {
            KeyboardHeightChoice(rawValue: defaults.string(forKey: Key.keyboardHeight) ?? "")
                ?? KeyboardHeightChoice(coreDefaults.keyboardHeight)
        }
        set { defaults.set(newValue.rawValue, forKey: Key.keyboardHeight) }
    }

    public var cursorSensitivity: CursorSensitivityChoice {
        get {
            CursorSensitivityChoice(rawValue: defaults.string(forKey: Key.cursorSensitivity) ?? "")
                ?? CursorSensitivityChoice(coreDefaults.cursorSensitivity)
        }
        set { defaults.set(newValue.rawValue, forKey: Key.cursorSensitivity) }
    }

    // MARK: - 셸이 쓰는 값

    /// 키를 누를 때 시스템 키보드 클릭음을 낸다. 순정과 같은 소리를 쓰려면 익스텐션에
    /// 전체 접근 권한이 필요하다 — 권한이 없으면 조용히 넘어간다.
    public var keySound: Bool {
        get { flag(Key.keySound, default: true) }
        set { defaults.set(newValue, forKey: Key.keySound) }
    }

    /// 키를 누를 때의 진동. 순정에는 없고 서드파티 키보드에는 흔하다.
    public var haptics: Bool {
        get { flag(Key.haptics, default: false) }
        set { defaults.set(newValue, forKey: Key.haptics) }
    }

    /// 글자 키를 누를 때 그 글자를 키 위에 크게 띄운다(순정 관례).
    public var keyPreview: Bool {
        get { flag(Key.keyPreview, default: true) }
        set { defaults.set(newValue, forKey: Key.keyPreview) }
    }

    /// shift를 두 번 눌러 대문자 고정(순정 관례).
    public var shiftDoubleTapLock: Bool {
        get { flag(Key.shiftDoubleTapLock, default: true) }
        set { defaults.set(newValue, forKey: Key.shiftDoubleTapLock) }
    }

    /// 스페이스바를 좌우로 밀어 언어를 바꾼다. 길게 눌러 커서를 옮기는 동작과 뜻이
    /// 겹치므로 기본은 꺼 둔다.
    public var spaceSwipeLanguage: Bool {
        get { flag(Key.spaceSwipeLanguage, default: false) }
        set { defaults.set(newValue, forKey: Key.spaceSwipeLanguage) }
    }

    /// 키에 테두리를 그린다.
    public var keyBorders: Bool {
        get { flag(Key.keyBorders, default: false) }
        set { defaults.set(newValue, forKey: Key.keyBorders) }
    }

    public var backspaceSpeed: BackspaceSpeed {
        get { BackspaceSpeed(rawValue: defaults.string(forKey: Key.backspaceSpeed) ?? "") ?? .standard }
        set { defaults.set(newValue.rawValue, forKey: Key.backspaceSpeed) }
    }

    public var theme: ThemeChoice {
        get { ThemeChoice(rawValue: defaults.string(forKey: Key.theme) ?? "") ?? .system }
        set { defaults.set(newValue.rawValue, forKey: Key.theme) }
    }

    // MARK: - 저장소

    private var coreDefaults: FfiUserPreferences { defaultUserPreferences() }

    private func flag(_ key: String, default fallback: Bool) -> Bool {
        defaults.object(forKey: key) as? Bool ?? fallback
    }

    private enum Key {
        static let keyAlternates = "keyAlternates"
        static let numberRow = "numberRow"
        static let candidateBarAlways = "candidateBarAlways"
        static let keyboardHeight = "keyboardHeight"
        static let cursorSensitivity = "cursorSensitivity"
        static let keySound = "keySound"
        static let haptics = "haptics"
        static let keyPreview = "keyPreview"
        static let shiftDoubleTapLock = "shiftDoubleTapLock"
        static let spaceSwipeLanguage = "spaceSwipeLanguage"
        static let keyBorders = "keyBorders"
        static let backspaceSpeed = "backspaceSpeed"
        static let theme = "theme"
    }
}

/// 코어 열거형을 저장소에 남길 수 있는 문자열로 옮긴다 — 정수로 남기면 코어에서 갈래
/// 순서가 바뀔 때 저장된 값의 뜻이 조용히 달라진다.
public enum KeyboardHeightChoice: String, CaseIterable, Sendable {
    case compact
    case standard
    case tall

    public init(_ core: FfiKeyboardHeight) {
        switch core {
        case .compact: self = .compact
        case .standard: self = .standard
        case .tall: self = .tall
        }
    }

    public var core: FfiKeyboardHeight {
        switch self {
        case .compact: return .compact
        case .standard: return .standard
        case .tall: return .tall
        }
    }
}

public enum CursorSensitivityChoice: String, CaseIterable, Sendable {
    case low
    case standard
    case high

    public init(_ core: FfiCursorSensitivity) {
        switch core {
        case .low: self = .low
        case .standard: self = .standard
        case .high: self = .high
        }
    }

    public var core: FfiCursorSensitivity {
        switch self {
        case .low: return .low
        case .standard: return .standard
        case .high: return .high
        }
    }
}

/// 백스페이스를 누르고 있을 때 이어 지우는 빠르기. 코어에는 판단할 것이 없어 셸이 갖는다.
public enum BackspaceSpeed: String, CaseIterable, Sendable {
    case slow
    case standard
    case fast

    /// 첫 틱까지의 간격과 가장 빨라졌을 때의 간격(초)
    public var interval: (first: TimeInterval, fastest: TimeInterval) {
        switch self {
        case .slow: return (0.24, 0.07)
        case .standard: return (0.16, 0.03)
        case .fast: return (0.11, 0.02)
        }
    }
}

public enum ThemeChoice: String, CaseIterable, Sendable {
    case system
    case light
    case dark
}
