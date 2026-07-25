mod field;
pub mod hit;
pub mod layouts;

pub use hit::{KeyProbability, KeySignal};

use crate::contract::{FieldKind, InputEvent};
use crate::lang::LanguageDescriptor;

/// 커서를 한 칸 옮기는 데 필요한 가로 이동 거리(pt). 순정 스페이스바 길게 눌러
/// 커서 이동과 비슷한 감도다. 정규화 좌표가 아니라 물리 거리로 잡아야 화면이
/// 넓어져도 손가락이 같은 만큼 움직인다.
const CURSOR_DRAG_STEP_POINTS: f32 = 10.0;

// 레이아웃 데이터 타입은 언어팩 와이어 타입이 원본이다 — 배열 추가는 팩 배포로 끝난다.
pub use crate::pack::layout::{KeyAction, KeyboardLayout, KeyboardLayoutSet, LayoutKey, LayoutRow};

/// 셸이 알려 주는 표시 폼팩터. 순정 키보드는 폼팩터마다 다른 키 높이·글자 크기를
/// 쓰므로, 기하를 코어가 정하려면 이 갈래가 필요하다. 플랫폼 값(size class,
/// window class)을 이 갈래로 옮기는 번역만 셸이 하고, 치수 자체는 코어가 정한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormFactor {
    PhonePortrait,
    /// 높이가 귀한 화면 — 순정도 행 높이만 줄이고 배열은 그대로 둔다.
    PhoneLandscape,
    Tablet,
}

impl FormFactor {
    fn key_row_height_points(self) -> f32 {
        match self {
            FormFactor::PhonePortrait => 54.0,
            FormFactor::PhoneLandscape => 40.0,
            FormFactor::Tablet => 62.0,
        }
    }

    fn candidate_bar_height_points(self) -> f32 {
        match self {
            FormFactor::PhonePortrait => 44.0,
            FormFactor::PhoneLandscape => 38.0,
            FormFactor::Tablet => 52.0,
        }
    }

    /// 순정 문자 키 글자는 키 높이에 견줘 큼직하다 — 22pt로는 작아 보인다.
    fn letter_font_size_points(self) -> f32 {
        match self {
            FormFactor::PhonePortrait => 25.0,
            FormFactor::PhoneLandscape => 22.0,
            FormFactor::Tablet => 28.0,
        }
    }

    fn control_font_size_points(self) -> f32 {
        match self {
            FormFactor::PhonePortrait => 16.0,
            FormFactor::PhoneLandscape => 15.0,
            FormFactor::Tablet => 18.0,
        }
    }
}

/// 셸이 주입하는 표시 환경. 코어는 이 값으로만 폼팩터를 알고, 셸은 화면이 바뀔 때
/// (회전·분할·기기 차이) 다시 주입한다.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyboardMetrics {
    pub form_factor: FormFactor,
    /// 키보드가 차지하는 가로 폭. 물리 거리로 판정해야 하는 동작(커서 이동 감도)이
    /// 화면 크기에 휘둘리지 않게 한다.
    pub width_points: f32,
}

impl Default for KeyboardMetrics {
    /// 셸이 아직 자기 크기를 모르는 첫 프레임용 기본값 — 표준 폰 세로.
    fn default() -> Self {
        KeyboardMetrics {
            form_factor: FormFactor::PhonePortrait,
            width_points: 390.0,
        }
    }
}

/// 프레임과 함께 내려가는 실측 치수(pt). 셸은 이 값을 제약·글꼴에 그대로 쓰고
/// 폼팩터를 다시 판단하지 않는다.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameMetrics {
    /// 키 그리드 높이 — 각 키의 정규화 높이에 이 값을 곱하면 실제 높이다.
    pub grid_height: f32,
    pub candidate_bar_height: f32,
    pub letter_font_size: f32,
    pub control_font_size: f32,
}

impl FrameMetrics {
    /// 후보 바까지 포함한 키보드 전체 높이 — 셸이 입력 뷰 높이로 쓴다.
    pub fn total_height(&self) -> f32 {
        self.grid_height + self.candidate_bar_height
    }
}

/// 레이어 전환 키 라벨을 고르기 위한 스크립트 판별 — 렌더링 관심사이므로 언어별
/// 합성기(feature로 빠질 수 있다)에 의존하지 않고 코드포인트 범위로만 판단한다.
fn is_hangul_script(character: char) -> bool {
    matches!(character, '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}' | '\u{AC00}'..='\u{D7A3}')
}

