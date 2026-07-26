//! 플랫폼 셸(Swift/Kotlin)이 소비하는 FFI 표면. 코어는 sans-io를 유지하고,
//! 파일 IO(팩 mmap·팩 설치)는 이 계층이 담당한다. 이벤트당 1회 왕복 계약을 그대로
//! 노출한다. 네트워크는 셸의 일이다 — 이 계층은 이미 내려받은 바이트만 다룬다.

mod install;

pub use install::{
    FfiInstallError, FfiInstalledPack, install_pack_archive, read_installed_pack,
    supported_pack_format_version,
};

use std::sync::{Arc, Mutex};

use taza_engine::contract::{
    Candidate, CandidateGroup, CandidateKind, Capitalization, CursorSensitivity, EditorContext,
    Effect, EmojiCategory, FieldKind, FieldTraits, InputEvent, KeySignal, KeyboardHeight,
    ReturnKey, UserPreferences,
};
use taza_engine::engine::{Engine, PackBytes};
use taza_engine::keyboard::{FormFactor, KeyLegend, KeyRole, KeyboardMetrics, ShellRequest};
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
    Search,
    Number,
    Decimal,
    Phone,
    Password,
}

/// 리턴키가 시키는 동작 — iOS `returnKeyType`, Android `imeOptions`를 셸이 옮긴다.
#[derive(uniffi::Enum)]
pub enum FfiReturnKey {
    Return,
    Go,
    Search,
    Send,
    Next,
    Done,
    Join,
    Route,
    Continue,
}

/// 앱이 요구하는 자동 대문자화의 범위 — iOS `autocapitalizationType`.
#[derive(uniffi::Enum)]
pub enum FfiCapitalization {
    None,
    Words,
    Sentences,
    AllCharacters,
}

/// 편집 대상이 스스로 밝힌 성격. 값의 주인이 앱이라 사용자 설정과 AND로 묶인다.
#[derive(uniffi::Record)]
pub struct FfiFieldTraits {
    pub kind: FfiFieldKind,
    pub return_key: FfiReturnKey,
    pub capitalization: FfiCapitalization,
    pub autocorrect: bool,
    pub smart_punctuation: bool,
}

fn convert_field_traits(traits: &FfiFieldTraits) -> FieldTraits {
    FieldTraits {
        kind: convert_field(&traits.kind),
        return_key: match traits.return_key {
            FfiReturnKey::Return => ReturnKey::Return,
            FfiReturnKey::Go => ReturnKey::Go,
            FfiReturnKey::Search => ReturnKey::Search,
            FfiReturnKey::Send => ReturnKey::Send,
            FfiReturnKey::Next => ReturnKey::Next,
            FfiReturnKey::Done => ReturnKey::Done,
            FfiReturnKey::Join => ReturnKey::Join,
            FfiReturnKey::Route => ReturnKey::Route,
            FfiReturnKey::Continue => ReturnKey::Continue,
        },
        capitalization: match traits.capitalization {
            FfiCapitalization::None => Capitalization::None,
            FfiCapitalization::Words => Capitalization::Words,
            FfiCapitalization::Sentences => Capitalization::Sentences,
            FfiCapitalization::AllCharacters => Capitalization::AllCharacters,
        },
        autocorrect: traits.autocorrect,
        smart_punctuation: traits.smart_punctuation,
    }
}

/// 친 말과 그 자리에 들어갈 말. 순정의 "텍스트 대치"와 같다.
#[derive(uniffi::Record)]
pub struct FfiShortcut {
    pub trigger: String,
    pub replacement: String,
}

#[derive(uniffi::Record)]
pub struct FfiEditorContext {
    pub text_before_cursor: Option<String>,
    pub incognito: bool,
    pub field: FfiFieldKind,
}

/// 설정 화면이 소유하는 값 — 순정 키보드의 설정은 서드파티가 읽을 수 없으므로
/// 셸이 자기 저장소에서 읽어 세션에 넣는다.
#[derive(uniffi::Record)]
pub struct FfiUserPreferences {
    pub auto_correction: bool,
    pub predictions: bool,
    pub double_space_period: bool,
    pub personalized_learning: bool,
    pub auto_capitalization: bool,
    pub smart_punctuation: bool,
    pub auto_pairing: bool,
    pub annotation_candidates: bool,
    pub key_alternates: bool,
    pub number_row: bool,
    pub candidate_bar_always: bool,
    pub keyboard_height: FfiKeyboardHeight,
    pub cursor_sensitivity: FfiCursorSensitivity,
}

