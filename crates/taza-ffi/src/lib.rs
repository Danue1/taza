//! 플랫폼 셸(Swift/Kotlin)이 소비하는 FFI 표면. 코어는 sans-io를 유지하고,
//! 파일 IO(팩 mmap·팩 설치)는 이 계층이 담당한다. 이벤트당 1회 왕복 계약을 그대로
//! 노출한다. 네트워크는 셸의 일이다 — 이 계층은 이미 내려받은 바이트만 다룬다.

mod install;

pub use install::{
    FfiInstallError, FfiInstalledPack, install_pack_archive, read_installed_pack,
    supported_pack_format_version,
};

use std::sync::{Arc, Mutex};

use taza_engine::contract::{Candidate, CandidateKind, EditorContext, Effect, FieldKind, InputEvent};
use taza_engine::engine::{Engine, PackBytes};
use taza_engine::keyboard::{FormFactor, KeyRole, KeyboardMetrics, ShellRequest};
use taza_engine::lang::LanguageDescriptor;
use taza_engine::personalization::PersonalizationState;

uniffi::setup_scaffolding!();

/// 코어가 알려 주는 언어 선언. 표시 이름·키캡 표기는 팩이 밝히므로 셸은 이 값을
/// 그대로 쓰고 자기 표를 따로 두지 않는다.
#[derive(uniffi::Record)]
pub struct FfiLanguageDescriptor {
    pub tag: String,
    pub display_name: String,
    pub keycap_label: String,
    pub layout_name: String,
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
    MoveCursor { offset: i32 },
}

#[derive(uniffi::Record)]
pub struct FfiKeyBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// 셸이 길게 누르기 같은 플랫폼 관습을 붙일 때 쓰는 갈래 (화이트리스트 분기)
#[derive(uniffi::Enum)]
pub enum FfiKeyRole {
    Character,
    Shift,
    Backspace,
    Space,
    Enter,
    LayerSwitch,
    LanguageSwitch,
}

#[derive(uniffi::Record)]
pub struct FfiFrameKey {
    pub row: u32,
    pub index: u32,
    pub label: String,
    pub accessibility_label: String,
    pub bounds: FfiKeyBounds,
    pub shift_active: bool,
    pub role: FfiKeyRole,
    pub alternates: Vec<String>,
}

/// 셸이 판별해 넘기는 표시 폼팩터 — 플랫폼 값(size class 등)의 번역일 뿐이고,
/// 이 갈래로 무엇을 할지는 코어가 정한다.
#[derive(uniffi::Enum)]
pub enum FfiFormFactor {
    PhonePortrait,
    PhoneLandscape,
    Tablet,
}

/// 코어가 정한 실측 치수(pt). 셸은 그대로 제약·글꼴에 쓴다.
#[derive(uniffi::Record)]
pub struct FfiFrameMetrics {
    /// 키 그리드 높이 — 키의 정규화 높이에 곱하면 실제 높이다
    pub grid_height: f32,
    pub candidate_bar_height: f32,
    /// 후보 바까지 포함한 입력 뷰 전체 높이
    pub total_height: f32,
    pub letter_font_size: f32,
    pub control_font_size: f32,
}

#[derive(uniffi::Record)]
pub struct FfiKeyboardFrame {
    pub rows: Vec<Vec<FfiFrameKey>>,
    pub metrics: FfiFrameMetrics,
}

#[derive(uniffi::Record)]
pub struct FfiPressResult {
    pub effects: Vec<FfiEffect>,
    pub layout_changed: bool,
    /// 코어가 셸에 낸 요청 — 언어 목록·순서는 셸이 소유하므로 전환 자체는 셸이 한다
    pub requests_next_language: bool,
}

/// 빌드에 포함되지 않은 언어를 요청한 경우 — 셸은 해당 언어를 목록에서 제외한다.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum FfiLanguageError {
    #[error("이 빌드에 포함되지 않은 언어")]
    Unsupported,
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
        FfiInputEvent::Separator { character } => InputEvent::Separator(character.chars().next()?),
        FfiInputEvent::CandidateSelected { index } => InputEvent::CandidateSelected(index as usize),
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
        Effect::MoveCursor(offset) => FfiEffect::MoveCursor { offset },
    }
}

fn convert_frame_key(key: taza_engine::keyboard::FrameKey) -> FfiFrameKey {
    FfiFrameKey {
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
        role: match key.role {
            KeyRole::Character => FfiKeyRole::Character,
            KeyRole::Shift => FfiKeyRole::Shift,
            KeyRole::Backspace => FfiKeyRole::Backspace,
            KeyRole::Space => FfiKeyRole::Space,
            KeyRole::Enter => FfiKeyRole::Enter,
            KeyRole::LayerSwitch => FfiKeyRole::LayerSwitch,
            KeyRole::LanguageSwitch => FfiKeyRole::LanguageSwitch,
        },
        alternates: key.alternates,
    }
}

