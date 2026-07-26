import SwiftUI

/// 입력 보조 항목의 화면 표기 — 이름은 순정 키보드(설정 → 일반 → 키보드)를 따르고,
/// 순정에 없는 항목만 우리가 이름을 짓는다. 저장소(TypingPreferences)는 표기를 모르고,
/// 번역은 앱의 문자열 카탈로그가 맡는다.
extension TypingOption {
    var title: LocalizedStringKey {
        switch self {
        case .autoCorrection: "자동 수정"
        case .predictions: "자동 완성"
        case .doubleSpacePeriod: "\".\" 단축키"
        case .personalizedLearning: "입력 학습"
        case .autoCapitalization: "자동 대문자 변환"
        case .smartPunctuation: "스마트 문장부호"
        case .autoPairing: "괄호·따옴표 자동 짝"
        case .annotationCandidates: "이모지·기호 후보"
        }
    }

    /// 무엇이 달라지는지 한 줄로 — 이름만으로는 알기 어려운 항목에만 붙인다.
    var explanation: LocalizedStringKey? {
        switch self {
        case .autoCapitalization: "문장이 시작되는 자리에서 shift가 미리 올라갑니다."
        case .smartPunctuation: "따옴표가 짝을 맞추고 \"--\"가 줄표가 됩니다."
        case .autoPairing: "여는 괄호를 치면 닫는 괄호가 함께 들어갑니다."
        case .annotationCandidates: "치고 있는 낱말에 딸린 이모지를 후보 바에 곁들입니다."
        default: nil
        }
    }

    /// 설정 화면에서 묶이는 자리. 항목이 늘어도 화면 코드가 아니라 이 표가 자란다.
    var group: TypingOptionGroup {
        switch self {
        case .autoCorrection, .predictions, .annotationCandidates: .suggestion
        case .doubleSpacePeriod, .autoCapitalization, .smartPunctuation, .autoPairing: .formatting
        case .personalizedLearning: .learning
        }
    }
}

enum TypingOptionGroup: CaseIterable {
    case suggestion
    case formatting
    case learning

    var title: LocalizedStringKey {
        switch self {
        case .suggestion: "교정과 예측"
        case .formatting: "자동 서식"
        case .learning: "학습"
        }
    }

    var options: [TypingOption] {
        TypingOption.allCases.filter { $0.group == self }
    }
}