#[derive(uniffi::Enum)]
pub enum FfiKeyboardHeight {
    Compact,
    Standard,
    Tall,
}

#[derive(uniffi::Enum)]
pub enum FfiCursorSensitivity {
    Low,
    Standard,
    High,
}

/// 스냅샷에 무엇이 얼마나 들어 있는지. 형식의 주인이 여기이므로 세는 일도 여기서 한다 —
/// 셸이 줄을 뜯어보기 시작하면 형식이 두 곳에 살게 된다.
#[derive(uniffi::Record)]
pub struct FfiPersonalizationSummary {
    pub learned_words: u32,
    pub recent_annotations: u32,
}

/// 설정 화면이 "지우기" 앞에서 무엇이 남아 있는지 보여 주는 통로.
#[uniffi::export]
pub fn personalization_summary(lines: Vec<String>) -> FfiPersonalizationSummary {
    let count = |prefix: &str| lines.iter().filter(|line| line.starts_with(prefix)).count() as u32;
    FfiPersonalizationSummary {
        learned_words: count("w\t"),
        recent_annotations: count("a\t"),
    }
}

/// 아직 사용자가 건드리지 않은 설정의 값. 기본값의 주인은 코어이므로 셸은 자기 표를
/// 두지 않고 이 함수를 부른다.
#[uniffi::export]
pub fn default_user_preferences() -> FfiUserPreferences {
    convert_preferences_out(UserPreferences::default())
}

fn convert_preferences_out(preferences: UserPreferences) -> FfiUserPreferences {
    FfiUserPreferences {
        auto_correction: preferences.auto_correction,
        predictions: preferences.predictions,
        double_space_period: preferences.double_space_period,
        personalized_learning: preferences.personalized_learning,
        auto_capitalization: preferences.auto_capitalization,
        smart_punctuation: preferences.smart_punctuation,
        auto_pairing: preferences.auto_pairing,
        annotation_candidates: preferences.annotation_candidates,
        key_alternates: preferences.key_alternates,
        number_row: preferences.number_row,
        candidate_bar_always: preferences.candidate_bar_always,
        keyboard_height: match preferences.keyboard_height {
            KeyboardHeight::Compact => FfiKeyboardHeight::Compact,
            KeyboardHeight::Standard => FfiKeyboardHeight::Standard,
            KeyboardHeight::Tall => FfiKeyboardHeight::Tall,
        },
        cursor_sensitivity: match preferences.cursor_sensitivity {
            CursorSensitivity::Low => FfiCursorSensitivity::Low,
            CursorSensitivity::Standard => FfiCursorSensitivity::Standard,
            CursorSensitivity::High => FfiCursorSensitivity::High,
        },
    }
}

fn convert_preferences_in(preferences: FfiUserPreferences) -> UserPreferences {
    UserPreferences {
        auto_correction: preferences.auto_correction,
        predictions: preferences.predictions,
        double_space_period: preferences.double_space_period,
        personalized_learning: preferences.personalized_learning,
        auto_capitalization: preferences.auto_capitalization,
        smart_punctuation: preferences.smart_punctuation,
        auto_pairing: preferences.auto_pairing,
        annotation_candidates: preferences.annotation_candidates,
        key_alternates: preferences.key_alternates,
        number_row: preferences.number_row,
        candidate_bar_always: preferences.candidate_bar_always,
        keyboard_height: match preferences.keyboard_height {
            FfiKeyboardHeight::Compact => KeyboardHeight::Compact,
            FfiKeyboardHeight::Standard => KeyboardHeight::Standard,
            FfiKeyboardHeight::Tall => KeyboardHeight::Tall,
        },
        cursor_sensitivity: match preferences.cursor_sensitivity {
            FfiCursorSensitivity::Low => CursorSensitivity::Low,
            FfiCursorSensitivity::Standard => CursorSensitivity::Standard,
            FfiCursorSensitivity::High => CursorSensitivity::High,
        },
    }
}

#[derive(uniffi::Enum)]
pub enum FfiInputEvent {
    /// 한 번에 여러 글자를 넣는 키 (`.com` 등)
    Text {
        text: String,
    },
    Key {
        character: String,
    },
    Backspace,
    Separator {
        character: String,
    },
    CandidateSelected {
        index: u32,
    },
    CursorMoved,
    FocusLost,
}

