import SwiftUI

/// 배열 고르기 전용 화면.
///
/// 언어 설정 안에 격자를 펴 두면 배열이 늘어날수록 언어팩·입력 보조가 그림에 밀린다.
/// 배열은 한 번 고르면 오래 두는 값이므로 자기 화면으로 내보내고, 언어 설정에는
/// 지금 고른 것만 남긴다.
struct LayoutSelectionList: View {
    @ObservedObject var model: SettingsModel
    let language: TazaLanguage

    var body: some View {
        List {
            Section {
                LayoutTileGrid(
                    previews: model.layoutPreviews(language),
                    selection: model.layoutBinding(for: language)
                )
                .listRowInsets(EdgeInsets(top: 8, leading: 12, bottom: 12, trailing: 12))
                .listRowBackground(Color.clear)
            } footer: {
                Text("고른 배열은 이 언어로 칠 때 바로 걸립니다.")
            }
        }
        .navigationTitle("키 배열")
        .navigationBarTitleDisplayMode(.inline)
    }
}
