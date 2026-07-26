import Foundation

/// 입력 보조 항목 — 언어마다 따로 정할 수 있는 켬·끔들이다. 저장 키를 항목 자신이 알고
/// 있어, 저장소와 화면이 항목 표를 따로 두지 않는다.
///
/// 여기 있는 것과 `KeyboardPreferences`에 있는 것의 차이는 값의 종류가 아니라 **언어마다
/// 다를 수 있는가**다. 자동 수정은 영어에서는 켜고 한국어에서는 끌 이유가 있지만, 키보드
/// 높이는 언어를 바꾼다고 달라질 것이 아니다.
public enum TypingOption: String, CaseIterable, Sendable {
    case autoCorrection
    case predictions
    case doubleSpacePeriod
    case personalizedLearning
    case autoCapitalization
    case smartPunctuation
    case autoPairing
    case annotationCandidates

    /// 저장된 적 없는 항목이 따르는 값 — 셸이 자기 기본값 표를 두지 않고 코어에 묻는다.
    var coreDefault: Bool {
        let defaults = defaultUserPreferences()
        switch self {
        case .autoCorrection: return defaults.autoCorrection
        case .predictions: return defaults.predictions
        case .doubleSpacePeriod: return defaults.doubleSpacePeriod
        case .personalizedLearning: return defaults.personalizedLearning
        case .autoCapitalization: return defaults.autoCapitalization
        case .smartPunctuation: return defaults.smartPunctuation
        case .autoPairing: return defaults.autoPairing
        case .annotationCandidates: return defaults.annotationCandidates
        }
    }
}

/// 순정 키보드의 입력 보조 설정에 대응하는 사용자 설정. iOS는 설정 → 일반 → 키보드의
/// 토글을 서드파티 키보드에 열어 주지 않으므로(값을 읽을 방법이 없다) 값의 주인은 우리
/// 설정 앱이고, 키보드 익스텐션은 표시될 때마다 이 값을 코어에 주입한다.
///
/// 값은 두 층이다: 모든 언어에 적용되는 공통값과, 언어가 항목마다 따로 정한 값. 언어가
/// 정하지 않은 항목은 공통값을 그대로 쓴다 — 언어를 늘려도 설정할 것이 늘지 않는다.
public struct TypingPreferences {
    private let defaults: UserDefaults

    /// App Group을 못 쓰는 상황(설정 누락·시뮬레이터)에서도 키보드가 동작해야 하므로
    /// 표준 저장소로 물러난다 — 이때는 앱과 값을 공유하지 못할 뿐이다.
    public init(defaults: UserDefaults? = nil) {
        self.defaults = defaults
            ?? UserDefaults(suiteName: LanguagePreferences.appGroupIdentifier)
            ?? .standard
    }

    // MARK: - 공통값

    public func common(_ option: TypingOption) -> Bool {
        flag(option.rawValue, default: option.coreDefault)
    }

    public func setCommon(_ option: TypingOption, _ value: Bool) {
        defaults.set(value, forKey: option.rawValue)
    }

    // MARK: - 언어별 값

    /// 그 언어가 따로 정한 값 — nil이면 공통값을 따른다.
    public func languageValue(_ option: TypingOption, for language: TazaLanguage) -> Bool? {
        defaults.object(forKey: key(option, language)) as? Bool
    }

    /// nil을 넣으면 그 언어의 값을 지워 다시 공통값을 따르게 한다.
    public func setLanguageValue(
        _ option: TypingOption,
        for language: TazaLanguage,
        _ value: Bool?
    ) {
        guard let value else {
            defaults.removeObject(forKey: key(option, language))
            return
        }
        defaults.set(value, forKey: key(option, language))
    }

    /// 그 언어에 실제로 적용되는 값
    public func value(_ option: TypingOption, for language: TazaLanguage) -> Bool {
        languageValue(option, for: language) ?? common(option)
    }

    /// 언어를 지울 때 그 언어에 매달린 값도 함께 지운다 — 다시 추가하면 공통값에서 시작한다.
    public func removeValues(for language: TazaLanguage) {
        for option in TypingOption.allCases {
            defaults.removeObject(forKey: key(option, language))
        }
    }

    /// 코어에 넘기는 형태 — 언어마다 유효값이 다를 수 있으므로 세션마다 따로 만든다.
    /// 언어를 타지 않는 설정은 `KeyboardPreferences`에서 그대로 얹는다.
    public func core(for language: TazaLanguage) -> FfiUserPreferences {
        let keyboard = KeyboardPreferences(defaults: defaults)
        return FfiUserPreferences(
            autoCorrection: value(.autoCorrection, for: language),
            predictions: value(.predictions, for: language),
            doubleSpacePeriod: value(.doubleSpacePeriod, for: language),
            personalizedLearning: value(.personalizedLearning, for: language),
            autoCapitalization: value(.autoCapitalization, for: language),
            smartPunctuation: value(.smartPunctuation, for: language),
            autoPairing: value(.autoPairing, for: language),
            annotationCandidates: value(.annotationCandidates, for: language),
            keyAlternates: keyboard.keyAlternates,
            numberRow: keyboard.numberRow,
            candidateBarAlways: keyboard.candidateBarAlways,
            keyboardHeight: keyboard.keyboardHeight.core,
            cursorSensitivity: keyboard.cursorSensitivity.core
        )
    }

    // MARK: - 저장소

    private func flag(_ key: String, default fallback: Bool) -> Bool {
        defaults.object(forKey: key) as? Bool ?? fallback
    }

    private func key(_ option: TypingOption, _ language: TazaLanguage) -> String {
        "\(language.tag).\(option.rawValue)"
    }
}
