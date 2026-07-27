//! 코어와 셸이 주고받는 경계 타입 전부. 여기에는 타입과 trait만 두고, 판단 규칙은
//! `policy`에, 조립은 `engine`에 둔다.
//!
//! 네 갈래로 나뉜다. `shell`(셸이 주입하는 값) · `event`(입력과 Effect) ·
//! `candidate`(후보·검색면)는 FFI를 건너가 셸에서 거울 타입을 갖고, `composer`는
//! 건너가지 않는 코어 안쪽 계약이다.

pub mod candidate;
pub mod composer;
pub mod event;
pub mod shell;

pub use candidate::{
    AnnotationPanel, AnnotationPanelGroup, AnnotationPanelItem, Candidate, CandidateGroup,
    CandidateKind, EmojiCategory,
};
pub use composer::{
    CommittedText, Composer, ComposerEvent, ComposerOutput, ComposerState, ComposingText,
    SuggestionRequest, WordBoundary,
};
pub use event::{Effect, InputEvent};
pub use shell::{
    Capitalization, CursorSensitivity, EditorContext, FieldKind, FieldTraits, KeyboardHeight,
    ReturnKey, UserPreferences,
};

pub use crate::keyboard::KeySignal;
pub use crate::pack::Pack;
