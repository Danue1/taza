import Foundation

/// 후보의 갈래와 신원을 디자인 시스템의 배치 단위로 옮긴다 — 어느 갈래가 어떤 자리를
/// 갖는지와 어떤 후보를 어떻게 세울지는 디자인 시스템이 알고, 셸은 이름만 맞춰 준다.
extension KeyboardViewController {
    func candidateModel(_ candidate: FfiCandidate) -> CandidateModel {
        CandidateModel(
            text: candidate.text,
            kind: candidateKind(candidate.kind),
            group: candidateGroup(candidate.group)
        )
    }

    func candidateKind(_ kind: FfiCandidateKind) -> CandidateModel.Kind {
        switch kind {
        case .typed: .typed
        case .prediction: .prediction
        case .conversion: .conversion
        case .correction: .correction
        }
    }

    func candidateGroup(_ group: FfiCandidateGroup) -> CandidateModel.Group {
        switch group {
        case .word: .word
        case .emoji: .emoji
        case .symbol: .symbol
        case .emoticon: .emoticon
        }
    }
}
