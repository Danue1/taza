import Foundation

/// 순정 키보드의 입력 보조 설정에 대응하는 사용자 설정. iOS는 설정 → 일반 → 키보드의
/// 토글을 서드파티 키보드에 열어 주지 않으므로(값을 읽을 방법이 없다) 값의 주인은 우리
/// 설정 앱이고, 키보드 익스텐션은 표시될 때마다 이 값을 코어에 주입한다.
///
/// 저장된 적 없는 항목은 코어가 밝힌 기본값을 따른다 — 셸이 자기 기본값 표를 따로
/// 두지 않는다.
public struct TypingPreferences {
    private enum Key {
        static let autoCorrection = "autoCorrection"
        static let predictions = "predictions"
        static let doubleSpacePeriod = "doubleSpacePeriod"
        static let personalizedLearning = "personalizedLearning"
    }

    private let defaults: UserDefaults

    /// App Group을 못 쓰는 상황(설정 누락·시뮬레이터)에서도 키보드가 동작해야 하므로
    /// 표준 저장소로 물러난다 — 이때는 앱과 값을 공유하지 못할 뿐이다.
    public init(defaults: UserDefaults? = nil) {
        self.defaults = defaults
            ?? UserDefaults(suiteName: LanguagePreferences.appGroupIdentifier)
            ?? .standard
    }

    public var autoCorrection: Bool {
        get { flag(Key.autoCorrection, default: defaultUserPreferences().autoCorrection) }
        nonmutating set { defaults.set(newValue, forKey: Key.autoCorrection) }
    }

    public var predictions: Bool {
        get { flag(Key.predictions, default: defaultUserPreferences().predictions) }
        nonmutating set { defaults.set(newValue, forKey: Key.predictions) }
    }

    public var doubleSpacePeriod: Bool {
        get { flag(Key.doubleSpacePeriod, default: defaultUserPreferences().doubleSpacePeriod) }
        nonmutating set { defaults.set(newValue, forKey: Key.doubleSpacePeriod) }
    }

    public var personalizedLearning: Bool {
        get {
            flag(Key.personalizedLearning, default: defaultUserPreferences().personalizedLearning)
        }
        nonmutating set { defaults.set(newValue, forKey: Key.personalizedLearning) }
    }

    /// 코어에 넘기는 형태
    public var core: FfiUserPreferences {
        FfiUserPreferences(
            autoCorrection: autoCorrection,
            predictions: predictions,
            doubleSpacePeriod: doubleSpacePeriod,
            personalizedLearning: personalizedLearning
        )
    }

    private func flag(_ key: String, default fallback: Bool) -> Bool {
        defaults.object(forKey: key) as? Bool ?? fallback
    }
}
