import SwiftUI

struct LanguageRow: View {
    let name: String
    /// 눌러 들어갈 수 있음을 알리는 셰브런 — 편집 상태에서는 물러난다
    var showsDisclosure = false
    /// 행 오른쪽에 붙는 표시 — 추가할 수 있는 언어는 이것으로 탭할 수 있음을 알린다
    var accessory: Image?

    var body: some View {
        HStack {
            Text(name)
                .lineLimit(1)
                .foregroundStyle(.tazaLabel)
            Spacer(minLength: 8)
            if let accessory {
                accessory.foregroundStyle(.tazaAccent)
            }
            if showsDisclosure {
                Image(systemName: "chevron.right")
                    .font(.footnote.weight(.semibold))
                    .foregroundStyle(Color(uiColor: .tertiaryLabel))
                    .transition(.opacity)
            }
        }
        .contentShape(Rectangle())
        .accessibilityElement(children: .combine)
    }
}
