import Foundation

/// 이모지·기호·이모티콘을 모아 보여 주는 검색면의 내용 번역과 선택 처리.
extension KeyboardViewController {
    func panelModel(from panel: FfiAnnotationPanel) -> AnnotationPanelModel {
        AnnotationPanelModel(
            groups: panel.groups.map { group in
                AnnotationPanelModel.Group(
                    group: group.group.map(candidateGroup),
                    category: group.category.map(emojiCategory),
                    label: label(group: group.group, category: group.category),
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

    /// 그룹 헤더 문구. 코어는 갈래와 묶음만 알려 주므로 그것을 무슨 말로 적을지는
    /// 화면이 정한다 — 이름은 빌트인 키보드가 쓰는 말을 그대로 따른다(계승 원칙).
    private func label(
        group: FfiCandidateGroup?,
        category: FfiEmojiCategory?
    ) -> String {
        if let category {
            return NSLocalizedString(emojiCategoryName(category), comment: "이모지 묶음 이름")
        }
        guard let group else {
            // 갈래도 묶음도 없는 그룹은 최근에 고른 것들이다
            return NSLocalizedString("자주 쓰는", comment: "최근에 고른 것들")
        }
        return NSLocalizedString(candidateGroupName(group), comment: "후보 갈래 이름")
    }

    private func emojiCategoryName(_ category: FfiEmojiCategory) -> String {
        switch category {
        case .smileysAndPeople: "스마일리 및 사람"
        case .animalsAndNature: "동물 및 자연"
        case .foodAndDrink: "음식 및 음료"
        case .activities: "활동"
        case .travelAndPlaces: "여행 및 장소"
        case .objects: "사물"
        case .symbols: "기호"
        case .flags: "깃발"
        }
    }

    private func candidateGroupName(_ group: FfiCandidateGroup) -> String {
        switch group {
        case .word: "낱말"
        case .emoji: "이모지"
        case .symbol: "기호"
        case .emoticon: "얼굴 문자"
        }
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
