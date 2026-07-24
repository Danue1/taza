import SwiftUI

@main
struct TazaApp: App {
    @State private var testText = ""
    @FocusState private var testFieldFocused: Bool

    var body: some Scene {
        WindowGroup {
            VStack(spacing: 16) {
                Text("Taza Keyboard")
                    .font(.title)
                Text("설정 → 일반 → 키보드 → 키보드에서 Taza Keyboard를 추가하세요.")
                    .multilineTextAlignment(.center)
                    .padding(.horizontal)
                TextField("키보드 테스트 입력란", text: $testText)
                    .textFieldStyle(.roundedBorder)
                    .padding(.horizontal)
                    .focused($testFieldFocused)
                    .onAppear {
                        DispatchQueue.main.asyncAfter(deadline: .now() + 1) {
                            testFieldFocused = true
                        }
                    }
            }
        }
    }
}
