import Foundation

/// 개인화(학습) 스냅샷이 놓이는 자리. 익스텐션 프로세스는 키보드를 띄울 때마다 다시
/// 만들어지므로, 이 왕복이 없으면 배운 말이 한 번의 표시를 넘기지 못한다.
///
/// 설정 앱은 같은 자리를 지워 "입력 학습 재설정"을 구현한다 — 순정 키보드의 "키보드
/// 사전 재설정"에 해당한다.
///
/// App Group을 못 쓰는 상황(서명 팀 미설정·시뮬레이터)에서는 각자의 저장소로 물러난다.
/// 학습 자체는 그대로 이어지지만 앱에서 누른 재설정이 익스텐션에 닿지 않는다 —
/// 언어·타이핑 설정이 공유되지 않는 것과 같은 한계다.
public struct LearningStore {
    private static let directoryName = "Learning"

    private let directory: URL?

    public init(appGroupIdentifier: String = LanguagePreferences.appGroupIdentifier) {
        let container = FileManager.default
            .containerURL(forSecurityApplicationGroupIdentifier: appGroupIdentifier)
            ?? FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
        self.directory = container?.appendingPathComponent(Self.directoryName, isDirectory: true)
    }

    /// 저장된 것이 없으면 빈 배열 — 호출자는 이것을 "배운 것이 없음"으로 다룬다.
    public func snapshot(for language: TazaLanguage) -> [String] {
        guard let url = url(for: language),
              let text = try? String(contentsOf: url, encoding: .utf8)
        else {
            return []
        }
        return text.split(separator: "\n", omittingEmptySubsequences: true).map(String.init)
    }

    public func save(_ snapshot: [String], for language: TazaLanguage) {
        guard let directory, let url = url(for: language) else { return }
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try? snapshot.joined(separator: "\n").write(to: url, atomically: true, encoding: .utf8)
    }

    /// 모든 언어의 학습을 지운다.
    public func removeAll() {
        guard let directory else { return }
        try? FileManager.default.removeItem(at: directory)
    }

    private func url(for language: TazaLanguage) -> URL? {
        directory?.appendingPathComponent("\(language.tag).learning")
    }
}
