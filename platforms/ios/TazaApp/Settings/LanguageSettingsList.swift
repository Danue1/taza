import SwiftUI

/// 언어 하나에만 걸리는 설정 — 배열·언어팩처럼 그 언어에만 있는 것과, 공통값을 대신할 값.
struct LanguageSettingsList: View {
    @ObservedObject var model: SettingsModel
    @ObservedObject var packs: PackLibraryModel
    let language: TazaLanguage

    private var layouts: [String] { model.availableLayouts(language) }

    /// 지금 고른 배열의 그림
    private var selectedPreview: TazaLayoutPreview? {
        let name = model.layoutName(language)
        return model.layoutPreviews(language).first { $0.name == name }
    }

    /// 이 언어의 언어팩이 아직 기기에 없는가. 받을 수 없는 상태(배포처 미설정)도
    /// 마찬가지로 "아직 없다"이므로 같이 묶는다.
    private var needsPack: Bool {
        switch packs.states[language] {
        case .notInstalled, .installing, .unavailable: true
        default: false
        }
    }

    var body: some View {
        List {
            Section {
                // 고르는 일은 자기 화면에서 하고, 여기에는 지금 무엇을 치고 있는지만
                // 남긴다 — 이름만으로는 알기 어려우므로 그림을 함께 둔다
                if layouts.count > 1 {
                    NavigationLink {
                        LayoutSelectionList(model: model, language: language)
                    } label: {
                        LabeledContent("키 배열", value: model.layoutName(language))
                    }
                } else {
                    LabeledContent("키 배열", value: model.layoutName(language))
                }
                if let preview = selectedPreview {
                    LayoutPreviewView(preview)
                        .listRowInsets(EdgeInsets(top: 6, leading: 12, bottom: 12, trailing: 12))
                }
            } header: {
                Text("배열")
            } footer: {
                // 배열은 언어팩에 함께 실려 온다 — 하나뿐인 까닭이 "이 언어에는 배열이
                // 하나"인지 "아직 안 받았다"인지를 사용자가 알 수 있어야 한다
                if layouts.count <= 1, needsPack {
                    Text("언어팩을 받으면 이 언어의 다른 배열도 고를 수 있습니다.")
                }
            }

            Section {
                HStack {
                    Text(model.languageName(language))
                    Spacer()
                    PackStatusView(
                        name: model.languageName(language),
                        state: packs.states[language],
                        install: { Task { await packs.install(language); model.reloadLanguageInfo() } },
                        remove: { packs.remove(language); model.reloadLanguageInfo() }
                    )
                }
                if packs.sources[language]?.isEmpty == false {
                    NavigationLink("언어팩 출처") {
                        PackSourceList(model: model, packs: packs, language: language)
                    }
                }
            } header: {
                // 내려받는 단위는 사전이 아니다 — 언어 선언·키 배열·어휘·언어모델·
                // 곁들일 것이 한 파일에 함께 실려 온다. "사전"이라 적으면 배열이 왜
                // 여기에 딸려 오는지 설명할 길이 없고, 순정의 "사전"(뜻풀이)과도 겹친다.
                Text("언어팩")
            } footer: {
                // 받을 수 없는 까닭은 경고 기호만으로 알 수 없다 — 배포처가 없는 것인지
                // 받다가 끊긴 것인지에 따라 사용자가 할 일이 다르다
                if case .unavailable(let message) = packs.states[language] {
                    Text(message)
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
        .scrollDismissesKeyboard(.interactively)
        .navigationTitle(model.languageName(language))
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
