import SwiftUI

/// 순정 키보드에서는 설정 → 일반 → 키보드에 있는 항목들. 서드파티 키보드는 그 값을
/// 읽을 수 없으므로 우리가 직접 갖고 키보드 세션에 넘긴다.
struct CommonSettingsList: View {
    @ObservedObject var model: SettingsModel
    @ObservedObject var packs: PackLibraryModel

    @State private var confirmingReset = false

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

            Section("입력") {
                ForEach(TypingOption.allCases, id: \.self) { option in
                    Toggle(option.title, isOn: model.commonValue(option))
                }
                Button("입력 학습 재설정", role: .destructive) { confirmingReset = true }
            }

            KeyboardTestSection()

            // 사전 원천의 라이선스가 요구하는 저작자 표시 — 팩 메타데이터에서 읽는다
            if !packs.attributions.isEmpty {
                Section("사전 출처") {
                    ForEach(packs.attributions, id: \.self) { attribution in
                        Text(attribution)
                            .font(.footnote)
                            .foregroundStyle(.tazaSecondaryLabel)
                    }
                }
            }
        }
        .alert("배운 단어를 모두 지울까요?", isPresented: $confirmingReset) {
            Button("취소", role: .cancel) {}
            Button("재설정", role: .destructive) { model.resetLearning() }
        }
    }
}
