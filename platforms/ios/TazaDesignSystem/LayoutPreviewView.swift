import SwiftUI

/// 배열 하나를 실제 키보드 모양 그대로 줄여 그린다.
///
/// 그리는 일은 키보드가 쓰는 판(`KeyboardGridView`)에 그대로 맡기고, 이 뷰는 배율만
/// 정한다 — 미리보기가 자기 그리기를 따로 두면 키캡 그림자·라벨 위치·스페이스 표기
/// 같은 것이 실제 키보드와 소리 없이 갈라진다. 손은 받지 않으므로 판의 터치는 끈다.
public struct LayoutPreviewView: View {
    private let preview: TazaLayoutPreview

    public init(_ preview: TazaLayoutPreview) {
        self.preview = preview
    }

    public var body: some View {
        GeometryReader { proxy in
            KeyboardGrid(
                model: preview.frame,
                contentScale: proxy.size.width / preview.referenceWidth,
                showsKeyBorders: preview.showsKeyBorders
            )
        }
        .aspectRatio(preview.aspectRatio, contentMode: .fit)
        // 문자 키는 흰색이므로 판에도 바탕이 있어야 키 경계가 보인다 — 실제 키보드가
        // 시스템에서 받는 바탕과 같은 자리다
        .padding(5)
        .background(
            Color(uiColor: .systemGray5),
            in: RoundedRectangle(cornerRadius: 8, style: .continuous)
        )
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text(preview.name))
    }
}

private struct KeyboardGrid: UIViewRepresentable {
    let model: KeyboardFrameModel
    let contentScale: CGFloat
    let showsKeyBorders: Bool

    func makeUIView(context: Context) -> KeyboardGridView {
        let view = KeyboardGridView()
        // 그림일 뿐이므로 손도 VoiceOver도 판 안으로 들이지 않는다 — 키 서른 개가
        // 배열을 고르는 자리에서 하나씩 읽히면 목록을 지나갈 수 없다
        view.isUserInteractionEnabled = false
        view.accessibilityElementsHidden = true
        return view
    }

    func updateUIView(_ view: KeyboardGridView, context: Context) {
        view.contentScale = contentScale
        view.showsKeyBorders = showsKeyBorders
        view.setFrame(model)
    }
}
