import SwiftUI

/// 키보드가 얼마나 자리를 차지하고 어떻게 보이는가. 치수는 코어가 정하므로 여기서
/// 고르는 것은 코어에 넘길 갈래이고, 색과 테두리는 셸의 디자인 시스템이 받는다.
struct DisplaySettingsList: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
        List {
            Section("크기") {
                SegmentedSetting(
                    title: "키보드 높이",
                    selection: model.keyboardBinding(\.keyboardHeight),
                    options: KeyboardHeightChoice.allCases,
                    label: \.title
                )

                Toggle("숫자 행", isOn: model.keyboardBinding(\.numberRow))
                Toggle(isOn: model.keyboardBinding(\.candidateBarAlways)) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("후보 바 항상 표시")
                        Text("후보를 내지 않는 칸에서도 자리를 남겨 높이가 변하지 않게 합니다.")
                            .font(.footnote)
                            .foregroundStyle(.tazaSecondaryLabel)
                    }
                }
            }

            Section("모양") {
                Picker("테마", selection: model.keyboardBinding(\.theme)) {
                    ForEach(ThemeChoice.allCases, id: \.self) { choice in
                        Text(choice.title).tag(choice)
                    }
                }
                Toggle("키 테두리", isOn: model.keyboardBinding(\.keyBorders))
            }

            KeyboardTestSection()
        }
        .scrollDismissesKeyboard(.interactively)
        .navigationTitle("표시")
    }
}

extension KeyboardHeightChoice {
    var title: LocalizedStringKey {
        switch self {
        case .compact: "낮게"
        case .standard: "기본"
        case .tall: "높게"
        }
    }
}

extension ThemeChoice {
    var title: LocalizedStringKey {
        switch self {
        case .system: "시스템"
        case .light: "밝게"
        case .dark: "어둡게"
        }
    }
}
