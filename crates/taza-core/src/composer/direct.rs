use super::{
    CommittedText, Composer, ComposerEvent, ComposerOutput, ComposerState, EditorContext,
};

/// composing 없이 즉시 확정하는 골격 — 라틴 전반. 악센트는 레이아웃의 롱프레스 팝업이,
/// 교정·예측은 자동교정 엔진이 담당하므로 여기서는 다루지 않는다.
#[derive(Debug, Default)]
pub struct DirectComposer;

impl DirectComposer {
    pub fn new() -> Self {
        DirectComposer
    }
}

impl Composer for DirectComposer {
    fn feed(&mut self, event: ComposerEvent, _context: &EditorContext) -> ComposerOutput {
        match event {
            ComposerEvent::Key(character) | ComposerEvent::Separator(character) => {
                ComposerOutput {
                    commit: Some(CommittedText::plain(character.to_string())),
                    ..ComposerOutput::default()
                }
            }
            ComposerEvent::Backspace => ComposerOutput {
                delete_before_commit: 1,
                ..ComposerOutput::default()
            },
            ComposerEvent::CandidateSelected(_) => ComposerOutput::default(),
        }
    }

    fn finalize(&mut self) -> Option<CommittedText> {
        None
    }

    fn is_composing(&self) -> bool {
        false
    }

    fn snapshot(&self) -> ComposerState {
        ComposerState::Direct
    }

    fn restore(&mut self, _state: ComposerState) {}
}