/// 레이어가 차지하는 높이 — 표준 행 몇 개분인가. 패널(통합 검색면)도 자기 높이를 갖는다.
fn layer_rows(layout: &KeyboardLayout) -> f32 {
    layout.panel_rows.max(0.0)
        + layout
            .rows
            .iter()
            .map(|row| row.height_ratio.max(0.0))
            .sum::<f32>()
}

/// 각 행의 정규화 높이 — 행별 상대 높이를 레이어 전체 높이로 나눈다. 값이 비어 있는
/// 레이아웃(높이를 지정하지 않은 팩)은 균등 배분으로 되돌린다.
pub(crate) fn row_heights(layout: &KeyboardLayout) -> Vec<f32> {
    let total = layer_rows(layout);
    if total <= 0.0 {
        return vec![1.0 / layout.rows.len() as f32; layout.rows.len()];
    }
    layout
        .rows
        .iter()
        .map(|row| row.height_ratio.max(0.0) / total)
        .collect()
}

/// 키 위에 놓이는 패널이 차지하는 몫(정규화). 0이면 키만 있는 레이어다.
pub(crate) fn panel_height_ratio(layout: &KeyboardLayout) -> f32 {
    let total = layer_rows(layout);
    if total <= 0.0 {
        return 0.0;
    }
    layout.panel_rows.max(0.0) / total
}

/// 한 행의 키 기하 — 언어·상태와 무관한 순수 배치 계산이라 레이아웃만 있으면 된다.
/// (오타 합성 같은 오프라인 도구가 세션 없이 쓰는 통로)
pub fn row_bounds(layout: &KeyboardLayout, row_index: usize) -> Vec<KeyBounds> {
    let heights = row_heights(layout);
    let row = &layout.rows[row_index];
    let height = heights[row_index];
    // 패널이 있는 레이어에서는 키 행이 패널 아래에서 시작한다
    let y: f32 = panel_height_ratio(layout) + heights[..row_index].iter().sum::<f32>();
    let total_ratio: f32 = row.keys.iter().map(|key| key.width_ratio).sum();
    // 행 폭이 1 미만이면 좌우 여백을 균등 분배해 가운데 정렬
    let mut x = (1.0 - total_ratio.min(1.0)) / 2.0;
    let mut bounds = Vec::with_capacity(row.keys.len());
    for key in &row.keys {
        bounds.push(KeyBounds {
            x,
            y,
            width: key.width_ratio,
            height,
        });
        x += key.width_ratio;
    }
    bounds
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShiftState {
    Released,
    /// 한 글자 입력 후 자동 해제되는 일회성 shift (모바일 관습)
    Pressed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyPosition {
    pub row: usize,
    pub index: usize,
}

/// 좌표는 키보드 영역 기준 정규화([0,1]×[0,1]) — px 변환은 셸의 몫이다.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl KeyBounds {
    pub(crate) fn center_x(&self) -> f32 {
        self.x + self.width / 2.0
    }
}

/// 셸이 그대로 그리는 화면 명세. 셸은 px 변환과 렌더링만 하고 판단하지 않는다.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyboardFrame {
    pub rows: Vec<Vec<FrameKey>>,
    pub metrics: FrameMetrics,
    /// 키 위에 놓이는 패널(통합 검색면)이 차지하는 높이 — 키보드 높이 기준 정규화값.
    /// 0이면 패널이 없는 레이어다. 패널 안의 내용은 `Engine::annotation_panel`이 낸다.
    pub panel_height_ratio: f32,
}

/// 셸이 길게 누르기 같은 플랫폼 관습을 붙일 때 쓰는 키의 갈래. 셸은 이 값으로만
/// 분기하고(화이트리스트), 입력 의미론은 여전히 코어가 판정한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyRole {
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

#[derive(Debug, Clone, PartialEq)]
pub struct FrameKey {
    pub position: KeyPosition,
    pub label: String,
    pub bounds: KeyBounds,
    /// VoiceOver/TalkBack에 노출할 라벨 — 접근성은 비통일 영역이 아니라 계약의 일부
    pub accessibility_label: String,
    pub shift_active: bool,
    /// 이 필드에서 눈에 띄어야 하는 키 — 검색 필드의 리턴키처럼 순정이 강조색을 쓰는
    /// 자리다. 어떤 색인지는 셸의 디자인 시스템이 정한다.
    pub emphasized: bool,
    pub role: KeyRole,
    /// 길게 눌러 고르는 변형 문자 — 표시 순서 그대로. 접근성 경로에서는 커스텀
    /// 액션 목록이 된다.
    pub alternates: Vec<String>,
}

