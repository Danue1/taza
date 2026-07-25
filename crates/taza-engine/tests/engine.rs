use taza_engine::keyboard::KeySignal;
use std::sync::Arc;
use taza_engine::contract::{
    Candidate, CandidateKind, CommittedText, Composer, ComposerEvent, ComposerOutput,
    ComposerState, EditorContext, Effect, InputEvent, SuggestionRequest,
};
use taza_engine::engine::Engine;
use taza_engine::lang::LanguageDescriptor;
use taza_engine::lang::direct::DirectComposer;
use taza_engine::pack::SectionKind;
use taza_toolchain::PackWriter;
use taza_toolchain::lexicon::LexiconBuilder;

#[test]
fn direct_composer_commits_every_key() {
    let mut engine = Engine::with_composer(LanguageDescriptor::builtin("en").unwrap(), Box::new(DirectComposer::new()));
    let context = EditorContext::unavailable();
    assert_eq!(
        engine.handle(InputEvent::Key(KeySignal::certain('h')), &context),
        vec![Effect::CommitText("h".to_string())]
    );
    assert_eq!(
        engine.handle(InputEvent::Separator(' '), &context),
        vec![Effect::CommitText(" ".to_string())]
    );
    assert_eq!(
        engine.handle(InputEvent::Backspace, &context),
        vec![Effect::DeleteBackward(1)]
    );
}

/// 자동교정 제안 치환("teh" → "the")처럼 확정 텍스트를 지우고 다시 쓰는 Composer의
/// 출력이 Effect로 올바르게 번역되는지 검증하는 스텁. 후보 목록은 Engine이 사전에서
/// 만들므로 여기서는 조회 키만 낸다.
struct ReplacingComposer;

impl Composer for ReplacingComposer {
    fn feed(&mut self, event: ComposerEvent, _context: &EditorContext) -> ComposerOutput {
        match event {
            ComposerEvent::Key(_) => ComposerOutput {
                suggest: SuggestionRequest::Word {
                    key: "teh".to_string(),
                },
                ..ComposerOutput::default()
            },
            ComposerEvent::CandidateSelected(text) => ComposerOutput {
                delete_before_commit: 3,
                commit: Some(CommittedText::plain(text)),
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
        ComposerState::default()
    }

    fn restore(&mut self, _state: ComposerState) {}
}

#[test]
fn candidate_replacement_translates_to_delete_then_commit() {
    let mut lexicon = LexiconBuilder::new();
    lexicon.insert("the", 65535);
    let mut writer = PackWriter::new("en");
    writer.add_section(SectionKind::Lexicon, lexicon.build());

    let mut engine = Engine::with_composer(LanguageDescriptor::builtin("en").unwrap(), Box::new(ReplacingComposer));
    engine.load_pack(Arc::new(writer.finish())).unwrap();
    let context = EditorContext::unavailable();

    let effects = engine.handle(InputEvent::Key(KeySignal::certain('h')), &context);
    // 자동교정을 쓰는 언어이므로 교정 후보 뒤에 원문(as-typed)이 함께 붙는다
    assert_eq!(
        effects,
        vec![Effect::UpdateCandidates(vec![
            Candidate {
                text: "the".to_string(),
                kind: CandidateKind::Correction,
            },
            Candidate {
                text: "teh".to_string(),
                kind: CandidateKind::Prediction,
            },
        ])]
    );

    let effects = engine.handle(InputEvent::CandidateSelected(0), &context);
    assert_eq!(
        effects,
        vec![
            Effect::DeleteBackward(3),
            Effect::CommitText("the".to_string()),
            Effect::UpdateCandidates(Vec::new()),
        ]
    );
}

#[test]
fn cursor_drag_finalizes_composing_then_moves() {
    let mut engine = Engine::new(LanguageDescriptor::builtin("ko").unwrap()).unwrap();
    let context = EditorContext::unavailable();
    engine.handle(InputEvent::Key(KeySignal::certain('ㄱ')), &context);
    engine.handle(InputEvent::Key(KeySignal::certain('ㅏ')), &context);

    // 커서가 빠져나가기 전에 조합 중이던 음절을 확정하고, 그다음 이동한다
    let effects = engine.handle(InputEvent::CursorDrag(-3), &context);
    assert_eq!(
        effects,
        vec![
            Effect::CommitText("가".to_string()),
            Effect::ClearComposing,
            Effect::MoveCursor(-3),
        ]
    );

    // 조합 중이 아니면 이동만 나간다
    assert_eq!(
        engine.handle(InputEvent::CursorDrag(2), &context),
        vec![Effect::MoveCursor(2)]
    );
}
