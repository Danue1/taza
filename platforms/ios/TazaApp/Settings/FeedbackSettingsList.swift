import SwiftUI

/// 키를 눌렀을 때의 소리·느낌과, 누르고 있을 때 벌어지는 일. 대부분 코어에 판단할 것이
/// 없어 익스텐션이 직접 읽는다 — 커서 감도만 코어가 이동 칸수를 계산하므로 주입된다.
struct FeedbackSettingsList: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
        List {
            Section("누를 때") {
                Toggle("키 사운드", isOn: model.keyboardBinding(\.keySound))
                Toggle("키 확대 미리보기", isOn: model.keyboardBinding(\.keyPreview))
            }

            Section("길게 누를 때") {
                Toggle("길게 눌러 변형 문자", isOn: model.keyboardBinding(\.keyAlternates))
                SegmentedSetting(
                    title: "삭제 반복 속도",
                    selection: model.keyboardBinding(\.backspaceSpeed),
                    options: BackspaceSpeed.allCases,
                    label: \.title
                )
                SegmentedSetting(
                    title: "커서 이동 감도",
                    selection: model.keyboardBinding(\.cursorSensitivity),
                    options: CursorSensitivityChoice.allCases,
                    label: \.title
                )
            }

            Section {
                Toggle("shift 두 번 눌러 고정", isOn: model.keyboardBinding(\.shiftDoubleTapLock))
                Toggle(isOn: model.keyboardBinding(\.spaceSwipeLanguage)) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("스페이스 밀어 언어 전환")
                        Text("길게 눌러 커서를 옮기는 동작과 뜻이 겹칠 수 있습니다.")
                            .font(.footnote)
                            .foregroundStyle(.tazaSecondaryLabel)
                    }
                }
            }

            KeyboardTestSection()
        }
        .navigationTitle("입력감·제스처")
    }
}

extension BackspaceSpeed {
    var title: LocalizedStringKey {
        switch self {
        case .slow: "느리게"
        case .standard: "기본"
        case .fast: "빠르게"
        }
    }
}

extension CursorSensitivityChoice {
    var title: LocalizedStringKey {
        switch self {
        case .low: "낮게"
        case .standard: "기본"
        case .high: "높게"
        }
    }
}
