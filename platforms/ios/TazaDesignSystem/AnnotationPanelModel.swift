import UIKit

// 검색면이 그리는 값. 뷰와 갈라 두는 이유는 이 값을 만드는 쪽
// (`AnnotationPanelMapping`)이 뷰를 알 필요가 없기 때문이다.

/// 통합 검색면에 담기는 것 — 코어가 그룹 단위로 내려 준다(`annotation_panel`).
public struct AnnotationPanelModel {
    public struct Item {
        public let text: String
        public let group: CandidateModel.Group

        public init(text: String, group: CandidateModel.Group) {
            self.text = text
            self.group = group
        }
    }

    /// 이모지가 서는 묶음 — 코어가 정해 내려 주고, 셸은 묶음마다 표식을 고른다.
    public enum Category {
        case smileysAndPeople
        case animalsAndNature
        case foodAndDrink
        case activities
        case travelAndPlaces
        case objects
        case symbols
        case flags
    }

    public struct Group {
        /// 이 그룹의 갈래. 자주 쓰는 것처럼 갈래가 섞이는 그룹은 nil이다.
        public let group: CandidateModel.Group?
        /// 이모지 묶음이면 그 자리
        public let category: Category?
        public let label: String
        public let items: [Item]

        public init(
            group: CandidateModel.Group?,
            category: Category? = nil,
            label: String,
            items: [Item]
        ) {
            self.group = group
            self.category = category
            self.label = label
            self.items = items
        }
    }

    public let groups: [Group]

    public init(groups: [Group]) {
        self.groups = groups
    }
}
