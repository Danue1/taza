use crate::composer::{
    Candidate, Composer, ComposerEvent, ComposerOutput, ComposingText, EditorContext,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputEvent {
    Key(char),
    Backspace,
    Separator(char),
    CandidateSelected(usize),
    CursorMoved,
    FocusLost,
}

/// 셸이 플랫폼 API로 번역하는 선언적 명령. 셸은 번역만 하고 판단하지 않는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// 활성 composing 구간이 있으면 그것을 치환하며 확정한다
    /// (iOS insertText / Android commitText의 공통 의미론)
    CommitText(String),
    SetComposing(ComposingText),
    ClearComposing,
    /// 코드포인트 수. iOS deleteBackward는 count 미보장이므로 셸은 적용 후 문맥 재동기화 필요
    DeleteBackward(usize),
    UpdateCandidates(Vec<Candidate>),
}

pub struct Session {
    composer: Box<dyn Composer>,
    showing_candidates: bool,
}

impl Session {
    pub fn new(composer: Box<dyn Composer>) -> Self {
        Session {
            composer,
            showing_candidates: false,
        }
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

    fn feed(&mut self, event: ComposerEvent, context: &EditorContext) -> Vec<Effect> {
        let was_composing = self.composer.is_composing();
        let output = self.composer.feed(event, context);
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
