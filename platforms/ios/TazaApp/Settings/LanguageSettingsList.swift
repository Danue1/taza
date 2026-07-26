import SwiftUI

/// 언어 하나에만 걸리는 설정 — 배열·사전처럼 그 언어에만 있는 것과, 공통값을 대신할 값.
struct LanguageSettingsList: View {
    @ObservedObject var model: SettingsModel
    @ObservedObject var packs: PackLibraryModel
    let language: TazaLanguage

    private var layouts: [String] { model.availableLayouts(language) }

    var body: some View {
        List {
            Section("배열") {
                // 배열이 한 벌뿐인 언어에서는 고를 것이 없으므로 이름만 보인다
                if layouts.count > 1 {
                    Picker("키 배열", selection: model.layoutBinding(for: language)) {
                        ForEach(layouts, id: \.self) { name in
                            Text(name).tag(name)
                        }
                    }
                } else {
                    LabeledContent("키 배열", value: model.layoutName(language))
                }
            }

            Section("사전") {
                HStack {
                    Text("\(model.displayName(language)) 사전")
                    Spacer()
                    PackStatusView(
                        name: model.displayName(language),
                        state: packs.states[language],
                        install: { Task { await packs.install(language); model.reloadLanguageInfo() } },
                        remove: { packs.remove(language); model.reloadLanguageInfo() }
                    )
                }
                if packs.sources[language]?.isEmpty == false {
                    NavigationLink("사전 출처") {
                        PackSourceList(model: model, packs: packs, language: language)
                    }
                }
            }

            // 항목은 공통 설정과 같은 모양(토글 한 줄)으로 서고, 이 언어에서 따로
            // 정한 것만 배지가 사라져 눈에 띈다
            ForEach(TypingOptionGroup.allCases, id: \.self) { group in
                Section {
                    ForEach(group.options, id: \.self) { option in
                        LanguageOptionRow(
                            option: option,
                            choice: model.choice(option, for: language),
                            effective: model.effectiveValue(option, for: language)
                        )
                    }
                } header: {
                    Text(group.title)
                } footer: {
                    if group.options.contains(where: { model.choice($0, for: language).wrappedValue != .common }) {
                        Text("옆으로 밀면 공통 설정으로 되돌립니다.")
                    }
                }
            }

            KeyboardTestSection { model.prepareKeyboard(for: language) }
        }
        .navigationTitle(LocalizedStringKey(model.displayName(language)))
    }
}

/// 항목 하나가 이 언어에서 어떻게 동작할지.
///
/// 갈래 셋을 늘어놓지 않는다. 보이는 것은 **지금 이 언어에서 실제로 걸리는 값** 하나이고,
/// 그것이 공통 설정에서 온 것이면 배지가 붙는다 — 대부분의 항목이 공통을 따르므로,
/// 세 갈래를 항목마다 펴면 화면이 같은 말을 여덟 번 되풀이한다.
///
/// 토글을 건드리면 이 언어의 값이 되고(배지가 사라진다), 옆으로 밀면 공통으로 돌아간다.
private struct LanguageOptionRow: View {
    let option: TypingOption
    @Binding var choice: SettingsModel.TypingChoice
    /// 지금 이 언어에 실제로 걸리는 값 — 공통을 따르는 중이면 공통값이다
    let effective: Bool

    private var followsCommon: Bool { choice == .common }

    var body: some View {
        Toggle(isOn: Binding(
            get: { effective },
            set: { choice = $0 ? .on : .off }
        )) {
            OptionLabel(option: option, showsCommonBadge: followsCommon)
        }
        .swipeActions(edge: .trailing) {
            if !followsCommon {
                Button("공통") { choice = .common }
                    .tint(.tazaAccent)
            }
        }
    }
}
