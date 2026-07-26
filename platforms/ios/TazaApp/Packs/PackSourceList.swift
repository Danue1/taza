import SwiftUI

/// 이 언어의 사전이 어떤 데이터로 만들어졌는지 — 원천마다 한 줄이다. 라이선스가
/// 요구하는 표시 문구는 그 줄에 딸리므로, 이름과 문구가 위치로만 짝지어지지 않는다.
///
/// 사전은 언어마다 다르므로 고지도 그 언어 화면에 있다. 루트에 모아 두면 어느 사전의
/// 출처인지가 흐려지고, 설정을 고치러 온 사람에게 가장 큰 덩어리로 보인다.
struct PackSourceList: View {
    @ObservedObject var model: SettingsModel
    @ObservedObject var packs: PackLibraryModel
    let language: TazaLanguage

    var body: some View {
        List {
            Section(model.displayName(language)) {
                // 같은 이름의 원천이 두 번 실릴 수 있으므로 자리로 가른다
                let sources = packs.sources[language] ?? []
                ForEach(sources.indices, id: \.self) { index in
                    PackSourceRow(source: sources[index])
                }
            }
        }
        .navigationTitle("사전 출처")
    }
}

private struct PackSourceRow: View {
    let source: FfiPackSource

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(alignment: .firstTextBaseline) {
                Text(source.name)
                Spacer(minLength: 8)
                Text(source.version)
                    .font(.footnote)
                    .foregroundStyle(.tazaSecondaryLabel)
            }
            // 파란색은 누를 수 있다는 뜻이므로 쓰지 않는다 — 라이선스는 이름표다
            Text(source.license)
                .font(.footnote.weight(.medium))
                .foregroundStyle(.tazaLabel)
            if !source.attribution.isEmpty {
                // 문구 안의 주소는 눌러서 열 수 있어야 한다 — 출처를 확인하러 온
                // 사람에게 주소가 글자로만 있으면 옮겨 적는 수밖에 없다
                Text(linked(source.attribution))
                    .font(.footnote)
                    .foregroundStyle(.tazaSecondaryLabel)
            }
        }
        .padding(.vertical, 2)
    }

    /// 문구에 섞인 주소를 링크로 만든다. 원천마다 주소를 따로 받지 않고 문구에서
    /// 찾아내므로, 새 원천이 들어와도 레시피에 적을 것이 늘지 않는다.
    private func linked(_ text: String) -> AttributedString {
        var attributed = AttributedString(text)
        let detector = try? NSDataDetector(types: NSTextCheckingResult.CheckingType.link.rawValue)
        let matches = detector?.matches(
            in: text,
            range: NSRange(text.startIndex..., in: text)
        ) ?? []
        for match in matches {
            guard let url = match.url,
                  let range = Range(match.range, in: text),
                  let attributedRange = Range(range, in: attributed)
            else {
                continue
            }
            attributed[attributedRange].link = url
        }
        return attributed
    }
}