#[derive(uniffi::Enum)]
pub enum FfiCandidateKind {
    Typed,
    Prediction,
    Conversion,
    Correction,
}

/// 후보 바에서 이 후보가 서는 자리 — 셸은 갈래별로 묶어 인라인으로 늘어놓는다.
#[derive(uniffi::Enum)]
pub enum FfiCandidateGroup {
    Word,
    Emoji,
    Symbol,
    Emoticon,
}

#[derive(uniffi::Record)]
pub struct FfiCandidate {
    pub text: String,
    pub kind: FfiCandidateKind,
    pub group: FfiCandidateGroup,
}

#[derive(uniffi::Enum)]
pub enum FfiEffect {
    CommitText {
        text: String,
    },
    SetComposing {
        text: String,
        caret: u32,
    },
    ClearComposing,
    DeleteBackward {
        code_points: u32,
    },
    UpdateCandidates {
        candidates: Vec<FfiCandidate>,
    },
    MoveCursor {
        offset: i32,
    },
    /// 밀리초 뒤에 `timer_fired`를 부르라는 요청. 앞선 타이머는 갈아 끼운다 —
    /// 끄는 명령은 없다(이미 끝난 주기에 울린 타이머는 아무 일도 하지 않는다).
    SetTimer {
        milliseconds: u32,
    },
}

/// 통합 검색면에 담기는 항목 하나.
#[derive(uniffi::Record)]
pub struct FfiAnnotationPanelItem {
    pub group: FfiCandidateGroup,
    pub text: String,
}

/// 검색면의 한 그룹. 헤더 문구는 싣지 않는다 — 갈래(`group`)와 묶음(`category`)이 곧
/// 신원이고, 그것을 어느 나라 말로 적을지는 화면의 일이다. 둘 다 비면 최근에 고른 것들이다.
#[derive(uniffi::Record)]
pub struct FfiAnnotationPanelGroup {
    pub group: Option<FfiCandidateGroup>,
    /// 이모지 묶음이면 그 자리 — 셸이 묶음마다 다른 표식을 세운다
    pub category: Option<FfiEmojiCategory>,
    pub items: Vec<FfiAnnotationPanelItem>,
}

/// 이모지가 검색면에서 서는 묶음 — 빌트인 키보드와 같은 갈래다
#[derive(uniffi::Enum)]
pub enum FfiEmojiCategory {
    SmileysAndPeople,
    AnimalsAndNature,
    FoodAndDrink,
    Activities,
    TravelAndPlaces,
    Objects,
    Symbols,
    Flags,
}

fn convert_emoji_category(category: EmojiCategory) -> FfiEmojiCategory {
    match category {
        EmojiCategory::SmileysAndPeople => FfiEmojiCategory::SmileysAndPeople,
        EmojiCategory::AnimalsAndNature => FfiEmojiCategory::AnimalsAndNature,
        EmojiCategory::FoodAndDrink => FfiEmojiCategory::FoodAndDrink,
        EmojiCategory::Activities => FfiEmojiCategory::Activities,
        EmojiCategory::TravelAndPlaces => FfiEmojiCategory::TravelAndPlaces,
        EmojiCategory::Objects => FfiEmojiCategory::Objects,
        EmojiCategory::Symbols => FfiEmojiCategory::Symbols,
        EmojiCategory::Flags => FfiEmojiCategory::Flags,
    }
}

#[derive(uniffi::Record)]
pub struct FfiAnnotationPanel {
    pub groups: Vec<FfiAnnotationPanelGroup>,
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
    /// 눌리지 않는 빈 자리 — 셸은 키를 그리지 않는다
    Blank,
}

/// 낱말로 적히는 키의 갈래 — 셸이 화면 언어로 옮긴다.
#[derive(uniffi::Enum)]
pub enum FfiKeyLegend {
    Return,
    Go,
    Search,
    Send,
    Next,
    Done,
    Join,
    Route,
    Continue,
}

