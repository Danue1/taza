import SwiftUI

/// 배열 고르기 — 이름만 늘어놓으면 "세벌식 최종"이 무엇인지 아는 사람만 고를 수 있다.
/// 배열마다 문자면을 그려 타일로 늘어놓아, 자기 손에 익은 글자 줄을 보고 고르게 한다.
struct LayoutTileGrid: View {
    let previews: [TazaLayoutPreview]
    @Binding var selection: String

    /// 폰 세로에서 두 벌씩 서고, 글자가 읽힐 만큼은 넓게 잡는다
    private let columns = [GridItem(.adaptive(minimum: 148), spacing: 12)]

    var body: some View {
        LazyVGrid(columns: columns, spacing: 12) {
            ForEach(previews) { preview in
                Button {
                    selection = preview.name
                } label: {
                    LayoutTile(preview: preview, isSelected: preview.name == selection)
                }
                .buttonStyle(.plain)
            }
        }
    }
}

private struct LayoutTile: View {
    let preview: TazaLayoutPreview
    let isSelected: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            LayoutPreviewView(preview)

            HStack(spacing: 4) {
                Text(preview.name)
                    .font(.subheadline)
                    .fontWeight(isSelected ? .semibold : .regular)
                    .lineLimit(1)
                Spacer(minLength: 0)
                if isSelected {
                    Image(systemName: "checkmark")
                        .font(.footnote.weight(.semibold))
                        .foregroundStyle(.tazaAccent)
                }
            }
        }
        .padding(9)
        .background(Color(uiColor: .secondarySystemGroupedBackground), in: shape)
        .overlay {
            shape.strokeBorder(isSelected ? .tazaAccent : .clear, lineWidth: 2)
        }
        .contentShape(shape)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(isSelected ? [.isButton, .isSelected] : .isButton)
    }

    private var shape: RoundedRectangle {
        RoundedRectangle(cornerRadius: 13, style: .continuous)
    }
}
