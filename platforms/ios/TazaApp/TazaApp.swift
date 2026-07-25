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
    @State private var descriptors: [String: FfiLanguageDescriptor] = [:]
    @State private var testText = ""

    var body: some View {
        NavigationStack {
            List {
                Section("사용 언어") {
                    ForEach(enabledLanguages, id: \.tag) { language in
                        LanguageRow(
                            language: language,
                            descriptor: descriptors[language.tag],
                            isLastUsed: language == preferences.lastUsedLanguage,
                            packState: packs.states[language],
                            install: { Task { await packs.install(language) } },
                            remove: { packs.remove(language) }
                        )
                    }
                    .onMove(perform: moveLanguage)
                }

                TypingSection()

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
        .onAppear {
            enabledLanguages = preferences.enabledLanguages
            let store = PackStore()
            descriptors = Dictionary(
                uniqueKeysWithValues: TazaLanguage.all.compactMap { language in
                    tazaLanguageDescriptor(for: language, packURL: store.packURL(for: language))
                        .map { (language.tag, $0) }
                }
            )
        }
        .task { await packs.refresh() }
    }

    /// 목록 순서가 곧 언어 키를 탭했을 때의 순환 순서다
    private func moveLanguage(from source: IndexSet, to destination: Int) {
        enabledLanguages.move(fromOffsets: source, toOffset: destination)
        preferences.enabledLanguages = enabledLanguages
    }
}

/// 순정 키보드에서는 설정 → 일반 → 키보드에 있는 항목들. 서드파티 키보드는 그 값을
/// 읽을 수 없으므로 우리가 직접 갖고 키보드 세션에 넘긴다.
private struct TypingSection: View {
    private let typing = TypingPreferences()
    private let learning = LearningStore()

    @State private var autoCorrection = true
    @State private var predictions = true
    @State private var doubleSpacePeriod = true
    @State private var personalizedLearning = true
    @State private var confirmingReset = false

    var body: some View {
        Section("입력") {
            // 항목 이름은 순정 키보드 표기를 따른다 (설정 → 일반 → 키보드)
            Toggle("자동 수정", isOn: $autoCorrection)
            Toggle("자동 완성", isOn: $predictions)
            Toggle("\".\" 단축키", isOn: $doubleSpacePeriod)
            Toggle("입력 학습", isOn: $personalizedLearning)
            Button("입력 학습 재설정", role: .destructive) { confirmingReset = true }
        }
        .confirmationDialog(
            "배운 단어를 모두 지울까요?",
            isPresented: $confirmingReset,
            titleVisibility: .visible
        ) {
            Button("재설정", role: .destructive) { learning.removeAll() }
            Button("취소", role: .cancel) {}
        }
        .onAppear {
            autoCorrection = typing.autoCorrection
            predictions = typing.predictions
            doubleSpacePeriod = typing.doubleSpacePeriod
            personalizedLearning = typing.personalizedLearning
        }
        .onChange(of: autoCorrection) { typing.autoCorrection = $0 }
        .onChange(of: predictions) { typing.predictions = $0 }
        .onChange(of: doubleSpacePeriod) { typing.doubleSpacePeriod = $0 }
        .onChange(of: personalizedLearning) { typing.personalizedLearning = $0 }
    }
}

private struct LanguageRow: View {
    let language: TazaLanguage
    /// 표시 이름·키캡 표기·배열 이름은 코어가 팩 선언에서 읽어 준다
    let descriptor: FfiLanguageDescriptor?
    let isLastUsed: Bool
    let packState: PackLibraryModel.State?
    let install: () -> Void
    let remove: () -> Void

    var body: some View {
        HStack {
            Text(descriptor?.keycapLabel ?? "")
                .font(.system(size: 15, weight: .semibold))
                .frame(width: 28, height: 28)
                .background(.tazaSelection, in: RoundedRectangle(cornerRadius: 6))
            VStack(alignment: .leading, spacing: 2) {
                Text(descriptor?.displayName ?? language.tag)
                    .foregroundStyle(.tazaLabel)
                Text(descriptor?.layoutName ?? "")
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
                .accessibilityLabel("\(descriptor?.displayName ?? language.tag) 사전 삭제")
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
            .accessibilityLabel("\(descriptor?.displayName ?? language.tag) 사전 다운로드")
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