#[derive(uniffi::Record)]
pub struct FfiFrameKey {
    pub row: u32,
    pub index: u32,
    pub label: String,
    /// 낱말로 적히는 키 — 화면 언어를 타므로 셸이 자기 말로 옮긴다. 없으면 `label`이
    /// 그대로 나가는 글자·기호 키다.
    pub legend: Option<FfiKeyLegend>,
    pub bounds: FfiKeyBounds,
    pub shift_active: bool,
    /// 이 필드에서 강조색으로 그릴 키 (검색 필드의 리턴키 등)
    pub emphasized: bool,
    pub role: FfiKeyRole,
    /// 이 키 라벨의 글꼴 크기(pt) — 글자 키·기호 제어 키·낱말 제어 키가 서로 다르다
    pub font_size: f32,
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
    /// 글자 키 글꼴 — 키 밖에서 같은 크기를 써야 하는 자리(변형 문자 팝업)가 쓴다
    pub letter_font_size: f32,
}

#[derive(uniffi::Record)]
pub struct FfiKeyboardFrame {
    pub rows: Vec<Vec<FfiFrameKey>>,
    pub metrics: FfiFrameMetrics,
    /// 키 위에 놓이는 통합 검색면이 차지하는 높이(키보드 높이 기준 정규화값). 0이면
    /// 패널이 없는 레이어다 — 내용은 `annotation_panel`로 따로 받는다.
    pub panel_height_ratio: f32,
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
        // 좌표 없이 오는 키는 물리 키보드·접근성 경로 — 어느 키인지 확실하다
        FfiInputEvent::Key { character } => {
            InputEvent::Key(KeySignal::certain(character.chars().next()?))
        }
        FfiInputEvent::Text { text } => InputEvent::Text(text),
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
            CandidateKind::Typed => FfiCandidateKind::Typed,
            CandidateKind::Prediction => FfiCandidateKind::Prediction,
            CandidateKind::Conversion => FfiCandidateKind::Conversion,
            CandidateKind::Correction => FfiCandidateKind::Correction,
        },
        group: convert_candidate_group(candidate.group),
    }
}

fn convert_candidate_group(group: CandidateGroup) -> FfiCandidateGroup {
    match group {
        CandidateGroup::Word => FfiCandidateGroup::Word,
        CandidateGroup::Emoji => FfiCandidateGroup::Emoji,
        CandidateGroup::Symbol => FfiCandidateGroup::Symbol,
        CandidateGroup::Emoticon => FfiCandidateGroup::Emoticon,
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
        Effect::SetTimer(milliseconds) => FfiEffect::SetTimer { milliseconds },
    }
}

fn convert_frame_key(key: taza_engine::keyboard::FrameKey) -> FfiFrameKey {
    FfiFrameKey {
        row: key.position.row as u32,
        index: key.position.index as u32,
        label: key.label,
        legend: key.legend.map(|legend| match legend {
            KeyLegend::Return => FfiKeyLegend::Return,
            KeyLegend::Go => FfiKeyLegend::Go,
            KeyLegend::Search => FfiKeyLegend::Search,
            KeyLegend::Send => FfiKeyLegend::Send,
            KeyLegend::Next => FfiKeyLegend::Next,
            KeyLegend::Done => FfiKeyLegend::Done,
            KeyLegend::Join => FfiKeyLegend::Join,
            KeyLegend::Route => FfiKeyLegend::Route,
            KeyLegend::Continue => FfiKeyLegend::Continue,
        }),
        bounds: FfiKeyBounds {
            x: key.bounds.x,
            y: key.bounds.y,
            width: key.bounds.width,
            height: key.bounds.height,
        },
        shift_active: key.shift_active,
        emphasized: key.emphasized,
        role: match key.role {
            KeyRole::Character => FfiKeyRole::Character,
            KeyRole::Shift => FfiKeyRole::Shift,
            KeyRole::Backspace => FfiKeyRole::Backspace,
            KeyRole::Space => FfiKeyRole::Space,
            KeyRole::Enter => FfiKeyRole::Enter,
            KeyRole::LayerSwitch => FfiKeyRole::LayerSwitch,
            KeyRole::LanguageSwitch => FfiKeyRole::LanguageSwitch,
            KeyRole::Blank => FfiKeyRole::Blank,
        },
        font_size: key.font_size,
        alternates: key.alternates,
    }
}

fn convert_frame_metrics(metrics: taza_engine::keyboard::FrameMetrics) -> FfiFrameMetrics {
    FfiFrameMetrics {
        grid_height: metrics.grid_height,
        candidate_bar_height: metrics.candidate_bar_height,
        total_height: metrics.total_height(),
        letter_font_size: metrics.letter_font_size,
    }
}

