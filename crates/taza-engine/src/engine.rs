//! 코어의 최상위 조립 지점. 키보드 상태·합성기·개인화·언어팩을 한 객체가 소유하므로
//! 셸(taza-ffi)은 타입 번역만 하고 조립하지 않는다.
//!
//! sans-io는 유지된다 — 파일을 여는 일은 셸의 몫이고, 코어는 이미 열린 바이트만 받는다.

use std::sync::Arc;

use crate::contract::{
    AnnotationPanel, AnnotationPanelGroup, AnnotationPanelItem, Candidate, CandidateGroup,
    Composer, ComposerEvent, ComposerOutput, EditorContext, Effect, EmojiCategory, FieldKind,
    InputEvent, Pack,
    SuggestionRequest, UserPreferences,
};
use crate::keyboard::{
    FrameKey, FrameMetrics, KeySignal, Keyboard, KeyboardFrame, KeyboardMetrics, ShellRequest,
};
use crate::lang::LanguageDescriptor;
use crate::pack::PackError;
use crate::personalization::{PersonalizationState, PersonalizationStore};
use crate::policy::Assistance;
use crate::suggest::{Suggester, Suggestion, SuggestionSources};

/// 언어팩 바이트의 소유자. 온디바이스에서는 mmap, 테스트·평가에서는 `Vec<u8>`이며
/// 둘 다 `AsRef<[u8]>`이므로 별도 구현이 필요 없다.
pub trait PackBytes: Send + Sync {
    fn bytes(&self) -> &[u8];
}

impl<Source: AsRef<[u8]> + Send + Sync> PackBytes for Source {
    fn bytes(&self) -> &[u8] {
        self.as_ref()
    }
}

/// 검색 결과 한 묶음에 담는 항목 수의 상한. 검색은 "이 낱말로 부르는 것"을 찾는 일이라
/// 앞쪽 몇 줄이면 충분하다. 검색어 없이 훑는 목록에는 이 상한을 걸지 않는다 — 묶음을
/// 끝까지 넘길 수 있어야 사람·제스처처럼 뒤쪽에 있는 이모지에 닿는다.
const PANEL_GROUP_LIMIT: usize = 128;
/// 검색어로 표를 훑을 때 모으는 항목 수 — 갈래로 나눈 뒤 그룹별 상한이 다시 걸린다.
const PANEL_SEARCH_POOL: usize = PANEL_GROUP_LIMIT * 3;

/// 검색면 그룹 이름 — 키 라벨과 마찬가지로 코어가 정한다(셸은 그리기만 한다).
fn panel_group_label(group: CandidateGroup) -> &'static str {
    match group {
        CandidateGroup::Word => "낱말",
        CandidateGroup::Emoji => "이모지",
        CandidateGroup::Symbol => "기호",
        CandidateGroup::Emoticon => "얼굴 문자",
    }
}

/// 이모지 묶음 이름 — 빌트인 키보드가 쓰는 이름을 그대로 따른다(계승 원칙).
fn emoji_category_label(category: EmojiCategory) -> &'static str {
    match category {
        EmojiCategory::SmileysAndPeople => "스마일리 및 사람",
        EmojiCategory::AnimalsAndNature => "동물 및 자연",
        EmojiCategory::FoodAndDrink => "음식 및 음료",
        EmojiCategory::Activities => "활동",
        EmojiCategory::TravelAndPlaces => "여행 및 장소",
        EmojiCategory::Objects => "사물",
        EmojiCategory::Symbols => "기호",
        EmojiCategory::Flags => "깃발",
    }
}

/// 최근에 고른 것들이 모이는 그룹의 이름 — 갈래가 섞이므로 갈래 이름을 쓸 수 없다.
const PANEL_RECENT_LABEL: &str = "자주 쓰는";

/// 터치 한 번의 결과 — 입력이 만든 Effect와, 코어가 판정할 수 없어 셸에 넘기는 요청.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PressResult {
    pub effects: Vec<Effect>,
    /// shift·레이어 전환 등으로 프레임을 다시 그려야 하는지
    pub layout_changed: bool,
    pub request: Option<ShellRequest>,
}

