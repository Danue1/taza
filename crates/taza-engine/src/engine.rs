//! 코어의 최상위 조립 지점. 키보드 상태·합성기·개인화·언어팩을 한 객체가 소유하므로
//! 셸(taza-ffi)은 타입 번역만 하고 조립하지 않는다.
//!
//! sans-io는 유지된다 — 파일을 여는 일은 셸의 몫이고, 코어는 이미 열린 바이트만 받는다.

use std::sync::Arc;

use crate::contract::{
    Candidate, Composer, ComposerEvent, ComposerOutput, EditorContext, Effect, InputEvent, Pack,
    SuggestionRequest,
};
use crate::keyboard::{
    FrameKey, FrameMetrics, Keyboard, KeyboardFrame, KeyboardMetrics, KeySignal, ShellRequest,
};
use crate::lang::LanguageDescriptor;
use crate::pack::PackError;
use crate::personalization::{PersonalizationState, PersonalizationStore};
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
    pack: Option<Arc<dyn PackBytes>>,
    /// 후보 목록은 Engine이 소유한다 — 셸은 인덱스로 고르고, 학습·문맥 추적에 쓰는
    /// 조회 키는 표시 텍스트와 함께 여기에만 남는다
    suggestions: Vec<Suggestion>,
    /// 직전에 확정된 어휘의 조회 키 — 언어모델 문맥
    previous_word: Option<String>,
    /// 지금 어절에 대해 눌린 키 신호들. 조회 키의 **끝에서부터** 맞춰 쓴다 — 커서 이동
    /// 뒤 문맥에서 되가져온 앞부분에는 신호가 없으므로 조회 키보다 짧을 수 있다.
    touches: Vec<KeySignal>,
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
            pack: None,
            suggestions: Vec::new(),
            previous_word: None,
            touches: Vec::new(),
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
        self.keyboard = Keyboard::new(layout, self.language.clone());
        self.keyboard.set_metrics(self.metrics);
        self.pack = Some(pack);
        Ok(())
    }

    /// 표시 환경 주입 — 셸이 자기 크기를 알게 될 때(첫 배치, 회전, 분할) 부른다.
    pub fn set_metrics(&mut self, metrics: KeyboardMetrics) {
        self.metrics = metrics;
        self.keyboard.set_metrics(metrics);
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
            InputEvent::CursorMoved | InputEvent::FocusLost => {
                let was_composing = self.composer.is_composing();
                let mut effects = Vec::new();
                if let Some(committed) = self.composer.finalize() {
                    effects.push(Effect::CommitText(committed.surface));
                }
                if was_composing {
                    effects.push(Effect::ClearComposing);
                }
                // 커서가 옮겨 갔으므로 직전 어휘는 더 이상 문맥이 아니다
                self.previous_word = None;
                self.touches.clear();
                self.replace_suggestions(Vec::new(), &mut effects);
                effects
            }
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
            InputEvent::Backspace => {
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

    /// 익스텐션 kill 대비 — 개인화 상태를 스냅샷해 컨테이너 저장소에 보관한다.
    pub fn personalization_snapshot(&self) -> PersonalizationState {
        self.personalization.snapshot()
    }

    pub fn restore_personalization(&mut self, state: PersonalizationState) {
        self.personalization = PersonalizationStore::restore(state);
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
        let output = self.composer.feed(event, context);
        let assistance = context.field.assistance_enabled();

        let ComposerOutput {
            mut delete_before_commit,
            commit,
            composing,
            boundary,
            suggest,
        } = output;
        let mut commit_text = commit.map(|text| text.surface).unwrap_or_default();

        // 어절이 끝났는가 — 경계 문자를 쳤거나 후보를 골랐거나
        let confirmed = match boundary {
            Some(boundary) => {
                let correction = if assistance {
                    self.suggester
                        .autocorrection(&boundary.key, &self.sources(pack.as_ref()))
                } else {
                    None
                };
                let key = match correction {
                    Some(correction) => {
                        delete_before_commit += boundary.surface.chars().count();
                        commit_text.push_str(&correction.text);
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
                if assistance && !context.incognito && !key.is_empty() {
                    self.personalization.record(key);
                }
                self.previous_word = (!key.is_empty()).then(|| key.clone());
                if assistance {
                    self.suggester.predict_next(&self.sources(pack.as_ref()))
                } else {
                    Vec::new()
                }
            }
            None => match &suggest {
                SuggestionRequest::Word { key } if assistance => {
                    self.suggester.suggest(key, &self.sources(pack.as_ref()))
                }
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

    fn sources<'call>(&'call self, pack: Option<&'call Pack<'call>>) -> SuggestionSources<'call> {
        SuggestionSources {
            pack,
            personalization: &self.personalization,
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
            })
            .collect();
        self.suggestions = suggestions;
        effects.push(Effect::UpdateCandidates(candidates));
    }
}
