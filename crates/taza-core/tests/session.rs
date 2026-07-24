use taza_core::composer::direct::DirectComposer;
use taza_core::composer::{
    Candidate, CandidateKind, CommittedText, Composer, ComposerEnvironment, ComposerEvent,
    ComposerOutput, ComposerState, EditorContext,
};
use taza_core::session::{Effect, InputEvent, Session};

#[test]
fn direct_composer_commits_every_key() {
    let mut session = Session::new(Box::new(DirectComposer::new()));
    let context = EditorContext::unavailable();
    assert_eq!(
        session.handle(InputEvent::Key('h'), &context, None),
        vec![Effect::CommitText("h".to_string())]
    );
    assert_eq!(
        session.handle(InputEvent::Separator(' '), &context, None),
        vec![Effect::CommitText(" ".to_string())]
    );
    assert_eq!(
        session.handle(InputEvent::Backspace, &context, None),
        vec![Effect::DeleteBackward(1)]
    );
}

/// 자동교정 제안 치환("teh" → "the")처럼 확정 텍스트를 지우고 다시 쓰는 Composer의
/// 출력이 Effect로 올바르게 번역되는지 검증하는 스텁
struct ReplacingComposer;

impl Composer for ReplacingComposer {
    fn feed(
        &mut self,
        event: ComposerEvent,
        _environment: &mut ComposerEnvironment<'_>,
    ) -> ComposerOutput {
        match event {
            ComposerEvent::Key(_) => ComposerOutput {
                candidates: vec![Candidate {
                    text: "the".to_string(),
                    kind: CandidateKind::Correction,
                }],
                ..ComposerOutput::default()
            },
            ComposerEvent::CandidateSelected(_) => ComposerOutput {
                delete_before_commit: 3,
                commit: Some(CommittedText {
                    surface: "the".to_string(),
                    reading: None,
                    corrected_from: Some("teh".to_string()),
                }),
                ..ComposerOutput::default()
            },
            _ => ComposerOutput::default(),
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

#[test]
fn candidate_replacement_translates_to_delete_then_commit() {
    let mut session = Session::new(Box::new(ReplacingComposer));
    let context = EditorContext::unavailable();

    let effects = session.handle(InputEvent::Key('h'), &context, None);
    assert_eq!(
        effects,
        vec![Effect::UpdateCandidates(vec![Candidate {
            text: "the".to_string(),
            kind: CandidateKind::Correction,
        }])]
    );

    let effects = session.handle(InputEvent::CandidateSelected(0), &context, None);
    assert_eq!(
        effects,
        vec![
            Effect::DeleteBackward(3),
            Effect::CommitText("the".to_string()),
            Effect::UpdateCandidates(Vec::new()),
        ]
    );
}
