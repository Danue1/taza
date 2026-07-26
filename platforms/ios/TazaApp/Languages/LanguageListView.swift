import SwiftUI

/// 쓰는 언어의 목록. 순서가 곧 언어 키를 탭했을 때의 순환 순서이고, 행을 누르면 그
/// 언어의 설정으로 들어간다. 왼쪽으로 밀면 그 자리에서 지우고, 순서는 순정 키보드
/// 목록과 같이 편집 상태에서 바꾼다.
struct LanguageListView: View {
    @ObservedObject var model: SettingsModel
    @ObservedObject var packs: PackLibraryModel

    @Environment(\.editMode) private var editMode
    @State private var openedLanguage: TazaLanguage?

    private var isEditing: Bool { editMode?.wrappedValue.isEditing ?? false }

    var body: some View {
        List {
            Section("사용 언어") {
                ForEach(model.enabledLanguages, id: \.tag) { language in
                    Button {
                        // 편집 상태에서는 행을 눌러도 들어가지 않는다 — 비활성화하면
                        // 이름까지 흐려지므로 여기서 막는다(순정은 이름이 그대로다).
                        guard !isEditing else { return }
                        openedLanguage = language
                    } label: {
                        LanguageRow(
                            name: model.displayName(language),
                            showsDisclosure: !isEditing
                        )
                    }
                    .buttonStyle(.plain)
                }
                .onDelete(perform: disableLanguages)
                .onMove(perform: model.moveLanguages)
                // 언어가 하나뿐이면 지울 수 없다 — 키보드가 그릴 배열이 남지 않는다
                .deleteDisabled(!model.canDisableLanguage)
                // 편집 상태가 아니면 끌어 옮길 수 없다
                .moveDisabled(!isEditing)
            }

            if !model.availableLanguages.isEmpty {
                Section("추가할 수 있는 언어") {
                    ForEach(model.availableLanguages, id: \.tag) { language in
                        Button {
                            model.enable(language)
                        } label: {
                            LanguageRow(
                                name: model.displayName(language),
                                accessory: Image(systemName: "plus.circle")
                            )
                        }
                        .buttonStyle(.borderless)
                        .foregroundStyle(.tazaLabel)
                    }
                }
            }
        }
        .animation(.default, value: isEditing)
        .navigationTitle("언어")
        .toolbar { EditButton() }
        .navigationDestination(
            isPresented: Binding(
                get: { openedLanguage != nil },
                set: { if !$0 { openedLanguage = nil } }
            )
        ) {
            if let openedLanguage {
                LanguageSettingsList(model: model, packs: packs, language: openedLanguage)
            }
        }
    }

    private func disableLanguages(_ offsets: IndexSet) {
        let languages = model.enabledLanguages
        for offset in offsets {
            model.disable(languages[offset])
        }
    }
}