/// 키보드 익스텐션 프로세스당 하나. 이벤트를 받아 Effect 목록을 낸다.
pub struct Engine {
    language: LanguageDescriptor,
    composer: Box<dyn Composer>,
    suggester: Suggester,
    keyboard: Keyboard,
    personalization: PersonalizationStore,
    /// 팩 교체로 키보드를 다시 만들어도 셸이 주입한 표시 환경은 이어져야 한다
    metrics: KeyboardMetrics,
    preferences: UserPreferences,
    pack: Option<Arc<dyn PackBytes>>,
    /// 후보 목록은 Engine이 소유한다 — 셸은 인덱스로 고르고, 학습·문맥 추적에 쓰는
    /// 조회 키는 표시 텍스트와 함께 여기에만 남는다
    suggestions: Vec<Suggestion>,
    /// 직전에 확정된 어휘의 조회 키 — 언어모델 문맥
    previous_word: Option<String>,
    /// 지금 어절에 대해 눌린 키 신호들. 조회 키의 **끝에서부터** 맞춰 쓴다 — 커서 이동
    /// 뒤 문맥에서 되가져온 앞부분에는 신호가 없으므로 조회 키보다 짧을 수 있다.
    touches: Vec<KeySignal>,
    /// 방금 자동교정으로 갈아치운 것 — 바로 뒤에 오는 Backspace는 이것을 되돌린다.
    /// 다음 입력이 하나라도 지나가면 사라진다(순정 키보드 관습).
    reverted_correction: Option<Correction>,
}

/// 자동교정이 갈아치운 내용. 되돌리기는 확정 텍스트를 원문으로 되돌려 놓기만 하고,
/// 이어지는 합성은 문맥 채택(adopt)이 알아서 잇는다.
struct Correction {
    /// 사용자가 실제로 친 형태
    original: String,
    /// 그 자리에 들어간 교정 결과와 경계 문자를 합친 확정 텍스트
    committed: String,
}

impl Engine {
    /// 이 빌드에 골격이 포함되지 않았으면 None — 셸은 해당 언어를 비활성 처리한다.
    pub fn new(language: LanguageDescriptor) -> Option<Self> {
        let composer = language.skeleton.composer()?;
        Some(Engine::with_composer(language, composer))
    }

    /// 언어의 기본 합성기 대신 다른 합성기를 꽂는다. 한 언어에 복수 배열·합성기를
    /// 두는 경우(인도계 음역↔네이티브 등)와 테스트가 쓰는 통로다.
    pub fn with_composer(language: LanguageDescriptor, composer: Box<dyn Composer>) -> Self {
        Engine {
            suggester: Suggester::new(language.suggestion_policy()),
            composer,
            keyboard: Keyboard::new(language.builtin_layout(), language.clone()),
            language,
            personalization: PersonalizationStore::new(),
            metrics: KeyboardMetrics::default(),
            preferences: UserPreferences::default(),
            pack: None,
            suggestions: Vec::new(),
            previous_word: None,
            touches: Vec::new(),
            reverted_correction: None,
        }
    }

    pub fn language(&self) -> &LanguageDescriptor {
        &self.language
    }

    /// 언어팩을 갈아 끼운다. 팩이 스스로 밝힌 선언(표시 이름·골격·조회 키 인코딩)이
    /// 내장 선언을 대신하고, 레이아웃 섹션이 있으면 배열도 함께 바뀐다 — 언어를
    /// 늘리는 일이 팩 배포로 끝나는 것은 이 갱신 덕분이다.
    pub fn load_pack(&mut self, pack: Arc<dyn PackBytes>) -> Result<(), PackError> {
        let opened = Pack::open(pack.bytes())?;
        let declared = LanguageDescriptor::from_pack(&opened);
        let layout = opened.layout();
        if let Some(declared) = declared {
            if declared.skeleton != self.language.skeleton
                && let Some(composer) = declared.skeleton.composer()
            {
                self.composer = composer;
            }
            self.suggester = Suggester::new(declared.suggestion_policy());
            self.language = declared;
        }
        let layout = layout.unwrap_or_else(|| self.language.builtin_layout());
        // 배열이 바뀌어도 셸이 알려 준 표시 환경과 필드 성격은 이어진다
        let field = self.keyboard.field();
        self.keyboard = Keyboard::new(layout, self.language.clone());
        self.keyboard.set_metrics(self.metrics);
        self.keyboard.set_field(field);
        self.pack = Some(pack);
        Ok(())
    }

    /// 사용자 설정 주입 — 셸이 설정 저장소에서 읽어 넣는다. 팩과 무관한 값이라
    /// 팩을 갈아 끼워도 유지되고, 설정 화면에서 바뀐 값은 다음 키보드 표시 때
    /// 이 호출로 반영된다.
    pub fn set_preferences(&mut self, preferences: UserPreferences) {
        self.preferences = preferences;
    }