fn convert_frame_metrics(metrics: taza_engine::keyboard::FrameMetrics) -> FfiFrameMetrics {
    FfiFrameMetrics {
        grid_height: metrics.grid_height,
        candidate_bar_height: metrics.candidate_bar_height,
        total_height: metrics.total_height(),
        letter_font_size: metrics.letter_font_size,
        control_font_size: metrics.control_font_size,
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

/// 키보드 익스텐션 프로세스당 하나 — 셸은 이 객체 하나로 입력·화면·팩을 오간다.
/// 조립은 코어(`Engine`)가 하고 이 계층은 타입 번역과 파일 IO만 맡는다.
#[derive(uniffi::Object)]
pub struct KeyboardSession {
    engine: Mutex<Engine>,
}

#[uniffi::export]
impl KeyboardSession {
    /// 언어 태그로 만든다. 내장 선언이 없는 태그는 팩을 받아야 쓸 수 있으므로
    /// 여기서 걸러진다 — 팩이 실리면 그 선언이 내장 선언을 대신한다.
    #[uniffi::constructor]
    pub fn new(language_tag: String) -> Result<Self, FfiLanguageError> {
        let language =
            LanguageDescriptor::builtin(&language_tag).ok_or(FfiLanguageError::Unsupported)?;
        let engine = Engine::new(language).ok_or(FfiLanguageError::Unsupported)?;
        Ok(KeyboardSession {
            engine: Mutex::new(engine),
        })
    }

    /// 지금 쓰고 있는 언어 선언 — 팩을 실으면 팩이 밝힌 값으로 바뀐다.
    pub fn language(&self) -> FfiLanguageDescriptor {
        let engine = self.engine.lock().unwrap();
        let language = engine.language();
        FfiLanguageDescriptor {
            tag: language.tag.clone(),
            display_name: language.display_name.clone(),
            keycap_label: language.keycap_label.clone(),
            layout_name: language.layout_name.clone(),
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
        self.engine
            .lock()
            .unwrap()
            .load_pack(Arc::new(bytes) as Arc<dyn PackBytes>)
            .map_err(|error| FfiPackError::Invalid {
                message: error.to_string(),
            })
    }

    /// 표시 환경 주입 — 셸이 자기 크기를 알게 될 때(첫 배치, 회전, 분할) 부른다.
    /// 이후 프레임의 치수는 이 값을 따른다.
    pub fn set_metrics(&self, form_factor: FfiFormFactor, width_points: f32) {
        self.engine.lock().unwrap().set_metrics(KeyboardMetrics {
            form_factor: match form_factor {
                FfiFormFactor::PhonePortrait => FormFactor::PhonePortrait,
                FfiFormFactor::PhoneLandscape => FormFactor::PhoneLandscape,
                FfiFormFactor::Tablet => FormFactor::Tablet,
            },
            width_points,
        });
    }

    /// 프레임 전체를 받지 않고 치수만 필요할 때(입력 뷰 높이 제약).
    pub fn frame_metrics(&self) -> FfiFrameMetrics {
        convert_frame_metrics(self.engine.lock().unwrap().frame_metrics())
    }

    /// 이벤트당 1회 왕복 — 반환된 Effect 목록을 셸이 순서대로 플랫폼 API로 번역한다.
    pub fn handle_event(&self, event: FfiInputEvent, context: FfiEditorContext) -> Vec<FfiEffect> {
        let Some(input_event) = convert_event(event) else {
            return Vec::new();
        };
        let effects = self
            .engine
            .lock()
            .unwrap()
            .handle(input_event, &convert_context(&context));
        effects.into_iter().map(convert_effect).collect()
    }

    /// 터치 좌표(정규화) → 코어 히트 테스트 → 합성까지 한 번에.
    pub fn press_at(&self, x: f32, y: f32, context: FfiEditorContext) -> FfiPressResult {
        let result = self
            .engine
            .lock()
            .unwrap()
            .press_at(x, y, &convert_context(&context));
        FfiPressResult {
            effects: result.effects.into_iter().map(convert_effect).collect(),
            layout_changed: result.layout_changed,
            requests_next_language: result.request == Some(ShellRequest::NextLanguage),
        }
    }

    /// 좌표에 있는 키 — 셸이 길게 누르기 대상을 알아내는 통로. 스냅 규칙이 탭과
    /// 같아야 하므로 코어 히트 테스트를 그대로 쓴다.
    pub fn key_at(&self, x: f32, y: f32) -> FfiFrameKey {
        convert_frame_key(self.engine.lock().unwrap().key_at(x, y))
    }

    /// 길게 눌러 연 팝업에서 고른 변형 문자 — 일반 키 입력과 같은 경로로 흐른다.
    pub fn select_alternate(&self, alternate: String, context: FfiEditorContext) -> Vec<FfiEffect> {
        let effects = self
            .engine
            .lock()
            .unwrap()
            .select_alternate(&alternate, &convert_context(&context));
        effects.into_iter().map(convert_effect).collect()
    }

    /// 스페이스바를 길게 눌러 끄는 커서 이동. 셸은 포인터 x(정규화)만 흘려보내고
    /// 몇 칸 움직일지는 코어가 판정한다.
    pub fn begin_cursor_drag(&self, x: f32) {
        self.engine.lock().unwrap().begin_cursor_drag(x);
    }

    pub fn update_cursor_drag(&self, x: f32, context: FfiEditorContext) -> Vec<FfiEffect> {
        let effects = self
            .engine
            .lock()
            .unwrap()
            .update_cursor_drag(x, &convert_context(&context));
        effects.into_iter().map(convert_effect).collect()
    }

    pub fn end_cursor_drag(&self) {
        self.engine.lock().unwrap().end_cursor_drag();
    }

    pub fn keyboard_frame(&self) -> FfiKeyboardFrame {
        let frame = self.engine.lock().unwrap().frame();
        FfiKeyboardFrame {
            rows: frame
                .rows
                .into_iter()
                .map(|row| row.into_iter().map(convert_frame_key).collect())
                .collect(),
            metrics: convert_frame_metrics(frame.metrics),
        }
    }

    /// 개인화 상태 직렬화 — 셸이 컨테이너 저장소(App Group 등)에 보관한다.
    pub fn personalization_snapshot(&self) -> Vec<String> {
        let snapshot = self.engine.lock().unwrap().personalization_snapshot();
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
        self.engine
            .lock()
            .unwrap()
            .restore_personalization(PersonalizationState { entries, clock });
    }
}