/// 코어가 판정할 수 없는, 셸이 소유한 상태에 대한 요청. 언어 목록·순서는 셸(설정)
/// 소관이므로 코어는 "다음 언어로" 같은 요청만 낸다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellRequest {
    NextLanguage,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PressOutcome {
    pub event: Option<InputEvent>,
    /// shift 등 상태 변화로 프레임을 다시 그려야 하는지
    pub layout_changed: bool,
    pub request: Option<ShellRequest>,
}

/// 레이아웃 묶음 + 레이어·shift 상태에서 프레임을 만들고, 터치 좌표를 InputEvent로
/// 판정한다. 히트 테스트는 v1 기하 판정(포함 → 같은 행 최근접) — 확률(공간) 모델은
/// 이 지점을 교체하는 방식으로 들어온다.
pub struct Keyboard {
    layout_set: KeyboardLayoutSet,
    language: LanguageDescriptor,
    metrics: KeyboardMetrics,
    active_layer: usize,
    shift: ShiftState,
    cursor_drag: Option<CursorDrag>,
    field: FieldKind,
    /// 레이어와 필드를 모두 적용한 배열. 프레임과 히트 테스트가 같은 것을 봐야 눌린
    /// 자리와 그려진 자리가 어긋나지 않으므로 한 번 만들어 두고 함께 쓴다.
    active: KeyboardLayout,
}

/// 스페이스바를 길게 눌러 끄는 커서 이동의 진행 상태. 판정(몇 칸 옮길지)은 코어가,
/// 포인터 스트림 전달만 셸이 한다.
struct CursorDrag {
    origin_x: f32,
    emitted_steps: i32,
}

impl Keyboard {
    pub fn new(layout_set: KeyboardLayoutSet, language: LanguageDescriptor) -> Self {
        assert!(!layout_set.layers.is_empty(), "레이어가 최소 1개 필요");
        let active = field::apply(&layout_set.layers[0], FieldKind::default());
        Keyboard {
            layout_set,
            language,
            metrics: KeyboardMetrics::default(),
            active_layer: 0,
            shift: ShiftState::Released,
            cursor_drag: None,
            field: FieldKind::default(),
            active,
        }
    }

    /// 셸이 자기 크기·폼팩터를 알게 될 때마다(첫 배치, 회전, 분할) 주입한다.
    pub fn set_metrics(&mut self, metrics: KeyboardMetrics) {
        self.metrics = metrics;
    }

    /// 셸이 편집 대상이 바뀔 때마다 알려 주는 필드 성격. 배열·리턴키·후보 바 자리가
    /// 여기서 갈리므로(`docs/inputmode.md`) 셸은 값만 넘기고 판단하지 않는다.
    pub fn set_field(&mut self, field: FieldKind) {
        if self.field == field {
            return;
        }
        self.field = field;
        // 숫자 패드에서 돌아올 때 문자면으로 되돌린다 — 필드가 바뀌면 직전 필드에서
        // 고른 레이어는 의미가 없다
        self.active_layer = 0;
        self.shift = ShiftState::Released;
        self.rebuild();
    }

    pub fn field(&self) -> FieldKind {
        self.field
    }

    fn rebuild(&mut self) {
        self.active = field::apply(&self.layout_set.layers[self.active_layer], self.field);
    }

    /// 지금 레이아웃·폼팩터에 맞는 실측 치수. 프레임을 다시 받지 않고 높이만
    /// 필요할 때(입력 뷰 높이 제약) 쓴다.
    pub fn frame_metrics(&self) -> FrameMetrics {
        let form_factor = self.metrics.form_factor;
        // 행 높이는 표준 행 대비 배수이므로, 그 합이 곧 몇 행치 높이인지가 된다
        let rows = layer_rows(self.layout());
        FrameMetrics {
            grid_height: form_factor.key_row_height_points() * rows,
            // 후보를 내지 않는 필드에서는 바 자리를 없애 키보드가 낮아진다(순정 실측)
            candidate_bar_height: if field::shows_candidate_bar(self.field) {
                form_factor.candidate_bar_height_points()
            } else {
                0.0
            },
            letter_font_size: form_factor.letter_font_size_points(),
            control_font_size: form_factor.control_font_size_points(),
        }
    }

    fn layout(&self) -> &KeyboardLayout {
        &self.active
    }

    fn shifted(&self) -> bool {
        self.shift == ShiftState::Pressed
    }

