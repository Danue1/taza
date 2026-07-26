import UIKit

/// 언어마다 하나씩 두는 코어 세션의 목록과 그중 지금 쓰는 하나.
/// 설정 앱에서 언어를 더하거나 지운 결과가 여기로 흘러든다.
extension KeyboardViewController {
    var activeSession: KeyboardSession? {
        sessions[currentLanguage]
    }

    /// 코어 빌드에 포함되지 않은 언어는 nil — 해당 언어는 전환 대상에서 빠진다.
    private func makeSession(language: TazaLanguage) -> KeyboardSession? {
        guard let session = try? KeyboardSession(languageTag: language.tag) else {
            return nil
        }
        // 내려받아 설치된 팩이 있으면 그쪽을, 없으면 내장 팩을 mmap한다.
        // 사전이 아직 없는 언어는 팩 없이도 동작한다(제안·자동교정만 비활성).
        let store = PackStore(bundle: Bundle(for: Self.self))
        if let packURL = store.packURL(for: language) {
            try? session.loadPack(path: packURL.path)
        }
        return session
    }

    /// 설정 앱에서 언어를 더하거나 지운 결과를 세션 목록에 옮긴다. 지운 언어의 학습
    /// 스냅샷은 남겨 둔다 — 다시 추가하면 배운 말이 그대로 돌아온다.
    func syncSessions() {
        let languages = preferences.enabledLanguages
        for language in languages where sessions[language] == nil {
            sessions[language] = makeSession(language: language)
        }
        for (language, session) in sessions where !languages.contains(language) {
            learning.save(session.personalizationSnapshot(), for: language)
            sessions.removeValue(forKey: language)
        }
        if sessions[currentLanguage] == nil {
            currentLanguage = languages.first { sessions[$0] != nil } ?? currentLanguage
        }
    }

    func switchLanguage(to language: TazaLanguage) {
        guard language != currentLanguage, sessions[language] != nil else { return }
        // 전환 전에 진행 중 composing을 현재 언어 규칙으로 확정한다
        if let session = activeSession {
            apply(effects: session.handleEvent(event: .focusLost, context: currentContext()))
        }
        currentLanguage = language
        preferences.lastUsedLanguage = language
        candidateBar.setCandidates([])
        refreshFrame()
    }
}
