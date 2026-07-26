import Foundation

/// 후보 갈래를 디자인 시스템의 배치 단위로 옮긴다 — 어느 갈래가 어떤 자리를 갖는지는
/// 디자인 시스템이 알고, 셸은 이름만 맞춰 준다.
extension KeyboardViewController {
    func candidateModel(_ candidate: FfiCandidate) -> CandidateModel {
        CandidateModel(text: candidate.text, group: candidateGroup(candidate.group))
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
