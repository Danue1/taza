import SwiftUI

/// 언어 하나에만 걸리는 설정 — 배열·사전처럼 그 언어에만 있는 것과, 공통값을 대신할 값.
struct LanguageSettingsList: View {
    @ObservedObject var model: SettingsModel
    @ObservedObject var packs: PackLibraryModel
    let language: TazaLanguage

    var body: some View {
        List {
            Section("배열") {
                LabeledContent("키 배열", value: model.layoutName(language))
            }

            Section("사전") {
                HStack {
                    Text("\(model.displayName(language)) 사전")
                    Spacer()
                    PackStatusView(
                        name: model.displayName(language),
                        state: packs.states[language],
                        install: { Task { await packs.install(language) } },
                        remove: { packs.remove(language) }
                    )
                }
            }

            // 항목마다 공통값을 따를지 이 언어에서 정할지를 그 자리에서 고른다
            Section("입력") {
                ForEach(TypingOption.allCases, id: \.self) { option in
                    Picker(option.title, selection: model.choice(option, for: language)) {
                        Text("공통 · \(model.commonValueDescription(option))")
                            .tag(SettingsModel.TypingChoice.common)
                        Text("켬").tag(SettingsModel.TypingChoice.on)
                        Text("끔").tag(SettingsModel.TypingChoice.off)
                    }
                }
            }

            KeyboardTestSection { model.prepareKeyboard(for: language) }
        }
        .navigationTitle(LocalizedStringKey(model.displayName(language)))
    }
}
