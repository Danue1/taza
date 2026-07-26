import SwiftUI

/// 설정을 바꾼 자리에서 바로 쳐 볼 수 있게 하는 칸 — 여러 줄까지 받는다.
struct KeyboardTestSection: View {
    /// 칸에 들어가는 순간 할 일 — 언어 화면은 이때 키보드의 언어를 맞춰 둔다
    var onFocus: (() -> Void)?

    @State private var text = ""
    @FocusState private var isFocused: Bool

    var body: some View {
        Section("키보드 테스트") {
            TextField("여기에 입력해 보세요", text: $text, axis: .vertical)
                .lineLimit(3...8)
                .focused($isFocused)
                .onChange(of: isFocused) { focused in
                    if focused { onFocus?() }
                }
                // 여러 줄을 받는 칸이라 리턴키가 줄바꿈이고, 목록도 짧아 끌어 내려
                // 닫을 여백이 없다 — 나가는 길을 키보드 위에 둔다.
                .toolbar {
                    ToolbarItemGroup(placement: .keyboard) {
                        Spacer()
                        Button("완료") { isFocused = false }
                    }
                }
        }
    }
}