    /// 레이어 전환 키 라벨 — 순정 관례: 문자면 복귀는 스크립트에 맞게(ABC/한글),
    /// 심볼 1·2면은 123 / #+=. 비밀번호 필드에서는 순정처럼 `.?123`이 된다.
    fn layer_switch_label(&self, target: u8) -> String {
        match target {
            1 if self.field == FieldKind::Password => ".?123".to_string(),
            0 => {
                let uses_hangul = self.layout_set.layers[0]
                    .rows
                    .iter()
                    .flat_map(|row| &row.keys)
                    .any(|key| {
                        matches!(key.action, KeyAction::Character { base, .. }
                        if is_hangul_script(base))
                    });
                if uses_hangul { "한글" } else { "ABC" }.to_string()
            }
            1 => "123".to_string(),
            2 => "#+=".to_string(),
            // 통합 검색면 — 순정 이모지 키와 같은 웃는 얼굴
            _ => "☺".to_string(),
        }
    }

    fn key_character(&self, action: &KeyAction) -> Option<char> {
        match *action {
            KeyAction::Character { base, shifted } => {
                Some(if self.shifted() { shifted } else { base })
            }
            _ => None,
        }
    }

    /// 리턴키 문구 — 순정은 필드가 시키는 동작을 적는다(검색 필드는 "검색").
    fn enter_label(&self) -> String {
        match self.field {
            FieldKind::Search => "검색".to_string(),
            _ => "⏎".to_string(),
        }
    }

    fn key_label(&self, action: &KeyAction) -> String {
        match action {
            KeyAction::Character { .. } => self.key_character(action).unwrap().to_string(),
            KeyAction::Text(text) => text.clone(),
            KeyAction::Shift => "⇧".to_string(),
            KeyAction::Backspace => "⌫".to_string(),
            KeyAction::Space => self.language.display_name.clone(),
            KeyAction::Enter => self.enter_label(),
            KeyAction::LayerSwitch { target } => self.layer_switch_label(*target),
            KeyAction::LanguageSwitch => self.language.keycap_label.clone(),
            KeyAction::Blank => String::new(),
        }
    }

    fn accessibility_label(&self, action: &KeyAction) -> String {
        match action {
            KeyAction::Character { .. } => self.key_character(action).unwrap().to_string(),
            KeyAction::Text(text) => text.clone(),
            KeyAction::Shift => "shift".to_string(),
            KeyAction::Backspace => "backspace".to_string(),
            KeyAction::Space => "space".to_string(),
            KeyAction::Enter => match self.field {
                FieldKind::Search => "search".to_string(),
                _ => "enter".to_string(),
            },
            KeyAction::LayerSwitch { target } => match target {
                0 => "letters".to_string(),
                1 => "numbers".to_string(),
                2 => "symbols".to_string(),
                _ => "emoji".to_string(),
            },
            KeyAction::LanguageSwitch => {
                format!("language, {}", self.language.display_name)
            }
            KeyAction::Blank => String::new(),
        }
    }

    fn key_role(&self, action: &KeyAction) -> KeyRole {
        match action {
            KeyAction::Character { .. } | KeyAction::Text(_) => KeyRole::Character,
            KeyAction::Shift => KeyRole::Shift,
            KeyAction::Backspace => KeyRole::Backspace,
            KeyAction::Space => KeyRole::Space,
            KeyAction::Enter => KeyRole::Enter,
            KeyAction::LayerSwitch { .. } => KeyRole::LayerSwitch,
            KeyAction::LanguageSwitch => KeyRole::LanguageSwitch,
            KeyAction::Blank => KeyRole::Blank,
        }
    }

    fn row_bounds(&self, row_index: usize) -> Vec<KeyBounds> {
        row_bounds(self.layout(), row_index)
    }

    pub fn frame(&self) -> KeyboardFrame {
        let rows = (0..self.layout().rows.len())
            .map(|row_index| {
                let bounds = self.row_bounds(row_index);
                self.layout().rows[row_index]
                    .keys
                    .iter()
                    .enumerate()
                    .map(|(key_index, key)| FrameKey {
                        position: KeyPosition {
                            row: row_index,
                            index: key_index,
                        },
                        label: self.key_label(&key.action),
                        bounds: bounds[key_index],
                        accessibility_label: self.accessibility_label(&key.action),
                        shift_active: key.action == KeyAction::Shift && self.shifted(),
                        emphasized: key.action == KeyAction::Enter
                            && self.field == FieldKind::Search,
                        role: self.key_role(&key.action),
                        alternates: key
                            .alternates
                            .iter()
                            .map(|alternate| alternate.to_string())
                            .collect(),
                    })
                    .collect()
            })
            .collect();
        KeyboardFrame {
            rows,
            metrics: self.frame_metrics(),
            panel_height_ratio: panel_height_ratio(self.layout()),
        }
    }

