import SwiftUI

/// 키보드가 기기에 남기는 것과 그것을 지우는 길.
///
/// 지우기 버튼만 두지 않는다: 무엇이 얼마나 남아 있는지 먼저 보인다. 숫자가 없으면
/// 사용자는 "지운다"가 무엇을 지우는 일인지 모른 채 눌러야 한다.
struct PrivacySettingsList: View {
    @ObservedObject var model: SettingsModel

    @State private var confirmingLearningReset = false
    @State private var confirmingRecentReset = false

    private var stored: [(language: TazaLanguage, summary: FfiPersonalizationSummary)] {
        model.enabledLanguages.compactMap { language in
            let summary = model.personalizationSummary(language)
            return summary.isEmpty ? nil : (language, summary)
        }
    }

    private var hasLearnedWords: Bool { stored.contains { $0.summary.learnedWords > 0 } }
    private var hasRecentAnnotations: Bool { stored.contains { $0.summary.recentAnnotations > 0 } }

    var body: some View {
        List {
            Section {
                if stored.isEmpty {
                    Text("아직 이 기기에 남은 것이 없습니다.")
                        .foregroundStyle(.tazaSecondaryLabel)
                }
                ForEach(stored, id: \.language.tag) { entry in
                    VStack(alignment: .leading, spacing: 2) {
                        Text(model.languageName(entry.language))
                        Text(detail(entry.summary))
                            .font(.footnote)
                            .foregroundStyle(.tazaSecondaryLabel)
                    }
                }
            } header: {
                Text("이 기기에 남은 것")
            } footer: {
                Text("배운 말과 최근에 고른 이모지는 이 기기에만 남고 어디에도 보내지 않습니다. 비밀번호 칸에서는 아무것도 기록하지 않습니다.")
            }

            Section {
                Button("최근 사용 이모지 지우기", role: .destructive) { confirmingRecentReset = true }
                    .disabled(!hasRecentAnnotations)
                Button("입력 학습 재설정", role: .destructive) { confirmingLearningReset = true }
                    .disabled(!hasLearnedWords && !hasRecentAnnotations)
            } footer: {
                Text("입력 학습을 끄면 앞으로 배우지 않고, 재설정은 이미 배운 것을 지웁니다.")
            }
        }
        .navigationTitle("개인 정보")
        .alert("배운 단어를 모두 지울까요?", isPresented: $confirmingLearningReset) {
            Button("취소", role: .cancel) {}
            Button("재설정", role: .destructive) { model.resetLearning() }
        } message: {
            Text("최근에 고른 이모지도 함께 사라집니다. 되돌릴 수 없습니다.")
        }
        .alert("최근 사용 이모지를 지울까요?", isPresented: $confirmingRecentReset) {
            Button("취소", role: .cancel) {}
            Button("지우기", role: .destructive) { model.resetRecentAnnotations() }
        } message: {
            Text("배운 단어는 그대로 남습니다.")
        }
    }

    /// 없는 것은 적지 않는다 — "이모지 0개"는 알려 주는 바가 없다.
    private func detail(_ summary: FfiPersonalizationSummary) -> String {
        var parts: [String] = []
        if summary.learnedWords > 0 {
            parts.append(String(
                format: NSLocalizedString("배운 단어 %lld개", comment: "학습된 단어 수"),
                Int(summary.learnedWords)
            ))
        }
        if summary.recentAnnotations > 0 {
            parts.append(String(
                format: NSLocalizedString("최근 이모지 %lld개", comment: "최근 사용 이모지 수"),
                Int(summary.recentAnnotations)
            ))
        }
        return parts.joined(separator: " · ")
    }
}
