import SwiftUI

/// 입력 보조 항목의 화면 표기 — 이름은 순정 키보드(설정 → 일반 → 키보드)를 따른다.
/// 저장소(TypingPreferences)는 표기를 모르고, 번역은 앱의 문자열 카탈로그가 맡는다.
extension TypingOption {
    var title: LocalizedStringKey {
        switch self {
        case .autoCorrection: "자동 수정"
        case .predictions: "자동 완성"
        case .doubleSpacePeriod: "\".\" 단축키"
        case .personalizedLearning: "입력 학습"
        }
    }
}