fn convert_field(field: &FfiFieldKind) -> FieldKind {
    match field {
        FfiFieldKind::Text => FieldKind::Text,
        FfiFieldKind::Email => FieldKind::Email,
        FfiFieldKind::Url => FieldKind::Url,
        FfiFieldKind::Search => FieldKind::Search,
        FfiFieldKind::Number => FieldKind::Number,
        FfiFieldKind::Decimal => FieldKind::Decimal,
        FfiFieldKind::Phone => FieldKind::Phone,
        FfiFieldKind::Password => FieldKind::Password,
    }
}

fn convert_context(context: &FfiEditorContext) -> EditorContext {
    EditorContext {
        text_before_cursor: context.text_before_cursor.clone(),
        incognito: context.incognito,
        field: convert_field(&context.field),
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

    /// 사용자 설정 주입 — 셸은 키보드를 띄울 때마다 자기 저장소에서 읽어 넣는다.
    /// 설정 화면에서 바뀐 값은 다음 표시 때 이 호출로 반영된다.
    pub fn set_preferences(&self, preferences: FfiUserPreferences) {
        self.engine
            .lock()
            .unwrap()
            .set_preferences(convert_preferences_in(preferences));
    }

    /// 이 언어로 칠 수 있는 배열의 이름들 — 첫 항목이 기본 배열이다.
    pub fn available_layouts(&self) -> Vec<String> {
        self.engine.lock().unwrap().available_layouts()
    }

    /// 지금 치고 있는 배열의 이름.
    pub fn selected_layout(&self) -> String {
        self.engine.lock().unwrap().layout_name().to_string()
    }

    /// 배열을 바꾼다. 그런 이름이 없으면 false — 설정에 남은 이름이 팩 갱신으로
    /// 사라졌을 때 셸이 기본 배열로 되돌리는 신호다.
    pub fn select_layout(&self, name: String) -> bool {
        self.engine.lock().unwrap().select_layout(&name)
    }

    /// 문맥이 문장 첫 자리를 가리키면 shift를 미리 올린다. 초점·필드가 바뀌었을 때와
    /// `handle_event`로 입력을 넣은 뒤에 부른다(`press_at`은 스스로 한다).
    /// 프레임을 다시 그려야 하면 true.
    pub fn sync_auto_shift(&self, context: FfiEditorContext) -> bool {
        self.engine
            .lock()
            .unwrap()
            .sync_auto_shift(&convert_context(&context))
    }

    /// 사용자 대치 표 주입(순정의 "텍스트 대치"). 값의 주인은 설정 앱이다.
    pub fn set_shortcuts(&self, shortcuts: Vec<FfiShortcut>) {
        self.engine.lock().unwrap().set_shortcuts(
            shortcuts
                .into_iter()
                .filter(|entry| !entry.trigger.is_empty())
                .map(|entry| (entry.trigger, entry.replacement))
                .collect(),
        );
    }

    /// 표시 환경 주입 — 셸이 자기 크기를 알게 될 때(첫 배치, 회전, 분할) 부른다.
    /// 이후 프레임의 치수는 이 값을 따른다.
    pub fn set_metrics(&self, form_factor: FfiFormFactor, width_points: f32, text_scale: f32) {
        self.engine.lock().unwrap().set_metrics(KeyboardMetrics {
            form_factor: match form_factor {
                FfiFormFactor::PhonePortrait => FormFactor::PhonePortrait,
                FfiFormFactor::PhoneLandscape => FormFactor::PhoneLandscape,
                FfiFormFactor::Tablet => FormFactor::Tablet,
            },
            width_points,
            text_scale,
        });
    }

    /// 편집 대상이 바뀔 때(초점 이동, 앱 전환) 셸이 알려 주는 필드 성격. 배열·리턴키·
    /// 후보 바 자리가 여기서 갈리므로 프레임을 받기 전에 넣는다.
    pub fn set_field(&self, traits: FfiFieldTraits) {
        self.engine
            .lock()
            .unwrap()
            .set_field(convert_field_traits(&traits));
    }

    /// 이메일·URL·비밀번호처럼 라틴 글자를 넣게 되어 있는 칸인가. 어느 언어로 열지는
    /// 언어 목록을 가진 셸이 정하므로 코어는 "라틴이 맞다"만 말한다.
    pub fn field_prefers_latin(&self) -> bool {
        self.engine.lock().unwrap().field_prefers_latin()
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

    /// 멀티탭 시한이 다 됐다고 셸이 알려 준다 — 다음에 같은 키를 눌러도 주기가 아니라
    /// 새 글자로 시작한다. 이미 끝난 주기에 울린 타이머는 아무 일도 하지 않는다.
    pub fn timer_fired(&self) {
        self.engine.lock().unwrap().timer_fired();
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

    /// shift 키를 두 번 누른 것 — 두 번 누름을 알아보는 것은 플랫폼 제스처라 셸이 하고,
    /// 고정할 수 있는 배열인지와 그 뒤 상태는 코어가 정한다. 고정되면 true.
    pub fn toggle_shift_lock(&self) -> bool {
        self.engine.lock().unwrap().toggle_shift_lock()
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
            panel_height_ratio: frame.panel_height_ratio,
        }
    }

    /// 통합 검색면 내용 — 검색어가 비면 자주 쓰는 것과 갈래별 목록이 온다.
    pub fn annotation_panel(&self, query: String) -> FfiAnnotationPanel {
        let panel = self.engine.lock().unwrap().annotation_panel(&query);
        FfiAnnotationPanel {
            groups: panel
                .groups
                .into_iter()
                .map(|group| FfiAnnotationPanelGroup {
                    group: group.group.map(convert_candidate_group),
                    category: group.category.map(convert_emoji_category),
                    items: group
                        .items
                        .into_iter()
                        .map(|item| FfiAnnotationPanelItem {
                            group: convert_candidate_group(item.group),
                            text: item.text,
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    /// 검색면에서 고른 것을 넣는다 — 진행 중 조합 확정과 최근 사용 기록은 코어가 한다.
    pub fn select_annotation(
        &self,
        group: FfiCandidateGroup,
        text: String,
        context: FfiEditorContext,
    ) -> Vec<FfiEffect> {
        let group = match group {
            FfiCandidateGroup::Word => CandidateGroup::Word,
            FfiCandidateGroup::Emoji => CandidateGroup::Emoji,
            FfiCandidateGroup::Symbol => CandidateGroup::Symbol,
            FfiCandidateGroup::Emoticon => CandidateGroup::Emoticon,
        };
        let effects =
            self.engine
                .lock()
                .unwrap()
                .select_annotation(group, &text, &convert_context(&context));
        effects.into_iter().map(convert_effect).collect()
    }

    /// 개인화 상태 직렬화 — 셸이 컨테이너 저장소(App Group 등)에 보관한다.
    pub fn personalization_snapshot(&self) -> Vec<String> {
        let snapshot = self.engine.lock().unwrap().personalization_snapshot();
        let mut lines = vec![snapshot.clock.to_string()];
        for (word, count, last_used) in snapshot.entries {
            lines.push(format!("w\t{word}\t{count}\t{last_used}"));
        }
        for (group, text) in snapshot.recent_annotations {
            let Some(tag) = group.tag() else { continue };
            lines.push(format!("a\t{tag}\t{text}"));
        }
        lines
    }

    /// 통합 검색면의 "자주 쓰는" 목록만 비운다 — 배운 어휘는 그대로 둔다.
    pub fn reset_recent_annotations(&self) {
        self.engine.lock().unwrap().reset_recent_annotations();
    }

    /// 배운 것을 전부 잊는다 — 순정의 "키보드 사전 재설정". 셸은 이 호출과 함께
    /// 보관 중인 스냅샷도 지운다.
    pub fn reset_personalization(&self) {
        self.engine.lock().unwrap().reset_personalization();
    }

    pub fn restore_personalization(&self, lines: Vec<String>) {
        let Some((clock_line, entry_lines)) = lines.split_first() else {
            return;
        };
        let Ok(clock) = clock_line.parse() else {
            return;
        };
        let mut entries = Vec::new();
        let mut recent_annotations = Vec::new();
        for line in entry_lines {
            match line.split('\t').collect::<Vec<&str>>().as_slice() {
                ["w", word, count, last_used] => {
                    let (Ok(count), Ok(last_used)) = (count.parse(), last_used.parse()) else {
                        continue;
                    };
                    entries.push((word.to_string(), count, last_used));
                }
                ["a", tag, text] => {
                    let Some(group) = tag.parse().ok().and_then(CandidateGroup::from_tag) else {
                        continue;
                    };
                    recent_annotations.push((group, text.to_string()));
                }
                _ => continue,
            }
        }
        self.engine
            .lock()
            .unwrap()
            .restore_personalization(PersonalizationState {
                entries,
                clock,
                recent_annotations,
            });
    }
}
