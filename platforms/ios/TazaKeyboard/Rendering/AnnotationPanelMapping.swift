import Foundation

/// 이모지·기호·이모티콘을 모아 보여 주는 검색면의 내용 번역과 선택 처리.
extension KeyboardViewController {
    func panelModel(from panel: FfiAnnotationPanel) -> AnnotationPanelModel {
        AnnotationPanelModel(
            groups: panel.groups.map { group in
                AnnotationPanelModel.Group(
                    group: group.group.map(candidateGroup),
                    category: group.category.map(emojiCategory),
                    label: group.label,
                    items: group.items.map { item in
                        AnnotationPanelModel.Item(
                            text: item.text,
                            group: candidateGroup(item.group)
                        )
                    }
                )
            }
        )
    }

    private func emojiCategory(_ category: FfiEmojiCategory) -> AnnotationPanelModel.Category {
        switch category {
        case .smileysAndPeople: .smileysAndPeople
        case .animalsAndNature: .animalsAndNature
        case .foodAndDrink: .foodAndDrink
        case .activities: .activities
        case .travelAndPlaces: .travelAndPlaces
        case .objects: .objects
        case .symbols: .symbols
        case .flags: .flags
        }
    }

    /// 검색면에서 고른 것 — 넣는 일과 최근 사용 기록은 코어가 한다.
    func selectAnnotation(_ group: CandidateModel.Group, _ text: String) {
        guard let session = activeSession else { return }
        let ffiGroup: FfiCandidateGroup = switch group {
        case .word: .word
        case .emoji: .emoji
        case .symbol: .symbol
        case .emoticon: .emoticon
        }
        apply(effects: session.selectAnnotation(
            group: ffiGroup,
            text: text,
            context: currentContext()
        ))
        // 고른 것이 자주 쓰는 목록 맨 앞으로 올라오므로 판을 다시 받는다
        panelView.setPanel(panelModel(from: session.annotationPanel(query: "")))
    }
}
