import SwiftUI

/// 친 말을 다른 말로 바꾸는 표(순정의 "텍스트 대치").
///
/// 자동 수정과 달리 사람이 손수 적은 것이므로 사전 교정보다 세다 — 그 판단은 코어에 있고
/// 이 화면은 표를 갖고 있을 뿐이다.
struct ShortcutSettingsList: View {
    @ObservedObject var model: SettingsModel

    @State private var trigger = ""
    @State private var replacement = ""
    @FocusState private var editing: Bool

    var body: some View {
        List {
            Section {
                TextField("친 말", text: $trigger)
                    .autocorrectionDisabled()
                    .textInputAutocapitalization(.never)
                    .focused($editing)
                TextField("바뀔 말", text: $replacement)
                    .focused($editing)
                Button("추가") {
                    model.addShortcut(trigger: trigger, replacement: replacement)
                    trigger = ""
                    replacement = ""
                    editing = false
                }
                .disabled(trigger.isEmpty || replacement.isEmpty)
            } footer: {
                Text("친 말을 확정하는 순간 바뀔 말이 대신 들어갑니다. 바로 이어 삭제를 누르면 친 대로 되돌아옵니다.")
            }

            if !model.shortcuts.isEmpty {
                Section {
                    ForEach(model.shortcuts.indices, id: \.self) { index in
                        LabeledContent(
                            model.shortcuts[index].trigger,
                            value: model.shortcuts[index].replacement
                        )
                    }
                    .onDelete(perform: model.removeShortcuts)
                }
            }

            KeyboardTestSection()
        }
        .scrollDismissesKeyboard(.interactively)
        .navigationTitle("텍스트 대치")
    }
}
