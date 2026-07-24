//! 플랫폼 셸(Swift/Kotlin)이 소비하는 FFI 표면. 코어는 sans-io를 유지하고,
//! 파일 IO(팩 mmap)는 이 계층이 담당한다. 이벤트당 1회 왕복 계약을 그대로 노출한다.

use std::sync::Mutex;

use taza_core::composer::hangul::HangulComposer;
use taza_core::composer::latin::LatinComposer;
use taza_core::composer::{Candidate, CandidateKind, EditorContext, FieldKind};
use taza_core::keyboard::{Keyboard, layouts};
use taza_core::session::{Effect, InputEvent, Session};
use taza_pack::Pack;

uniffi::setup_scaffolding!();

#[derive(uniffi::Enum)]
pub enum FfiLanguage {
    English,
    Korean,
}

#[derive(uniffi::Enum)]
pub enum FfiFieldKind {
    Text,
    Email,
    Url,
    Number,
    Phone,
    Password,
}

#[derive(uniffi::Record)]
pub struct FfiEditorContext {
    pub text_before_cursor: Option<String>,
    pub incognito: bool,
    pub field: FfiFieldKind,
}

#[derive(uniffi::Enum)]
pub enum FfiInputEvent {
    Key { character: String },
    Backspace,
    Separator { character: String },
    CandidateSelected { index: u32 },
    CursorMoved,
    FocusLost,
}

#[derive(uniffi::Enum)]
pub enum FfiCandidateKind {
    Prediction,
    Conversion,
    Correction,
}

#[derive(uniffi::Record)]
pub struct FfiCandidate {
    pub text: String,
    pub kind: FfiCandidateKind,
}

#[derive(uniffi::Enum)]
pub enum FfiEffect {
    CommitText { text: String },
    SetComposing { text: String, caret: u32 },
    ClearComposing,
    DeleteBackward { code_points: u32 },
    UpdateCandidates { candidates: Vec<FfiCandidate> },
}

#[derive(uniffi::Record)]
pub struct FfiKeyBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(uniffi::Record)]
pub struct FfiFrameKey {
    pub row: u32,
    pub index: u32,
    pub label: String,
    pub accessibility_label: String,
    pub bounds: FfiKeyBounds,
    pub shift_active: bool,
}

#[derive(uniffi::Record)]
pub struct FfiKeyboardFrame {
    pub rows: Vec<Vec<FfiFrameKey>>,
}

#[derive(uniffi::Record)]
pub struct FfiPressResult {
    pub effects: Vec<FfiEffect>,
    pub layout_changed: bool,
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum FfiPackError {
    #[error("팩 파일을 열 수 없음: {message}")]
    Io { message: String },
    #[error("팩 형식 오류: {message}")]
    Invalid { message: String },
}

fn convert_event(event: FfiInputEvent) -> Option<InputEvent> {
    Some(match event {
        FfiInputEvent::Key { character } => InputEvent::Key(character.chars().next()?),
        FfiInputEvent::Backspace => InputEvent::Backspace,
        FfiInputEvent::Separator { character } => {
            InputEvent::Separator(character.chars().next()?)
        }
        FfiInputEvent::CandidateSelected { index } => {
            InputEvent::CandidateSelected(index as usize)
        }
        FfiInputEvent::CursorMoved => InputEvent::CursorMoved,
        FfiInputEvent::FocusLost => InputEvent::FocusLost,
    })
}

fn convert_candidate(candidate: Candidate) -> FfiCandidate {
    FfiCandidate {
        text: candidate.text,
        kind: match candidate.kind {
            CandidateKind::Prediction => FfiCandidateKind::Prediction,
            CandidateKind::Conversion => FfiCandidateKind::Conversion,
            CandidateKind::Correction => FfiCandidateKind::Correction,
        },
    }
}

fn convert_effect(effect: Effect) -> FfiEffect {
    match effect {
        Effect::CommitText(text) => FfiEffect::CommitText { text },
        Effect::SetComposing(composing) => FfiEffect::SetComposing {
            text: composing.text,
            caret: composing.caret as u32,
        },
        Effect::ClearComposing => FfiEffect::ClearComposing,
        Effect::DeleteBackward(count) => FfiEffect::DeleteBackward {
            code_points: count as u32,
        },
        Effect::UpdateCandidates(candidates) => FfiEffect::UpdateCandidates {
            candidates: candidates.into_iter().map(convert_candidate).collect(),
        },
    }
}

fn convert_context(context: &FfiEditorContext) -> EditorContext {
    EditorContext {
        text_before_cursor: context.text_before_cursor.clone(),
        incognito: context.incognito,
        field: match context.field {
            FfiFieldKind::Text => FieldKind::Text,
            FfiFieldKind::Email => FieldKind::Email,
            FfiFieldKind::Url => FieldKind::Url,
            FfiFieldKind::Number => FieldKind::Number,
            FfiFieldKind::Phone => FieldKind::Phone,
            FfiFieldKind::Password => FieldKind::Password,
        },
    }
}

struct SessionState {
    session: Session,
    keyboard: Keyboard,
    pack_bytes: Option<memmap2::Mmap>,
}

impl SessionState {
    fn with_pack<Output>(
        &mut self,
        operation: impl FnOnce(&mut Session, Option<&Pack<'_>>) -> Output,
    ) -> Output {
        match &self.pack_bytes {
            Some(bytes) => match Pack::open(bytes) {
                Ok(pack) => operation(&mut self.session, Some(&pack)),
                Err(_) => operation(&mut self.session, None),
            },
            None => operation(&mut self.session, None),
        }
    }
}

/// 키보드 익스텐션 프로세스당 하나 — 셸은 이 객체 하나로 입력·화면·팩을 오간다.
#[derive(uniffi::Object)]
pub struct KeyboardSession {
    state: Mutex<SessionState>,
}

#[uniffi::export]
impl KeyboardSession {
    #[uniffi::constructor]
    pub fn new(language: FfiLanguage) -> Self {
        let (composer, layout): (Box<dyn taza_core::composer::Composer>, _) = match language {
            FfiLanguage::English => (Box::new(LatinComposer::new()), layouts::qwerty()),
            FfiLanguage::Korean => (Box::new(HangulComposer::new()), layouts::dubeolsik()),
        };
        KeyboardSession {
            state: Mutex::new(SessionState {
                session: Session::new(composer),
                keyboard: Keyboard::new(layout),
                pack_bytes: None,
            }),
        }
    }

