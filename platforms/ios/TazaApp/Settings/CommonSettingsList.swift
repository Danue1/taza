import SwiftUI

/// 설정의 첫 화면. 항목을 평평하게 늘어놓지 않고 갈래로 나눈다 — 갈래의 기준은 값이
/// 코어로 가는지 셸에 남는지(그것은 우리 사정이다)가 아니라, 사용자가 무엇을 고치러
/// 왔는가다.
struct CommonSettingsList: View {
    @ObservedObject var model: SettingsModel
    @ObservedObject var packs: PackLibraryModel

    var body: some View {
        List {
            // 키보드를 아직 켜지 않았으면 그 일이 먼저다 — 켠 뒤에는 사라진다
            if !model.isKeyboardEnabled {
                Section {
                    KeyboardSetupBanner()
                }
            }

            Section {
                NavigationLink {
                    LanguageListView(model: model, packs: packs)
                } label: {
                    LabeledContent("언어", value: "\(model.enabledLanguages.count)")
                }
            }

            Section {
                NavigationLink("입력 보조") { TypingSettingsList(model: model) }
                NavigationLink("표시") { DisplaySettingsList(model: model) }
                NavigationLink("입력감·제스처") { FeedbackSettingsList(model: model) }
                NavigationLink("개인 정보") { PrivacySettingsList(model: model) }
            }

            // 사전 출처는 사전마다 다르므로 그 언어 화면에 있다. 여기 남는 것은
            // 빌드 하나에 하나뿐인 고지다.
            Section {
                NavigationLink("소프트웨어 라이선스") {
                    SoftwareLicenseList()
                }
            }

            KeyboardTestSection()
        }
        .navigationTitle("설정")
    }
}
