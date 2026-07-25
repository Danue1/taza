import SwiftUI

@main
struct TazaApp: App {
    var body: some Scene {
        WindowGroup {
            SettingsView()
        }
    }
}

/// 키보드 설정 — 언어 키를 길게 눌러 "설정"을 고르면 이 화면이 열린다.
/// 키보드와 같은 디자인 토큰(TazaTheme)을 쓴다.
struct SettingsView: View {
    private let preferences = LanguagePreferences()

    @StateObject private var packs = PackLibraryModel()
    @State private var enabledLanguages: [TazaLanguage] = []
    @State private var testText = ""

    var body: some View {
        NavigationStack {
            List {
                Section("사용 언어") {
                    ForEach(enabledLanguages, id: \.rawValue) { language in
                        LanguageRow(
                            language: language,
                            isLastUsed: language == preferences.lastUsedLanguage,
                            packState: packs.states[language],
                            install: { Task { await packs.install(language) } },
                            remove: { packs.remove(language) }
                        )
                    }
                    .onMove(perform: moveLanguage)
                }

                Section("키보드 테스트") {
                    TextField("여기에 입력해 보세요", text: $testText)
                }

                Section {
                    Text("설정 → 일반 → 키보드 → 키보드에서 Taza Keyboard를 추가하세요.")
                        .font(.footnote)
                        .foregroundStyle(.tazaSecondaryLabel)
                }

                // 사전 원천의 라이선스가 요구하는 저작자 표시 — 팩 메타데이터에서 읽는다
                if !packs.attributions.isEmpty {
                    Section("사전 출처") {
                        ForEach(packs.attributions, id: \.self) { attribution in
                            Text(attribution)
                                .font(.footnote)
                                .foregroundStyle(.tazaSecondaryLabel)
                        }
                    }
                }
            }
            .navigationTitle("Taza")
            .toolbar { EditButton() }
        }
        .tint(.tazaAccent)
        .onAppear { enabledLanguages = preferences.enabledLanguages }
        .task { await packs.refresh() }
    }

    /// 목록 순서가 곧 언어 키를 탭했을 때의 순환 순서다
    private func moveLanguage(from source: IndexSet, to destination: Int) {
        enabledLanguages.move(fromOffsets: source, toOffset: destination)
        preferences.enabledLanguages = enabledLanguages
    }
}

private struct LanguageRow: View {
    let language: TazaLanguage
    let isLastUsed: Bool
    let packState: PackLibraryModel.State?
    let install: () -> Void
    let remove: () -> Void

    var body: some View {
        HStack {
            Text(language.keycapLabel)
                .font(.system(size: 15, weight: .semibold))
                .frame(width: 28, height: 28)
                .background(.tazaSelection, in: RoundedRectangle(cornerRadius: 6))
            VStack(alignment: .leading, spacing: 2) {
                Text(language.displayName)
                    .foregroundStyle(.tazaLabel)
                Text(language.layoutName)
                    .font(.footnote)
                    .foregroundStyle(.tazaSecondaryLabel)
            }
            Spacer()
            if isLastUsed {
                Text("사용 중")
                    .font(.footnote)
                    .foregroundStyle(.tazaSecondaryLabel)
            }
            packStatus
        }
        .accessibilityElement(children: .combine)
    }

    /// 사전 상태는 순정 키보드의 언어 목록처럼 행 오른쪽에 붙는다 — 내장 언어는
    /// 내려받을 것이 없으므로 조작이 보이지 않는다.
    @ViewBuilder
    private var packStatus: some View {
        switch packState {
        case .bundled:
            Text("내장")
                .font(.footnote)
                .foregroundStyle(.tazaSecondaryLabel)
        case .installed(_, let updateAvailable):
            if updateAvailable {
                Button("갱신", action: install)
                    .font(.footnote)
            } else {
                Button(action: remove) {
                    Image(systemName: "trash")
                }
                .accessibilityLabel("\(language.displayName) 사전 삭제")
            }
        case .notInstalled(let size):
            Button(action: install) {
                HStack(spacing: 4) {
                    Image(systemName: "arrow.down.circle")
                    if size > 0 {
                        Text(
                            ByteCountFormatter.string(
                                fromByteCount: Int64(size),
                                countStyle: .file
                            )
                        )
                        .font(.footnote)
                    }
                }
            }
            .accessibilityLabel("\(language.displayName) 사전 다운로드")
        case .installing:
            ProgressView()
        case .unavailable(let message):
            Image(systemName: "exclamationmark.triangle")
                .foregroundStyle(.tazaSecondaryLabel)
                .accessibilityLabel(message)
        case .none:
            EmptyView()
        }
    }
}
