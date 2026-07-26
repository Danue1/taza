import SwiftUI

/// 갈래가 셋인 설정 한 줄. 목록 안의 세그먼트 컨트롤은 자기 라벨을 그리지 않으므로
/// (`Picker`의 라벨이 숨는다) 이름을 위에 따로 세운다.
struct SegmentedSetting<Value: Hashable>: View {
    let title: LocalizedStringKey
    @Binding var selection: Value
    let options: [Value]
    let label: (Value) -> LocalizedStringKey

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title)
            Picker(title, selection: $selection) {
                ForEach(options, id: \.self) { option in
                    Text(label(option)).tag(option)
                }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
        }
        .padding(.vertical, 2)
    }
}