    /// 언어팩 파일을 mmap으로 연다. 파일 백드 clean page라 익스텐션 메모리
    /// 예산(jetsam footprint)에 산입되지 않는다. 레이아웃 섹션이 있으면 교체한다.
    pub fn load_pack(&self, path: String) -> Result<(), FfiPackError> {
        let file = std::fs::File::open(&path).map_err(|error| FfiPackError::Io {
            message: error.to_string(),
        })?;
        let bytes = unsafe { memmap2::Mmap::map(&file) }.map_err(|error| FfiPackError::Io {
            message: error.to_string(),
        })?;
        let pack = Pack::open(&bytes).map_err(|error| FfiPackError::Invalid {
            message: error.to_string(),
        })?;
        let layout = pack.layout();
        let mut state = self.state.lock().unwrap();
        if let Some(layout) = layout {
            state.keyboard = Keyboard::new(layout);
        }
        state.pack_bytes = Some(bytes);
        Ok(())
    }

    /// 이벤트당 1회 왕복 — 반환된 Effect 목록을 셸이 순서대로 플랫폼 API로 번역한다.
    pub fn handle_event(
        &self,
        event: FfiInputEvent,
        context: FfiEditorContext,
    ) -> Vec<FfiEffect> {
        let Some(input_event) = convert_event(event) else {
            return Vec::new();
        };
        let core_context = convert_context(&context);
        let mut state = self.state.lock().unwrap();
        state
            .with_pack(|session, pack| session.handle(input_event, &core_context, pack))
            .into_iter()
            .map(convert_effect)
            .collect()
    }

    /// 터치 좌표(정규화) → 코어 히트 테스트 → 세션 처리까지 한 번에.
    pub fn press_at(
        &self,
        x: f32,
        y: f32,
        context: FfiEditorContext,
    ) -> FfiPressResult {
        let core_context = convert_context(&context);
        let mut state = self.state.lock().unwrap();
        let outcome = state.keyboard.press_at(x, y);
        let effects = match outcome.event {
            Some(event) => state
                .with_pack(|session, pack| session.handle(event, &core_context, pack))
                .into_iter()
                .map(convert_effect)
                .collect(),
            None => Vec::new(),
        };
        FfiPressResult {
            effects,
            layout_changed: outcome.layout_changed,
        }
    }

    pub fn keyboard_frame(&self) -> FfiKeyboardFrame {
        let state = self.state.lock().unwrap();
        FfiKeyboardFrame {
            rows: state
                .keyboard
                .frame()
                .rows
                .into_iter()
                .map(|row| {
                    row.into_iter()
                        .map(|key| FfiFrameKey {
                            row: key.position.row as u32,
                            index: key.position.index as u32,
                            label: key.label,
                            accessibility_label: key.accessibility_label,
                            bounds: FfiKeyBounds {
                                x: key.bounds.x,
                                y: key.bounds.y,
                                width: key.bounds.width,
                                height: key.bounds.height,
                            },
                            shift_active: key.shift_active,
                        })
                        .collect()
                })
                .collect(),
        }
    }

    /// 개인화 상태 직렬화 — 셸이 컨테이너 저장소(App Group 등)에 보관한다.
    pub fn personalization_snapshot(&self) -> Vec<String> {
        let state = self.state.lock().unwrap();
        let snapshot = state.session.personalization_snapshot();
        let mut lines = vec![snapshot.clock.to_string()];
        for (word, count, last_used) in snapshot.entries {
            lines.push(format!("{word}\t{count}\t{last_used}"));
        }
        lines
    }

    pub fn restore_personalization(&self, lines: Vec<String>) {
        let Some((clock_line, entry_lines)) = lines.split_first() else {
            return;
        };
        let Ok(clock) = clock_line.parse() else {
            return;
        };
        let mut entries = Vec::new();
        for line in entry_lines {
            let fields: Vec<&str> = line.split('\t').collect();
            let [word, count, last_used] = fields.as_slice() else {
                continue;
            };
            let (Ok(count), Ok(last_used)) = (count.parse(), last_used.parse()) else {
                continue;
            };
            entries.push((word.to_string(), count, last_used));
        }
        let mut state = self.state.lock().unwrap();
        state
            .session
            .restore_personalization(taza_core::personalization::PersonalizationState {
                entries,
                clock,
            });
    }
}
