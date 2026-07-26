import SwiftUI

/// 키보드가 쓰는 라이브러리와 그 라이선스. 사전 출처와 나란히 두지 않고 따로 둔다 —
/// 사전 출처는 팩에 따라 달라지고 이쪽은 빌드에 따라 달라진다.
struct SoftwareLicenseList: View {
    private let catalog = SoftwareLicenseCatalog.load()

    var body: some View {
        List {
            Section {
                Text("키보드에 링크되는 라이브러리입니다. 빌드 도구와 사전 제작 도구는 기기에 나가지 않으므로 여기 없습니다.")
                    .font(.footnote)
                    .foregroundStyle(.tazaSecondaryLabel)
            }
            Section {
                ForEach(catalog.packages, id: \.name) { package in
                    NavigationLink {
                        SoftwareLicenseDetail(
                            package: package,
                            texts: catalog.texts(for: package)
                        )
                    } label: {
                        VStack(alignment: .leading, spacing: 2) {
                            HStack(alignment: .firstTextBaseline) {
                                Text(package.name)
                                Spacer(minLength: 8)
                                Text(package.version)
                                    .font(.footnote)
                                    .foregroundStyle(.tazaSecondaryLabel)
                            }
                            if !package.license.isEmpty {
                                Text(package.license)
                                    .font(.footnote)
                                    .foregroundStyle(.tazaSecondaryLabel)
                            }
                        }
                    }
                }
            } header: {
                Text("라이브러리 \(catalog.packages.count)개")
            }
        }
        .navigationTitle("소프트웨어 라이선스")
    }
}

/// 라이선스 본문 — 이중 라이선스 크레이트는 본문이 둘이고 둘 다 싣는다.
private struct SoftwareLicenseDetail: View {
    let package: SoftwareLicenseCatalog.Package
    let texts: [String]

    var body: some View {
        List {
            Section {
                LabeledContent("판", value: package.version)
                if !package.license.isEmpty {
                    LabeledContent("라이선스", value: package.license)
                }
                if let url = URL(string: package.repository), !package.repository.isEmpty {
                    Link("저장소", destination: url)
                }
            }
            if texts.isEmpty {
                Section {
                    // 본문을 함께 배포하지 않는 크레이트가 있다 — 저장소로 안내한다
                    Text("이 라이브러리는 라이선스 본문을 함께 배포하지 않습니다. 저장소에서 확인하세요.")
                        .font(.footnote)
                        .foregroundStyle(.tazaSecondaryLabel)
                }
            }
            ForEach(texts.indices, id: \.self) { index in
                Section {
                    Text(texts[index])
                        .font(.footnote.monospaced())
                        .foregroundStyle(.tazaSecondaryLabel)
                        .textSelection(.enabled)
                }
            }
        }
        .navigationTitle(package.name)
        .navigationBarTitleDisplayMode(.inline)
    }
}
