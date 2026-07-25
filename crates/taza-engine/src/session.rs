use crate::contract::{
    Candidate, Composer, ComposerEnvironment, ComposerEvent, ComposerOutput, ComposingText,
    EditorContext, Pack,
};
use crate::personalization::{PersonalizationState, PersonalizationStore};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputEvent {
    Key(char),
    Backspace,
    Separator(char),
    CandidateSelected(usize),
    CursorMoved,
    /// 스페이스바를 길게 눌러 끄는 커서 이동. 값은 논리적 이동 칸수(부호 = 방향)로,
    /// 코어가 포인터 이동량에서 산출한다.
    CursorDrag(i32),
    FocusLost,
}

/// 셸이 플랫폼 API로 번역하는 선언적 명령. 셸은 번역만 하고 판단하지 않는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// 활성 composing 구간이 있으면 그것을 치환하며 확정한다
    /// (iOS insertText / Android commitText의 공통 의미론)
    CommitText(String),
    SetComposing(ComposingText),
    /// composing 구간의 텍스트를 제거하고 composing 상태를 끝낸다.
    /// 주의: iOS unmarkText / Android finishComposingText는 "확정"이므로 그대로 쓰면
    /// 안 된다 — 빈 문자열로 치환 후 종료해야 한다 (셸 계약).
    ClearComposing,
    /// 코드포인트 수. iOS deleteBackward는 count 미보장이므로 셸은 적용 후 문맥 재동기화 필요
    DeleteBackward(usize),
    UpdateCandidates(Vec<Candidate>),
    /// 커서를 논리적으로 옮긴다(부호 = 방향, 단위 = 코드포인트).
    /// RTL에서도 의미는 "논리적 이동"으로 고정 — 시각적 방향은 플랫폼이 해석한다.
    MoveCursor(i32),
}

pub struct Session {
    composer: Box<dyn Composer>,
    showing_candidates: bool,
    personalization: PersonalizationStore,
}

impl Session {
    pub fn new(composer: Box<dyn Composer>) -> Self {
        Session {
            composer,
            showing_candidates: false,
            personalization: PersonalizationStore::new(),
        }
    }

    /// 익스텐션 kill 대비 — 개인화 상태를 스냅샷해 컨테이너 저장소에 보관한다.
    pub fn personalization_snapshot(&self) -> PersonalizationState {
        self.personalization.snapshot()
    }

    pub fn restore_personalization(&mut self, state: PersonalizationState) {
        self.personalization = PersonalizationStore::restore(state);
    }

    pub fn handle(
        &mut self,
        event: InputEvent,
        context: &EditorContext,
        pack: Option<&Pack<'_>>,
    ) -> Vec<Effect> {
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
                let mut effects = self.handle(InputEvent::CursorMoved, context, pack);
                if steps != 0 {
                    effects.push(Effect::MoveCursor(steps));
                }
                effects
            }
            InputEvent::Key(character) => self.feed(ComposerEvent::Key(character), context, pack),
            InputEvent::Backspace => self.feed(ComposerEvent::Backspace, context, pack),
            InputEvent::Separator(character) => {
                self.feed(ComposerEvent::Separator(character), context, pack)
            }
            InputEvent::CandidateSelected(index) => {
                self.feed(ComposerEvent::CandidateSelected(index), context, pack)
            }
        }
    }

    fn feed(
        &mut self,
        event: ComposerEvent,
        context: &EditorContext,
        pack: Option<&Pack<'_>>,
    ) -> Vec<Effect> {
        let was_composing = self.composer.is_composing();
        let mut environment = ComposerEnvironment {
            context,
            pack,
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
        if !output.candidates.is_empty() {
            self.showing_candidates = true;
            effects.push(Effect::UpdateCandidates(output.candidates));
        } else if self.showing_candidates {
            self.showing_candidates = false;
            effects.push(Effect::UpdateCandidates(Vec::new()));
        }
        effects
    }
}
