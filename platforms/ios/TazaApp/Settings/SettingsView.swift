import SwiftUI

/// 키보드 설정 — 언어 키를 길게 눌러 "설정"을 고르면 이 화면이 열린다.
/// 키보드와 같은 디자인 토큰(TazaTheme)을 쓴다.
///
/// 순정 키보드 설정과 같은 결로 파고든다: 모든 언어에 함께 걸리는 값이 여기 있고,
/// 언어 하나의 사정은 언어 → 목록 → 그 언어 화면으로 들어가 만진다.
struct SettingsView: View {
    @StateObject private var model = SettingsModel()
    @StateObject private var packs = PackLibraryModel()

    @Environment(\.scenePhase) private var scenePhase

    var body: some View {
        NavigationStack {
            CommonSettingsList(model: model, packs: packs)
        }
        .tint(.tazaAccent)
        .task { await packs.refresh() }
        // 설정에서 키보드를 켜고 돌아온 순간을 잡는다
        .onChange(of: scenePhase) { phase in
            if phase == .active {
                model.refreshKeyboardActivation()
            }
        }
    }
}