    /// 표시 환경 주입 — 셸이 자기 크기를 알게 될 때(첫 배치, 회전, 분할) 부른다.
    pub fn set_metrics(&mut self, metrics: KeyboardMetrics) {
        self.metrics = metrics;
        self.keyboard.set_metrics(metrics);
    }

    /// 편집 대상이 바뀔 때 셸이 알려 주는 필드 성격. 보조 기능은 이벤트마다 오는
    /// `EditorContext`가 정하지만, 화면(배열·리턴키·후보 바 자리)은 이벤트 없이도
    /// 그려야 하므로 별도로 주입받는다.
    pub fn set_field(&mut self, field: FieldKind) {
        self.keyboard.set_field(field);
    }

    pub fn frame_metrics(&self) -> FrameMetrics {
        self.keyboard.frame_metrics()
    }

    pub fn frame(&self) -> KeyboardFrame {
        self.keyboard.frame()
    }

    /// 좌표에 있는 키 — 셸이 길게 누르기 대상을 알아내는 통로. 스냅 규칙이 탭과
    /// 같아야 하므로 코어 히트 테스트를 그대로 쓴다.
    pub fn key_at(&self, x: f32, y: f32) -> FrameKey {
        self.keyboard.key_at(x, y)
    }

    /// 터치 좌표 → 히트 테스트 → 합성까지 한 번에.
    pub fn press_at(&mut self, x: f32, y: f32, context: &EditorContext) -> PressResult {
        let outcome = self.keyboard.press_at(x, y);
        let effects = match outcome.event {
            Some(event) => self.handle(event, context),
            None => Vec::new(),
        };
        PressResult {
            effects,
            layout_changed: outcome.layout_changed,
            request: outcome.request,
        }
    }

    /// shift 키를 두 번 눌렀을 때(순정 관례) shift를 고정하거나 푼다. 고정할 수 없는
    /// 배열이면 false — 그때 셸은 두 번째 누름을 평범한 shift 토글로 남겨 둔다.
    pub fn toggle_shift_lock(&mut self) -> bool {
        self.keyboard.toggle_shift_lock()
    }

    /// 길게 눌러 연 팝업에서 고른 변형 문자 — 일반 키 입력과 같은 경로로 흐른다.
    pub fn select_alternate(&mut self, alternate: &str, context: &EditorContext) -> Vec<Effect> {
        match self.keyboard.select_alternate(alternate) {
            Some(event) => self.handle(event, context),
            None => Vec::new(),
        }
    }

    /// 스페이스바를 길게 눌러 끄는 커서 이동. 셸은 포인터 x(정규화)만 흘려보내고
    /// 몇 칸 움직일지는 코어가 판정한다.
    pub fn begin_cursor_drag(&mut self, x: f32) {
        self.keyboard.begin_cursor_drag(x);
    }

    pub fn update_cursor_drag(&mut self, x: f32, context: &EditorContext) -> Vec<Effect> {
        let steps = self.keyboard.update_cursor_drag(x);
        if steps == 0 {
            return Vec::new();
        }
        self.handle(InputEvent::CursorDrag(steps), context)
    }

    pub fn end_cursor_drag(&mut self) {
        self.keyboard.end_cursor_drag();
    }

    pub fn handle(&mut self, event: InputEvent, context: &EditorContext) -> Vec<Effect> {
        match event {
            InputEvent::CursorMoved | InputEvent::FocusLost => self.finalize_composition(),
            InputEvent::CursorDrag(steps) => {
                // 이동 전에 진행 중 composing을 언어별 규칙으로 확정한다 — 커서가
                // 빠져나간 자리에 조합 중 텍스트를 남기지 않는다.
                let mut effects = self.handle(InputEvent::CursorMoved, context);
                if steps != 0 {
                    effects.push(Effect::MoveCursor(steps));
                }
                effects
            }
            InputEvent::Key(signal) => {
                let character = signal.character();
                self.touches.push(signal);
                self.feed(ComposerEvent::Key(character), context, None)
            }
            // 여러 글자를 한 번에 넣는 키는 어절을 끊는다 — 조합 중이던 것을 언어별
            // 규칙으로 확정한 뒤에 넣어야 `.com`이 조합에 말려들지 않는다
            InputEvent::Text(text) => {
                let mut effects = self.finalize_composition();
                effects.push(Effect::CommitText(text));
                effects
            }
            InputEvent::Backspace => {
                if let Some(correction) = self.reverted_correction.take() {
                    return self.revert(correction);
                }
                self.touches.pop();
                self.feed(ComposerEvent::Backspace, context, None)
            }
            InputEvent::Separator(character) => {
                self.feed(ComposerEvent::Separator(character), context, None)
            }
            InputEvent::CandidateSelected(index) => {
                let Some(selected) = self.suggestions.get(index).cloned() else {
                    return Vec::new();
                };
                self.feed(
                    ComposerEvent::CandidateSelected(selected.text.clone()),
                    context,
                    Some(selected),
                )
            }
        }
    }

