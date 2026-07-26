import UIKit

/// 언어 키를 길게 눌렀을 때 서는 메뉴 — 순정 지구본 메뉴처럼 언어를 세우고 설정을
/// 맨 아래에 둔다.
extension KeyboardViewController {
    func showLanguageMenu(near point: CGPoint) {
        let languages = preferences.enabledLanguages.filter { sessions[$0] != nil }
        // 표시 이름·배열 이름·키캡 표기는 각 언어의 코어 세션이 팩 선언에서 읽어 알려 준다.
        var items = languages.map { language in
            let descriptor = sessions[language]?.language()
            return PopupMenuView.Item(
                title: descriptor?.displayName ?? language.tag,
                detail: descriptor?.layoutName ?? "",
                badge: descriptor?.keycapLabel ?? "",
                isSelected: language == currentLanguage
            )
        }
        items.append(
            PopupMenuView.Item(
                title: NSLocalizedString("키보드 설정", comment: "언어 메뉴에서 설정 앱 열기"),
                accessibilityHint: NSLocalizedString(
                    "키보드 설정 앱을 엽니다",
                    comment: "언어 메뉴에서 설정 앱 열기"
                )
            )
        )

        let menu = PopupMenuView(items: items)
        menu.onSelect = { [weak self] index in
            guard let self else { return }
            dismissLanguageMenu()
            if index < languages.count {
                switchLanguage(to: languages[index])
            } else {
                openSettingsApplication()
            }
        }
        menu.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(menu)

        let anchorX = point.x * view.bounds.width
        let leading = min(
            max(anchorX - TazaTheme.Popup.width / 2, 6),
            max(view.bounds.width - TazaTheme.Popup.width - 6, 6)
        )
        // 메뉴 하단을 눌린 키의 윗변에 맞춘다 — 셸이 행 높이를 따로 알 필요가 없다
        let keyTop = gridView.frame.minY + (gridView.keyFrame(at: point)?.minY ?? 0)
        NSLayoutConstraint.activate([
            menu.bottomAnchor.constraint(
                equalTo: view.bottomAnchor,
                constant: keyTop - view.bounds.height
            ),
            menu.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: leading),
        ])
        languageMenu = menu
        UIAccessibility.post(notification: .layoutChanged, argument: menu)
    }

    func dismissLanguageMenu() {
        languageMenu?.removeFromSuperview()
        languageMenu = nil
    }

    /// 익스텐션은 UIApplication에 직접 닿을 수 없어 응답자 사슬을 타고 open을 부른다.
    private func openSettingsApplication() {
        var responder: UIResponder? = self
        while let current = responder {
            if let application = current as? UIApplication {
                application.open(TazaDeepLink.settings)
                return
            }
            responder = current.next
        }
    }
}
