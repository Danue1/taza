//! 코어의 최상위 조립 지점. 키보드 상태·합성기·개인화·언어팩을 한 객체가 소유하므로
//! 셸(taza-ffi)은 타입 번역만 하고 조립하지 않는다.
//!
//! sans-io는 유지된다 — 파일을 여는 일은 셸의 몫이고, 코어는 이미 열린 바이트만 받는다.

use std::sync::Arc;

use crate::contract::{
    Candidate, Composer, ComposerEnvironment, ComposerEvent, ComposerOutput, EditorContext, Effect,
    InputEvent, Pack,
};
use crate::keyboard::{
    FrameMetrics, Keyboard, KeyboardFrame, KeyboardMetrics, FrameKey, ShellRequest,
};
use crate::lang::Language;
use crate::pack::PackError;
use crate::personalization::{PersonalizationState, PersonalizationStore};

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
    language: Language,
    composer: Box<dyn Composer>,
    keyboard: Keyboard,
    personalization: PersonalizationStore,
    /// 팩 교체로 키보드를 다시 만들어도 셸이 주입한 표시 환경은 이어져야 한다
    metrics: KeyboardMetrics,
    pack: Option<Arc<dyn PackBytes>>,
    showing_candidates: bool,
}

impl Engine {
    /// 이 빌드에 언어가 포함되지 않았으면 None — 셸은 해당 언어를 비활성 처리한다.
    pub fn new(language: Language) -> Option<Self> {
        Some(Engine::with_composer(language, language.composer()?))
    }

    /// 언어의 기본 합성기 대신 다른 합성기를 꽂는다. 한 언어에 복수 배열·합성기를
    /// 두는 경우(인도계 음역↔네이티브 등)와 테스트가 쓰는 통로다.
    pub fn with_composer(language: Language, composer: Box<dyn Composer>) -> Self {
        Engine {
            language,
            composer,
            keyboard: Keyboard::new(language.builtin_layout(), language),
            personalization: PersonalizationStore::new(),
            metrics: KeyboardMetrics::default(),
            pack: None,
            showing_candidates: false,
        }
    }

    pub fn language(&self) -> Language {
        self.language
    }

    /// 언어팩을 갈아 끼운다. 레이아웃 섹션이 있으면 배열도 함께 바뀐다.
    pub fn load_pack(&mut self, pack: Arc<dyn PackBytes>) -> Result<(), PackError> {
        let layout = Pack::open(pack.bytes())?.layout();
        if let Some(layout) = layout {
            self.keyboard = Keyboard::new(layout, self.language);
            self.keyboard.set_metrics(self.metrics);
        }
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
                if self.showing_candidates {
                    self.showing_candidates = false;
                    effects.push(Effect::UpdateCandidates(Vec::new()));
                }
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
            InputEvent::Key(character) => self.feed(ComposerEvent::Key(character), context),
            InputEvent::Backspace => self.feed(ComposerEvent::Backspace, context),
            InputEvent::Separator(character) => {
                self.feed(ComposerEvent::Separator(character), context)
            }
            InputEvent::CandidateSelected(index) => {
                self.feed(ComposerEvent::CandidateSelected(index), context)
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

    fn feed(&mut self, event: ComposerEvent, context: &EditorContext) -> Vec<Effect> {
        // Arc를 지역으로 복제해 팩 바이트 대여와 합성기·개인화 가변 대여를 분리한다
        let holder = self.pack.clone();
        let pack = holder
            .as_ref()
            .and_then(|holder| Pack::open(holder.bytes()).ok());
        let was_composing = self.composer.is_composing();
        let mut environment = ComposerEnvironment {
            context,
            pack: pack.as_ref(),
            personalization: &mut self.personalization,
        };
        let output = self.composer.feed(event, &mut environment);
        self.translate(output, was_composing)
    }

    fn translate(&mut self, output: ComposerOutput, was_composing: bool) -> Vec<Effect> {
        let mut effects = Vec::new();
        if output.delete_before_commit > 0 {
            effects.push(Effect::DeleteBackward(output.delete_before_commit));
        }
        if let Some(committed) = output.commit {
            effects.push(Effect::CommitText(committed.surface));
        }
        match output.composing {
            Some(composing) => effects.push(Effect::SetComposing(composing)),
            None if was_composing => effects.push(Effect::ClearComposing),
            None => {}
        }
        self.update_candidates(output.candidates, &mut effects);
        effects
    }

    fn update_candidates(&mut self, candidates: Vec<Candidate>, effects: &mut Vec<Effect>) {
        if !candidates.is_empty() {
            self.showing_candidates = true;
            effects.push(Effect::UpdateCandidates(candidates));
        } else if self.showing_candidates {
            self.showing_candidates = false;
            effects.push(Effect::UpdateCandidates(Vec::new()));
        }
    }
}
