import SwiftUI

/// 설정을 바꾼 자리에서 바로 쳐 볼 수 있게 하는 칸 — 여러 줄까지 받는다.
///
/// 키보드는 순정 설정 앱처럼 목록을 쓸어내려 닫는다 — 이 칸을 싣는 목록은
/// `scrollDismissesKeyboard(.interactively)`를 건다(수정자가 스크롤 뷰에 걸려야 해
/// 칸 안에서는 걸 수 없다).
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
        }
    }
}
