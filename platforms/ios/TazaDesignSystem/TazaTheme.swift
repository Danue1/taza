import SwiftUI
import UIKit

/// 키보드 익스텐션과 설정 앱이 함께 쓰는 디자인 토큰.
///
/// 값은 iOS 순정 키보드를 기준선으로 잡았다 (빌트인 UX 계승 원칙) — 키 높이·폭·글자
/// 크기가 순정과 비슷해야 사용자가 손 위치를 다시 배우지 않는다. 색은 라이트·다크
/// 양쪽을 담은 동적 색으로 정의해 셸에서 분기하지 않는다.
public enum TazaTheme {
    // MARK: - 색

    public enum Color {
        /// 키보드 판 배경
        public static let keyboardBackground = dynamic(light: 0xD1D3D9, dark: 0x2C2C2E)
        /// 문자 키
        public static let keySurface = dynamic(light: 0xFFFFFF, dark: 0x6B6B6D)
        /// shift·삭제·심볼처럼 문자가 아닌 키 (순정은 한 톤 어둡다)
        public static let controlKeySurface = dynamic(light: 0xACB4BE, dark: 0x4A4A4C)
        /// 눌린 상태 — 문자 키는 어두워지고 제어 키는 밝아진다(순정 관례)
        public static let keySurfacePressed = dynamic(light: 0xACB0BA, dark: 0x7F7F81)
        public static let controlKeySurfacePressed = dynamic(light: 0xFFFFFF, dark: 0x6B6B6D)
        /// 켜진 shift 등 상태 표시
        public static let keySurfaceActive = dynamic(light: 0xFFFFFF, dark: 0x8E8E90)

        public static let label = dynamic(light: 0x000000, dark: 0xFFFFFF)
        public static let secondaryLabel = dynamic(light: 0x6D6D72, dark: 0xA0A0A5)
        public static let separator = dynamic(light: 0xE5E5EA, dark: 0x48484A)
        /// 팝업·시트 바탕
        public static let popupSurface = dynamic(light: 0xFFFFFF, dark: 0x3A3A3C)
        public static let selection = UIColor.systemBlue.withAlphaComponent(0.16)
        public static let accent = UIColor.systemBlue
        /// 강조색 바탕 위의 글자 — 라이트·다크 모두 흰색이다(순정 리턴키)
        public static let emphasizedLabel = UIColor.white

        private static func dynamic(light: UInt32, dark: UInt32) -> UIColor {
            UIColor { traits in
                traits.userInterfaceStyle == .dark ? color(dark) : color(light)
            }
        }

        private static func color(_ value: UInt32) -> UIColor {
            UIColor(
                red: CGFloat((value >> 16) & 0xFF) / 255,
                green: CGFloat((value >> 8) & 0xFF) / 255,
                blue: CGFloat(value & 0xFF) / 255,
                alpha: 1
            )
        }
    }

    // MARK: - 치수

    /// 코어가 폼팩터를 보고 정해 내려준 실측 치수를 옮겨 담는 그릇. 셸은 폼팩터를
    /// 다시 판단하지 않으므로 여기에 세로·가로 같은 갈래가 없다.
    public struct Metrics: Sendable {
        public let candidateBarHeight: CGFloat
        /// 키 그리드 높이 — 키의 정규화 높이에 곱하면 실제 높이다
        public let gridHeight: CGFloat
        public let letterFontSize: CGFloat
        public let controlFontSize: CGFloat

        public var totalHeight: CGFloat { candidateBarHeight + gridHeight }

        public init(
            candidateBarHeight: CGFloat,
            gridHeight: CGFloat,
            letterFontSize: CGFloat,
            controlFontSize: CGFloat
        ) {
            self.candidateBarHeight = candidateBarHeight
            self.gridHeight = gridHeight
            self.letterFontSize = letterFontSize
            self.controlFontSize = controlFontSize
        }

        /// 코어가 아직 자기 크기를 모를 때의 첫 프레임용 — 표준 폰 세로 값이다.
        public static let placeholder = Metrics(
            candidateBarHeight: 44,
            gridHeight: 216,
            letterFontSize: 22,
            controlFontSize: 16
        )
    }

