import UIKit

// 키보드 한 판을 그리기 위한 값. 옮겨 담는 일은 `KeyboardFrameMapping`이 하고,
// 설정 앱의 배열 미리보기도 같은 모델을 그린다.

/// 코어가 준 키 하나를 그리기 위한 표현. FFI 타입을 셸이 이 모델로 옮겨 담아
/// 디자인 시스템이 엔진에 의존하지 않게 한다(설정 앱도 같은 컴포넌트를 쓴다).
public struct KeyModel {
    public let label: String
    public let appearance: KeyCapView.Appearance
    /// 코어가 정한 이 키 라벨의 글꼴 크기(pt)
    public let fontSize: CGFloat
    /// 키보드 영역 기준 정규화 좌표
    public let bounds: CGRect
    public let accessibilityLabel: String
    public let accessibilityValue: String?
    public let accessibilityHint: String?
    public let alternates: [String]
    public let isActive: Bool
    /// shift 오른쪽·backspace 왼쪽처럼 글자 열과 갈라지는 자리에 더할 여백(pt)
    public let leadingExtraGap: CGFloat
    public let trailingExtraGap: CGFloat

    public init(
        label: String,
        appearance: KeyCapView.Appearance,
        fontSize: CGFloat,
        bounds: CGRect,
        accessibilityLabel: String,
        accessibilityValue: String? = nil,
        accessibilityHint: String? = nil,
        alternates: [String] = [],
        isActive: Bool = false,
        leadingExtraGap: CGFloat = 0,
        trailingExtraGap: CGFloat = 0
    ) {
        self.label = label
        self.appearance = appearance
        self.fontSize = fontSize
        self.bounds = bounds
        self.accessibilityLabel = accessibilityLabel
        self.accessibilityValue = accessibilityValue
        self.accessibilityHint = accessibilityHint
        self.alternates = alternates
        self.isActive = isActive
        self.leadingExtraGap = leadingExtraGap
        self.trailingExtraGap = trailingExtraGap
    }
}

public struct KeyboardFrameModel {
    public let rows: [[KeyModel]]

    public init(rows: [[KeyModel]]) {
        self.rows = rows
    }
}