    fn key_position_at(&self, x: f32, y: f32) -> KeyPosition {
        hit::key_position_at(self.layout(), x, y)
    }

    /// 좌표에 해당하는 키의 프레임 정보. 셸이 길게 누르기 대상을 알아내는 통로다 —
    /// 판정 기준(스냅 규칙)이 탭과 같아야 하므로 코어가 제공한다.
    pub fn key_at(&self, x: f32, y: f32) -> FrameKey {
        let position = self.key_position_at(x, y);
        self.frame().rows[position.row][position.index].clone()
    }

    pub fn press_at(&mut self, x: f32, y: f32) -> PressOutcome {
        let position = self.key_position_at(x, y);
        let action = self.layout().rows[position.row].keys[position.index]
            .action
            .clone();
        match action {
            KeyAction::Shift => {
                self.shift = if self.shifted() {
                    ShiftState::Released
                } else {
                    ShiftState::Pressed
                };
                PressOutcome {
                    event: None,
                    layout_changed: true,
                    request: None,
                }
            }
            KeyAction::Character { .. } => {
                let character = self.key_character(&action).unwrap();
                // shift 상태에서 나온 글자는 배열의 기본 글자와 다르므로 이웃 확률을
                // 그대로 쓸 수 없다 — 그때는 확실한 신호로 둔다
                let signal = if self.shifted() {
                    KeySignal::certain(character)
                } else {
                    hit::key_signal_at(self.layout(), x, y, character)
                };
                let layout_changed = self.shifted();
                self.shift = ShiftState::Released;
                PressOutcome {
                    event: Some(InputEvent::Key(signal)),
                    layout_changed,
                    request: None,
                }
            }
            KeyAction::LayerSwitch { target } => {
                self.active_layer = (target as usize).min(self.layout_set.layers.len() - 1);
                self.shift = ShiftState::Released;
                self.rebuild();
                PressOutcome {
                    event: None,
                    layout_changed: true,
                    request: None,
                }
            }
            KeyAction::Text(text) => PressOutcome {
                event: Some(InputEvent::Text(text)),
                layout_changed: false,
                request: None,
            },
            KeyAction::Blank => PressOutcome {
                event: None,
                layout_changed: false,
                request: None,
            },
            KeyAction::LanguageSwitch => PressOutcome {
                event: None,
                layout_changed: false,
                request: Some(ShellRequest::NextLanguage),
            },
            KeyAction::Backspace => PressOutcome {
                event: Some(InputEvent::Backspace),
                layout_changed: false,
                request: None,
            },
            KeyAction::Space => PressOutcome {
                event: Some(InputEvent::Separator(' ')),
                layout_changed: false,
                request: None,
            },
            KeyAction::Enter => PressOutcome {
                event: Some(InputEvent::Separator('\n')),
                layout_changed: false,
                request: None,
            },
        }
    }

    /// 길게 눌러 고를 수 있는 변형 문자를 실제 입력 이벤트로 바꾼다. 팝업에서 고른
    /// 결과도 일반 키 입력과 같은 경로로 흐른다.
    pub fn select_alternate(&mut self, alternate: &str) -> Option<InputEvent> {
        self.shift = ShiftState::Released;
        alternate
            .chars()
            .next()
            .map(|character| InputEvent::Key(KeySignal::certain(character)))
    }

    pub fn begin_cursor_drag(&mut self, x: f32) {
        self.cursor_drag = Some(CursorDrag {
            origin_x: x,
            emitted_steps: 0,
        });
    }

    /// 드래그 중 새로 발생한 이동 칸수(부호 = 방향). 드래그 중이 아니면 0.
    pub fn update_cursor_drag(&mut self, x: f32) -> i32 {
        let step = CURSOR_DRAG_STEP_POINTS / self.metrics.width_points.max(1.0);
        let Some(drag) = &mut self.cursor_drag else {
            return 0;
        };
        let total_steps = ((x - drag.origin_x) / step).trunc() as i32;
        let delta = total_steps - drag.emitted_steps;
        drag.emitted_steps = total_steps;
        delta
    }

    pub fn end_cursor_drag(&mut self) {
        self.cursor_drag = None;
    }
}
