import SwiftUI

/// 모든 언어에 함께 걸리는 입력 보조. 언어 하나만 다르게 하고 싶으면 언어 → 그 언어
/// 화면에서 이 항목들을 하나씩 되짚어 정할 수 있다.
struct TypingSettingsList: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
        List {
            ForEach(TypingOptionGroup.allCases, id: \.self) { group in
                Section(group.title) {
                    ForEach(group.options, id: \.self) { option in
                        Toggle(isOn: model.commonValue(option)) {
                            OptionLabel(option: option)
                        }
                    }
                }
            }

            KeyboardTestSection()
        }
        .navigationTitle("입력 보조")
    }
}

/// 이름만으로 무엇이 달라지는지 알기 어려운 항목에는 한 줄을 붙인다.
///
/// 배지는 이름과 같은 줄에 둔다 — 라벨 덩어리 바깥에 붙이면 설명이 있는 항목에서
/// 설명 줄 끝까지 밀려나 어디에 걸린 배지인지 흐려진다.
struct OptionLabel: View {
    let option: TypingOption
    /// 이 항목이 공통 설정을 따르는 중임을 알리는 표식 — 언어별 화면에서만 쓴다
    var showsCommonBadge = false

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(alignment: .firstTextBaseline, spacing: 6) {
                Text(option.title)
                if showsCommonBadge {
                    Text("공통")
                        .font(.caption2.weight(.medium))
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(.tazaSelection, in: Capsule())
                        .foregroundStyle(.tazaAccent)
                }
            }
            if let explanation = option.explanation {
                Text(explanation)
                    .font(.footnote)
                    .foregroundStyle(.tazaSecondaryLabel)
            }
        }
    }
}
