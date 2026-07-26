import SwiftUI

/// 사전 상태는 순정 키보드의 언어 목록처럼 행 오른쪽에 붙는다 — 내장 언어는
/// 내려받을 것이 없으므로 조작이 보이지 않는다.
struct PackStatusView: View {
    let name: String
    let state: PackLibraryModel.State?
    let install: () -> Void
    let remove: () -> Void

    var body: some View {
        switch state {
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
                .accessibilityLabel("\(name) 사전 삭제")
            }
        case .notInstalled(let size):
            Button(action: install) {
                HStack(spacing: 4) {
                    Image(systemName: "arrow.down.circle")
                    if size > 0 {
                        Text(ByteCountFormatter.string(fromByteCount: Int64(size), countStyle: .file))
                            .font(.footnote)
                    }
                }
            }
            .accessibilityLabel("\(name) 사전 다운로드")
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
