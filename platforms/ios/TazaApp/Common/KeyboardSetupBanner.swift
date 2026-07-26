import SwiftUI
import UIKit

/// 키보드를 아직 켜지 않은 사용자에게만 보이는 첫 걸음. iOS는 키보드 목록으로 바로 가는
/// 링크를 열어 주지 않으므로 앱 설정까지 데려다주고 마지막 한 걸음만 사용자가 딛는다.
struct KeyboardSetupBanner: View {
    @Environment(\.openURL) private var openURL

    var body: some View {
        Button {
            if let url = URL(string: UIApplication.openSettingsURLString) {
                openURL(url)
            }
        } label: {
            HStack(spacing: 12) {
                Image(systemName: "keyboard")
                    .font(.system(size: 22))
                VStack(alignment: .leading, spacing: 2) {
                    Text("Taza 키보드 추가")
                        .font(.subheadline.weight(.semibold))
                    Text("설정에서 키보드 → 새로운 키보드 추가에 있는 Taza를 켜세요.")
                        .font(.footnote)
                        .foregroundStyle(.tazaSecondaryLabel)
                        .multilineTextAlignment(.leading)
                }
                Spacer(minLength: 8)
                Image(systemName: "chevron.right")
                    .font(.footnote.weight(.semibold))
                    .foregroundStyle(Color(uiColor: .tertiaryLabel))
            }
            .padding(.vertical, 4)
        }
        .foregroundStyle(.tazaAccent)
    }
}