    /// 키 하나를 그리는 방식. 순정은 가로 간격 6pt, 모서리 5pt이고 세로 간격은 키
    /// 높이를 따라가므로, 폼팩터가 늘어도 이 비율만 지키면 된다.
    public enum Key {
        public static let horizontalGap: CGFloat = 6
        public static let verticalGapRatio: CGFloat = 0.22
        public static let cornerRadius: CGFloat = 5
        /// 순정 키캡의 아래쪽 1pt 그림자
        public static let shadowColor = UIColor(red: 0.53, green: 0.53, blue: 0.55, alpha: 1)
        public static let shadowOpacity: Float = 0.9
    }

    // MARK: - 타이포그래피

    public enum Typography {
        public static func keyLabel(size: CGFloat) -> UIFont {
            UIFont.systemFont(ofSize: size, weight: .regular)
        }

        public static func languageKeyLabel(size: CGFloat) -> UIFont {
            UIFont.systemFont(ofSize: size, weight: .semibold)
        }

        /// 스페이스바의 현재 언어 표기 — 순정처럼 작고 흐리게
        public static let spaceLabel = UIFont.systemFont(ofSize: 13)

        /// 후보 바 글꼴 — 낱말은 순정 예측 바와 같은 17pt이고, 곁들이는 것은 갈래마다
        /// 눈에 잡히는 크기가 달라 따로 잡는다(이모지는 크게, 글자로 그린 얼굴은 작게).
        public static func candidate(group: CandidateModel.Group) -> UIFont {
            switch group {
            case .word: UIFont.systemFont(ofSize: 17)
            case .emoji: UIFont.systemFont(ofSize: 24)
            case .symbol: UIFont.systemFont(ofSize: 21)
            case .emoticon: UIFont.systemFont(ofSize: 15)
            }
        }

        /// 통합 검색면 글꼴 — 훑어보는 자리라 후보 바보다 크게 잡는다.
        public static func panelItem(group: CandidateModel.Group) -> UIFont {
            switch group {
            case .word: UIFont.systemFont(ofSize: 20)
            case .emoji: UIFont.systemFont(ofSize: 30)
            case .symbol: UIFont.systemFont(ofSize: 24)
            case .emoticon: UIFont.systemFont(ofSize: 15)
            }
        }

        public static let panelGroupLabel = UIFont.systemFont(ofSize: 12, weight: .semibold)
        public static let panelRail = UIFont.systemFont(ofSize: 13)

        public static let popupItem = UIFont.systemFont(ofSize: 16)
        public static let popupItemSelected = UIFont.systemFont(ofSize: 16, weight: .semibold)
    }

    // MARK: - 팝업

    public enum Popup {
        public static let cornerRadius: CGFloat = 10
        public static let itemHeight: CGFloat = 44
        public static let width: CGFloat = 220
        public static let shadowRadius: CGFloat = 14
        public static let shadowOpacity: Float = 0.22
    }
}

/// SwiftUI(설정 앱)에서 같은 토큰을 쓰기 위한 접근자.
public extension ShapeStyle where Self == SwiftUI.Color {
    static var tazaLabel: SwiftUI.Color { SwiftUI.Color(uiColor: TazaTheme.Color.label) }
    static var tazaSecondaryLabel: SwiftUI.Color {
        SwiftUI.Color(uiColor: TazaTheme.Color.secondaryLabel)
    }
    static var tazaSeparator: SwiftUI.Color { SwiftUI.Color(uiColor: TazaTheme.Color.separator) }
    static var tazaSurface: SwiftUI.Color { SwiftUI.Color(uiColor: TazaTheme.Color.popupSurface) }
    static var tazaSelection: SwiftUI.Color { SwiftUI.Color(uiColor: TazaTheme.Color.selection) }
    static var tazaAccent: SwiftUI.Color { SwiftUI.Color(uiColor: TazaTheme.Color.accent) }
}