    /// 진행 중 조합을 언어별 규칙으로 확정하고 후보 바를 비운다. 커서가 옮겨 가거나
    /// 초점을 잃을 때, 그리고 검색면에서 곁들일 것을 골라 어절을 끝낼 때 함께 쓴다.
    fn finalize_composition(&mut self) -> Vec<Effect> {
        let was_composing = self.composer.is_composing();
        let mut effects = Vec::new();
        if let Some(committed) = self.composer.finalize() {
            effects.push(Effect::CommitText(committed.surface));
        }
        if was_composing {
            effects.push(Effect::ClearComposing);
        }
        // 어절이 여기서 끝났으므로 직전 어휘는 더 이상 문맥이 아니다
        self.previous_word = None;
        self.touches.clear();
        self.replace_suggestions(Vec::new(), &mut effects);
        effects
    }

    /// 통합 검색면 내용. `query`는 검색 필드에 친 표시 문자열이며, 비어 있으면 자주 쓰는
    /// 것과 갈래별 표시 순서를 낸다. 무엇을 어떤 순서로 보일지는 데이터(팩)와 개인화가
    /// 정하므로 셸은 그리기만 한다.
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
                groups.push(AnnotationPanelGroup {
                    group: None,
                    category: None,
                    label: PANEL_RECENT_LABEL.to_string(),
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
                        label: match section.category {
                            Some(category) => emoji_category_label(category),
                            None => panel_group_label(section.group),
                        }
                        .to_string(),
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
        for group in Self::accompanying_groups() {
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
                    label: panel_group_label(group).to_string(),
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
        let assistance = crate::policy::assistance(&self.preferences, context);
        let mut effects = self.finalize_composition();
        effects.push(Effect::CommitText(text.to_string()));
        if assistance.personalizing && !context.incognito {
            self.personalization.record_annotation(group, text);
        }
        effects
    }

    /// 낱말에 곁들이는 갈래들 — 검색면이 그룹으로 세우는 것은 이들뿐이다.
    fn accompanying_groups() -> impl Iterator<Item = CandidateGroup> {
        CandidateGroup::DISPLAY_ORDER
            .into_iter()
            .filter(|group| *group != CandidateGroup::Word)
    }

    /// 익스텐션 kill 대비 — 개인화 상태를 스냅샷해 컨테이너 저장소에 보관한다.
    pub fn personalization_snapshot(&self) -> PersonalizationState {
        self.personalization.snapshot()
    }

    pub fn restore_personalization(&mut self, state: PersonalizationState) {
        self.personalization = PersonalizationStore::restore(state);
    }

    /// 배운 것을 전부 잊는다 — 순정 키보드의 "키보드 사전 재설정"에 해당한다.
    /// 학습을 끄는 설정과 짝이다: 설정은 앞으로를 막고 이것은 지난 것을 지운다.
    pub fn reset_personalization(&mut self) {
        self.personalization = PersonalizationStore::new();
    }

    /// 합성기를 돌린 뒤 랭킹·자동교정·학습을 얹어 Effect로 옮긴다.
    /// `selected`는 후보 선택으로 어절이 끝난 경우의 그 후보다.
    fn feed(
        &mut self,
        event: ComposerEvent,
        context: &EditorContext,
        selected: Option<Suggestion>,
    ) -> Vec<Effect> {
        // Arc를 지역으로 복제해 팩 바이트 대여와 엔진의 가변 대여를 분리한다
        let holder = self.pack.clone();
        let pack = holder
            .as_ref()
            .and_then(|holder| Pack::open(holder.bytes()).ok());
        let was_composing = self.composer.is_composing();
        let output = self.compose(event, context);
        let assistance = crate::policy::assistance(&self.preferences, context);

        let ComposerOutput {
            mut delete_before_commit,
            commit,
            composing,
            boundary,
            suggest,
        } = output;
        let mut commit_text = commit.map(|text| text.surface).unwrap_or_default();

        // 되돌릴 교정은 바로 다음 입력까지만 유효하다 (순정 키보드 관습)
        self.reverted_correction = None;

        // 어절이 끝났는가 — 경계 문자를 쳤거나 후보를 골랐거나
        let confirmed = match boundary {
            Some(boundary) => {
                let correction = if assistance.correcting {
                    self.suggester
                        .autocorrection(&boundary.key, &self.sources(pack.as_ref(), assistance))
                } else {
                    None
                };
                let key = match correction {
                    Some(correction) => {
                        delete_before_commit += boundary.surface.chars().count();
                        commit_text.push_str(&correction.text);
                        self.reverted_correction = Some(Correction {
                            original: boundary.surface,
                            committed: format!("{}{}", correction.text, boundary.separator),
                        });
                        correction.key
                    }
                    None => boundary.key,
                };
                commit_text.push(boundary.separator);
                Some(key)
            }
            None => selected.map(|suggestion| suggestion.key),
        };

        let suggestions = match &confirmed {
            Some(key) => {
                // 어절이 끝났으므로 다음 어절은 새 터치 신호로 시작한다
                self.touches.clear();
                // 시크릿 필드는 학습만 막고 조회는 그대로 둔다 — 순정 키보드도
                // 이미 배운 말은 시크릿에서 계속 제안한다
                if assistance.personalizing && !context.incognito && !key.is_empty() {
                    self.personalization.record(key);
                }
                self.previous_word = (!key.is_empty()).then(|| key.clone());
                if assistance.predicting {
                    self.suggester
                        .predict_next(&self.sources(pack.as_ref(), assistance))
                } else {
                    Vec::new()
                }
            }
            None => match &suggest {
                SuggestionRequest::Word { key } if assistance.predicting => self
                    .suggester
                    .suggest(key, &self.sources(pack.as_ref(), assistance)),
                _ => Vec::new(),
            },
        };

        let mut effects = Vec::new();
        if delete_before_commit > 0 {
            effects.push(Effect::DeleteBackward(delete_before_commit));
        }
        if !commit_text.is_empty() {
            effects.push(Effect::CommitText(commit_text));
        }
        match composing {
            Some(composing) => effects.push(Effect::SetComposing(composing)),
            None if was_composing => effects.push(Effect::ClearComposing),
            None => {}
        }
        self.replace_suggestions(suggestions, &mut effects);
        effects
    }

    /// 합성기를 돌리기 전에 언어와 무관한 규칙을 먼저 본다. 지금은 더블 스페이스
    /// 마침표 하나뿐이고, 조합 중에는 성립할 수 없으므로 합성기 상태를 건드리지 않는다.
    fn compose(&mut self, event: ComposerEvent, context: &EditorContext) -> ComposerOutput {
        if event == ComposerEvent::Separator(' ')
            && self.preferences.double_space_period
            && !self.composer.is_composing()
            && let Some(output) = crate::policy::double_space_period(context)
        {
            return output;
        }
        self.composer.feed(event, context)
    }

    /// 자동교정 직후의 Backspace — 교정 결과를 지우고 사용자가 친 원문을 되살린다.
    /// 이어지는 타이핑은 문맥 채택(adopt)이 알아서 그 어절을 잇는다.
    fn revert(&mut self, correction: Correction) -> Vec<Effect> {
        self.previous_word = None;
        let mut effects = vec![
            Effect::DeleteBackward(correction.committed.chars().count()),
            Effect::CommitText(correction.original),
        ];
        self.replace_suggestions(Vec::new(), &mut effects);
        effects
    }

    fn sources<'call>(
        &'call self,
        pack: Option<&'call Pack<'call>>,
        assistance: Assistance,
    ) -> SuggestionSources<'call> {
        SuggestionSources {
            pack,
            personalization: assistance.personalizing.then_some(&self.personalization),
            previous_word: self.previous_word.as_deref(),
            touches: &self.touches,
        }
    }

    fn replace_suggestions(&mut self, suggestions: Vec<Suggestion>, effects: &mut Vec<Effect>) {
        if suggestions.is_empty() && self.suggestions.is_empty() {
            return;
        }
        let candidates = suggestions
            .iter()
            .map(|suggestion| Candidate {
                text: suggestion.text.clone(),
                kind: suggestion.kind.clone(),
                group: suggestion.group,
            })
            .collect();
        self.suggestions = suggestions;
        effects.push(Effect::UpdateCandidates(candidates));
    }
}
