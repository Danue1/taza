//! 통합 검색면 — 낱말에 곁들이는 것들(이모지·기호)을 찾고 넣는 자리. 무엇을 어떤
//! 순서로 보일지는 팩 데이터와 개인화가 정하므로 셸은 그리기만 한다.

use crate::contract::{
    AnnotationPanel, AnnotationPanelGroup, AnnotationPanelItem, CandidateGroup, EditorContext,
    Effect, Pack,
};

use super::Engine;

/// 검색 결과 한 묶음에 담는 항목 수의 상한. 검색은 "이 낱말로 부르는 것"을 찾는 일이라
/// 앞쪽 몇 줄이면 충분하다. 검색어 없이 훑는 목록에는 이 상한을 걸지 않는다 — 묶음을
/// 끝까지 넘길 수 있어야 사람·제스처처럼 뒤쪽에 있는 이모지에 닿는다.
const PANEL_GROUP_LIMIT: usize = 128;
/// 검색어로 표를 훑을 때 모으는 항목 수 — 갈래로 나눈 뒤 그룹별 상한이 다시 걸린다.
const PANEL_SEARCH_POOL: usize = PANEL_GROUP_LIMIT * 3;

/// 낱말에 곁들이는 갈래들 — 검색면이 그룹으로 세우는 것은 이들뿐이다.
fn accompanying_groups() -> impl Iterator<Item = CandidateGroup> {
    CandidateGroup::DISPLAY_ORDER
        .into_iter()
        .filter(|group| *group != CandidateGroup::Word)
}

impl Engine {
    /// 통합 검색면 내용. `query`는 검색 필드에 친 표시 문자열이며, 비어 있으면 자주 쓰는
    /// 것과 갈래별 표시 순서를 낸다.
    pub fn annotation_panel(&self, query: &str) -> AnnotationPanel {
        let holder = self.pack.clone();
        let pack = holder
            .as_ref()
            .and_then(|holder| Pack::open(holder.bytes()).ok());
        let mut groups = Vec::new();
        if query.is_empty() {
            let recent: Vec<AnnotationPanelItem> = self
                .personalization
                .recent_annotations()
                .iter()
                .map(|(group, text)| AnnotationPanelItem {
                    group: *group,
                    text: text.clone(),
                })
                .collect();
            if !recent.is_empty() {
                // 갈래도 묶음도 없는 그룹이 곧 "최근에 고른 것들"이다
                groups.push(AnnotationPanelGroup {
                    group: None,
                    category: None,
                    items: recent,
                });
            }
            if let Some(catalog) = pack.as_ref().and_then(|pack| pack.annotation_catalog()) {
                // 묶음과 순서는 팩이 정한다 — 이모지는 빌트인 키보드와 같은 묶음으로 실려 온다
                for section in catalog.sections(usize::MAX) {
                    let items: Vec<AnnotationPanelItem> = section
                        .items
                        .into_iter()
                        .map(|text| AnnotationPanelItem {
                            group: section.group,
                            text: text.to_string(),
                        })
                        .collect();
                    if items.is_empty() {
                        continue;
                    }
                    groups.push(AnnotationPanelGroup {
                        group: Some(section.group),
                        category: section.category,
                        items,
                    });
                }
            }
            return AnnotationPanel { groups };
        }

        // 검색은 낱말로 한다 — 표의 키가 조회 키 공간에 있으므로 검색어도 그리로 옮긴다
        let (Some(table), Some(key)) = (
            pack.as_ref().and_then(|pack| pack.annotations()),
            self.suggester.policy().encoding.encode(query),
        ) else {
            return AnnotationPanel::default();
        };
        let found = table.search(&key, PANEL_SEARCH_POOL);
        for group in accompanying_groups() {
            let items: Vec<AnnotationPanelItem> = found
                .iter()
                .filter(|annotation| annotation.group == group)
                .take(PANEL_GROUP_LIMIT)
                .map(|annotation| AnnotationPanelItem {
                    group,
                    text: annotation.text.to_string(),
                })
                .collect();
            if !items.is_empty() {
                groups.push(AnnotationPanelGroup {
                    group: Some(group),
                    category: None,
                    items,
                });
            }
        }
        AnnotationPanel { groups }
    }

    /// 검색면에서 고른 것을 넣는다. 진행 중 조합은 언어별 규칙으로 먼저 확정하고,
    /// 고른 것은 자주 쓰는 목록 맨 앞으로 올라간다(시크릿 필드에서는 남기지 않는다).
    pub fn select_annotation(
        &mut self,
        group: CandidateGroup,
        text: &str,
        context: &EditorContext,
    ) -> Vec<Effect> {
        if text.is_empty() {
            return Vec::new();
        }
        let assistance =
            crate::policy::assistance(&self.preferences, self.keyboard.traits(), context);
        let mut effects = self.finalize_composition();
        effects.push(Effect::CommitText(text.to_string()));
        if assistance.personalizing && !context.incognito {
            self.personalization.record_annotation(group, text);
        }
        effects
    }
}
