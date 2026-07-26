import Foundation

/// 사용자가 정한 텍스트 대치(순정 설정 → 일반 → 키보드 → 텍스트 대치).
///
/// 값의 주인은 설정 앱이고 익스텐션은 주입만 받는다 — 학습과 달리 익스텐션이 여기에
/// 쓰는 일이 없어, 두 프로세스가 같은 값을 다투지 않는다.
public struct ShortcutStore {
    private static let key = "shortcuts"

    private let defaults: UserDefaults

    public init(defaults: UserDefaults? = nil) {
        self.defaults = defaults
            ?? UserDefaults(suiteName: LanguagePreferences.appGroupIdentifier)
            ?? .standard
    }

    /// 친 말 순서대로. 같은 말이 둘이면 앞의 것이 이긴다.
    public var entries: [FfiShortcut] {
        get {
            let stored = defaults.array(forKey: Self.key) as? [[String]] ?? []
            return stored.compactMap { pair in
                guard pair.count == 2, !pair[0].isEmpty else { return nil }
                return FfiShortcut(trigger: pair[0], replacement: pair[1])
            }
        }
        nonmutating set {
            defaults.set(newValue.map { [$0.trigger, $0.replacement] }, forKey: Self.key)
        }
    }

    /// 이미 있는 말이면 덮어쓴다 — 같은 말에 두 대치를 두면 어느 쪽이 걸릴지 알 수 없다.
    public func add(trigger: String, replacement: String) {
        let trigger = trigger.trimmingCharacters(in: .whitespacesAndNewlines)
        let replacement = replacement.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trigger.isEmpty, !replacement.isEmpty else { return }
        var kept = entries.filter { $0.trigger != trigger }
        kept.append(FfiShortcut(trigger: trigger, replacement: replacement))
        entries = kept.sorted { $0.trigger < $1.trigger }
    }

    public func remove(at offsets: IndexSet) {
        var kept = entries
        kept.remove(atOffsets: offsets)
        entries = kept
    }
}
