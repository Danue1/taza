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
        applyLayoutChoice(to: session, for: language)
        return session
    }

    /// 설정에서 고른 배열을 세션에 넣는다. 고른 적이 없거나 팩 갱신으로 그 배열이
    /// 사라졌으면 코어가 기본 배열에 머문다 — 셸이 되돌릴 것이 없다.
    func applyLayoutChoice(to session: KeyboardSession, for language: TazaLanguage) {
        guard let name = preferences.layoutName(for: language) else { return }
        _ = session.selectLayout(name: name)
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

    /// 라틴 글자를 넣는 칸(이메일·URL·비밀번호)에서 라틴 배열로 연다. 어느 언어가
    /// 라틴인지는 코어가 팩 선언으로 알려 주므로(조합 골격) 셸은 목록에서 고르기만 한다.
    func switchToLatinLanguage() {
        guard let current = sessions[currentLanguage], !isLatin(current) else { return }
        guard let latin = preferences.enabledLanguages.first(where: { language in
            sessions[language].map(isLatin) == true
        }) else {
            return
        }
        switchLanguage(to: latin)
    }

    /// 한글처럼 조합이 필요한 배열인지 — 코어가 밝힌 배열의 글자로 가른다.
    private func isLatin(_ session: KeyboardSession) -> Bool {
        session.keyboardFrame().rows
            .flatMap { $0 }
            .contains { key in
                key.role == .character && key.label.unicodeScalars.allSatisfy { scalar in
                    ("a"..."z").contains(String(scalar)) || ("A"..."Z").contains(String(scalar))
                }
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
        // 새 언어의 세션은 자기 shift 상태를 갖는다 — 지금 문맥에 맞춰 준다
        refreshAutoShift()
    }
}
