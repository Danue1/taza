mod field;
pub mod hit;
pub mod layouts;

pub use hit::{KeyProbability, KeySignal};

use crate::contract::{
    Capitalization, FieldKind, FieldTraits, InputEvent, ReturnKey, UserPreferences,
};
use crate::lang::LanguageDescriptor;

/// 숫자 행 한 칸의 폭 — 열 칸이 한 줄을 가득 채운다.
const NUMBER_ROW_KEY_WIDTH: f32 = 0.1;
/// 숫자 행의 높이 — 글자 행보다 낮게 잡는다. 순정에 없는 줄이라 자리를 덜 차지해야
/// 문자 행이 좁아 보이지 않는다.
const NUMBER_ROW_HEIGHT: f32 = 0.8;

/// 숫자 행의 키와 길게 눌러 나오는 것들. 숫자 하나에 딸린 기호는 대개 그 숫자로 부르는
/// 것이라(1→느낌표, 6→탈자 부호) 심볼면까지 가지 않고 그 자리에서 닿는다.
const NUMBER_ROW_KEYS: [(char, &str); 10] = [
    ('1', "!¹½"),
    ('2', "@²"),
    ('3', "#³"),
    ('4', "$₩¢£¥€"),
    ('5', "%‰"),
    ('6', "^"),
    ('7', "&"),
    ('8', "*"),
    ('9', "("),
    ('0', ")°"),
];

/// 커서를 한 칸 옮기는 데 필요한 가로 이동 거리(pt). 순정처럼 손가락을 따라 커서가
/// 흐르려면 이 값이 글자 하나의 폭에 가까워야 한다 — 크게 잡으면 커서가 손가락을
/// 뒤늦게 따라오며 툭툭 건너뛴다. 정규화 좌표가 아니라 물리 거리로 잡아야 화면이
/// 넓어져도 손가락이 같은 만큼 움직인다.
const CURSOR_DRAG_STEP_POINTS: f32 = 5.0;

/// 멀티탭 주기가 살아 있는 시간(밀리초). 짧으면 이어 누르려던 손이 새 글자를 내고,
/// 길면 같은 자음을 잇달아 치는 낱말("학교")에서 앞 글자가 갈린다.
const MULTITAP_TIMEOUT_MILLISECONDS: u32 = 700;

/// 글자 배율의 상한. 이보다 키우면 라벨이 키 밖으로 번진다 — 접근성 크기 단계는
/// 본문 글꼴 기준 2배를 넘지만, 키 하나에 글자 하나가 들어가야 하는 자리에는 그대로
/// 쓸 수 없다.
const TEXT_SCALE_LIMIT: f32 = 1.4;

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

    /// 후보 바 높이 — 낱말 한 줄이 서는 자리다. 순정 예측 바는 키 한 행보다 눈에 띄게
    /// 낮아, 여기가 두꺼우면 위쪽이 비어 보인다.
    fn candidate_bar_height_points(self) -> f32 {
        match self {
            FormFactor::PhonePortrait => 30.0,
            FormFactor::PhoneLandscape => 27.0,
            FormFactor::Tablet => 38.0,
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

    /// 기호 하나로 된 제어 키(⇧·⌫·⏎·☺) 글자 — 글자 키만큼은 아니어도 큼직해야 눈에 든다.
    fn control_font_size_points(self) -> f32 {
        match self {
            FormFactor::PhonePortrait => 22.0,
            FormFactor::PhoneLandscape => 20.0,
            FormFactor::Tablet => 24.0,
        }
    }

    /// 낱말로 된 제어 키(ABC·한글·123·#+=·검색) 글자 — 기호 한 자보다 자리를 많이 쓰므로
    /// 순정도 여기만 한 단계 작게 잡는다.
    fn word_font_size_points(self) -> f32 {
        match self {
            FormFactor::PhonePortrait => 15.0,
            FormFactor::PhoneLandscape => 14.0,
            FormFactor::Tablet => 17.0,
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
    /// 시스템 글자 크기 설정이 요구하는 배율. 셸이 플랫폼 값(iOS Dynamic Type,
    /// Android fontScale)에서 옮겨 준다.
    ///
    /// **글꼴에만 곱하고 키 높이에는 곱하지 않는다** — 순정 키보드도 글자 크기를 키우면
    /// 라벨만 커지고 판은 그대로다. 판까지 커지면 화면의 절반을 키보드가 먹는다.
    pub text_scale: f32,
}

impl Default for KeyboardMetrics {
    /// 셸이 아직 자기 크기를 모르는 첫 프레임용 기본값 — 표준 폰 세로.
    fn default() -> Self {
        KeyboardMetrics {
            form_factor: FormFactor::PhonePortrait,
            width_points: 390.0,
            text_scale: 1.0,
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
    /// 글자 키 글꼴 — 변형 문자 팝업처럼 키 밖에서 같은 크기를 써야 하는 자리가 쓴다.
    /// 키 하나하나의 글꼴은 `FrameKey::font_size`에 실려 간다.
    pub letter_font_size: f32,
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

/// 문자면 위에 얹는 숫자 행. 배열과 무관하게 같은 줄이므로(어느 언어든 숫자는 아라비아
/// 숫자다) 팩 데이터가 아니라 코어가 만든다 — 배열마다 이 줄을 다시 싣게 하지 않는다.
fn number_row() -> LayoutRow {
    LayoutRow {
        keys: NUMBER_ROW_KEYS
            .iter()
            .map(|(digit, alternates)| LayoutKey {
                action: KeyAction::Character {
                    base: *digit,
                    shifted: *digit,
                },
                width_ratio: NUMBER_ROW_KEY_WIDTH,
                alternates: alternates.chars().collect(),
            })
            .collect(),
        height_ratio: NUMBER_ROW_HEIGHT,
    }
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
    /// 다시 누를 때까지 유지되는 고정 shift — 대소문자가 있는 배열에서만 걸린다
    Locked,
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

/// 낱말로 적히는 키의 갈래. 글자·기호 키는 여기 오지 않는다 — 그 라벨은 어느 나라
/// 말로 보든 같은 글자다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyLegend {
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

impl From<ReturnKey> for KeyLegend {
    fn from(key: ReturnKey) -> Self {
        match key {
            ReturnKey::Return => KeyLegend::Return,
            ReturnKey::Go => KeyLegend::Go,
            ReturnKey::Search => KeyLegend::Search,
            ReturnKey::Send => KeyLegend::Send,
            ReturnKey::Next => KeyLegend::Next,
            ReturnKey::Done => KeyLegend::Done,
            ReturnKey::Join => KeyLegend::Join,
            ReturnKey::Route => KeyLegend::Route,
            ReturnKey::Continue => KeyLegend::Continue,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrameKey {
    pub position: KeyPosition,
    pub label: String,
    pub bounds: KeyBounds,
    /// VoiceOver/TalkBack에 노출할 라벨 — 접근성은 비통일 영역이 아니라 계약의 일부
    /// 사람 말로 적히는 키 — 글자가 아니라 낱말이라 화면 언어를 탄다. 어느 낱말인지는
    /// 코어가 정하고(필드가 시키는 동작), 그 말을 무엇으로 적을지는 셸이 정한다.
    pub legend: Option<KeyLegend>,
    pub shift_active: bool,
    /// 이 필드에서 눈에 띄어야 하는 키 — 검색 필드의 리턴키처럼 순정이 강조색을 쓰는
    /// 자리다. 어떤 색인지는 셸의 디자인 시스템이 정한다.
    pub emphasized: bool,
    pub role: KeyRole,
    /// 이 키 라벨의 글꼴 크기(pt) — 글자 키·기호 제어 키·낱말 제어 키가 서로 다르다.
    pub font_size: f32,
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
    /// 멀티탭 주기를 끊을 시한(밀리초). 이어 누르면 새로 시작한다.
    pub timer: Option<u32>,
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
    /// 지금 올라간 shift가 사용자가 올린 것이 아니라 자동 대문자화가 올린 것인가.
    /// 자동으로 올린 것만 자동으로 내린다 — 사용자가 올린 shift를 문맥이 내려 버리면
    /// 방금 누른 키가 없던 일이 된다.
    shift_from_auto: bool,
    cursor_drag: Option<CursorDrag>,
    /// 편집 대상이 밝힌 성격 — 배열·리턴키·보조 기능이 여기서 갈린다
    traits: FieldTraits,
    preferences: UserPreferences,
    /// 레이어와 필드를 모두 적용한 배열. 프레임과 히트 테스트가 같은 것을 봐야 눌린
    /// 자리와 그려진 자리가 어긋나지 않으므로 한 번 만들어 두고 함께 쓴다.
    active: KeyboardLayout,
    /// 지금 돌고 있는 멀티탭 주기. 같은 키를 이어 눌렀는지를 자리로 판정한다 —
    /// 시각은 셸의 타이머가 재고, 다 되면 `expire_multitap`으로 끊어 준다.
    multitap: Option<Multitap>,
}

/// 이어 누르는 중인 멀티탭 키.
struct Multitap {
    key: KeyPosition,
    /// 지금 나와 있는 글자가 주기의 몇 번째인가
    index: usize,
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
        let mut keyboard = Keyboard {
            layout_set,
            language,
            metrics: KeyboardMetrics::default(),
            active_layer: 0,
            shift: ShiftState::Released,
            shift_from_auto: false,
            cursor_drag: None,
            traits: FieldTraits::default(),
            preferences: UserPreferences::default(),
            active: KeyboardLayout {
                rows: Vec::new(),
                panel_rows: 0.0,
            },
            multitap: None,
        };
        keyboard.rebuild();
        keyboard
    }

    /// 셸이 자기 크기·폼팩터를 알게 될 때마다(첫 배치, 회전, 분할, 글자 크기 변경)
    /// 주입한다.
    pub fn set_metrics(&mut self, metrics: KeyboardMetrics) {
        self.metrics = metrics;
    }

    /// 글자 크기 배율. 아주 큰 설정에서도 라벨이 키를 넘지 않도록 위를 막는다 —
    /// 접근성 크기를 그대로 곱하면 글자가 옆 키를 침범한다.
    fn text_scale(&self) -> f32 {
        self.metrics.text_scale.clamp(1.0, TEXT_SCALE_LIMIT)
    }

    /// 화면을 바꾸는 설정(숫자 행·키보드 높이·변형 문자·커서 감도)이 여기로 들어온다.
    pub fn set_preferences(&mut self, preferences: UserPreferences) {
        let rebuilds = self.preferences.number_row != preferences.number_row;
        self.preferences = preferences;
        if rebuilds {
            self.rebuild();
        }
    }

    /// 셸이 편집 대상이 바뀔 때마다 알려 주는 필드 성격. 배열·리턴키·후보 바 자리가
    /// 여기서 갈리므로(`docs/inputmode.md`) 셸은 값만 넘기고 판단하지 않는다.
    pub fn set_field(&mut self, traits: FieldTraits) {
        if self.traits == traits {
            return;
        }
        let same_kind = self.traits.kind == traits.kind;
        self.traits = traits;
        if same_kind {
            // 리턴키 낱말만 달라진 것이면 배열을 다시 만들 필요가 없다
            return;
        }
        // 숫자 패드에서 돌아올 때 문자면으로 되돌린다 — 필드가 바뀌면 직전 필드에서
        // 고른 레이어는 의미가 없다
        self.active_layer = 0;
        self.shift = ShiftState::Released;
        self.rebuild();
    }

    pub fn field(&self) -> FieldKind {
        self.traits.kind
    }

    pub fn traits(&self) -> FieldTraits {
        self.traits
    }

    /// 지금 자리에서 shift를 올려야 하는가 — 앱이 요구한 범위와 문맥을 함께 본다.
    /// 판단 재료(문장 시작 여부)는 Engine이 문맥에서 뽑아 넘긴다.
    pub(crate) fn capitalizes(&self, sentence_start: bool, word_start: bool) -> bool {
        if !self.traits.kind.auto_capitalizes() {
            return false;
        }
        match self.traits.capitalization {
            Capitalization::None => false,
            Capitalization::Sentences => sentence_start,
            Capitalization::Words => word_start,
            Capitalization::AllCharacters => true,
        }
    }

    fn rebuild(&mut self) {
        let mut layout = field::apply(&self.layout_set.layers[self.active_layer], self.traits.kind);
        // 숫자 행은 문자면에만 붙는다 — 심볼면에는 이미 숫자가 있고, 숫자 패드와 검색면은
        // 행을 늘릴 자리가 아니다
        if self.preferences.number_row
            && self.active_layer == 0
            && layout.panel_rows <= 0.0
            && !field::uses_number_pad(self.traits.kind)
        {
            layout.rows.insert(0, number_row());
        }
        self.active = layout;
    }

    /// 지금 레이아웃·폼팩터에 맞는 실측 치수. 프레임을 다시 받지 않고 높이만
    /// 필요할 때(입력 뷰 높이 제약) 쓴다.
    pub fn frame_metrics(&self) -> FrameMetrics {
        let form_factor = self.metrics.form_factor;
        // 행 높이는 표준 행 대비 배수이므로, 그 합이 곧 몇 행치 높이인지가 된다
        let rows = layer_rows(self.layout());
        let scale = self.preferences.keyboard_height.scale();
        let text_scale = self.text_scale();
        FrameMetrics {
            grid_height: form_factor.key_row_height_points() * rows * scale,
            // 후보를 내지 않는 필드에서는 바 자리를 없애 키보드가 낮아진다(순정 실측)
            candidate_bar_height: if field::shows_candidate_bar(self.traits.kind)
                || self.preferences.candidate_bar_always
            {
                form_factor.candidate_bar_height_points()
            } else {
                0.0
            },
            letter_font_size: form_factor.letter_font_size_points() * text_scale,
        }
    }

    /// 키 하나에 쓸 글꼴 크기 — 라벨이 글자 하나인지 낱말인지에 따라 갈린다. 셸이 라벨을
    /// 들여다보고 판단하지 않도록 코어가 실측값으로 내려 준다.
    fn key_font_size(&self, action: &KeyAction, label: &str) -> f32 {
        let form_factor = self.metrics.form_factor;
        let base = match action {
            KeyAction::Character { .. } | KeyAction::Text(_) => {
                form_factor.letter_font_size_points()
            }
            _ if label.chars().count() > 1 => form_factor.word_font_size_points(),
            _ => form_factor.control_font_size_points(),
        };
        base * self.text_scale()
    }

    fn layout(&self) -> &KeyboardLayout {
        &self.active
    }

    fn shifted(&self) -> bool {
        self.shift != ShiftState::Released
    }

    /// 문자면이 한글로 짜였는지 — 문자면 복귀 라벨과 shift 고정 지원이 여기서 갈린다.
    fn uses_hangul_letters(&self) -> bool {
        self.layout_set.layers[0]
            .rows
            .iter()
            .flat_map(|row| &row.keys)
            .any(|key| matches!(key.action, KeyAction::Character { base, .. } if is_hangul_script(base)))
    }

    /// shift 고정(캡스 락)을 걸 수 있는 배열인지. 한글처럼 shift가 대문자가 아니라
    /// 다른 자모를 내는 배열에서는 고정할 것이 없다.
    pub fn supports_shift_lock(&self) -> bool {
        !self.uses_hangul_letters()
    }

    /// shift 키를 두 번 눌렀을 때(순정 관례) 고정을 걸거나 푼다. 두 번 누름을 알아보는
    /// 것은 플랫폼 제스처라 셸이 하고, 걸 수 있는지와 그 뒤 상태는 코어가 정한다.
    pub fn toggle_shift_lock(&mut self) -> bool {
        if !self.supports_shift_lock() {
            return false;
        }
        self.shift = match self.shift {
            ShiftState::Locked => ShiftState::Released,
            _ => ShiftState::Locked,
        };
        self.shift_from_auto = false;
        true
    }

    /// 문장 시작 여부를 shift에 반영한다. 판단(지금이 문장 첫 자리인가)은 Engine이
    /// 문맥으로 하고, 여기서는 사용자가 손으로 만든 shift 상태를 지키며 반영만 한다.
    /// 프레임을 다시 그려야 하면 true.
    pub fn set_auto_shift(&mut self, engaged: bool) -> bool {
        if self.shift == ShiftState::Locked || !self.supports_shift_lock() {
            return false;
        }
        match (engaged, self.shift) {
            (true, ShiftState::Released) => {
                self.shift = ShiftState::Pressed;
                self.shift_from_auto = true;
                true
            }
            (false, ShiftState::Pressed) if self.shift_from_auto => {
                self.shift = ShiftState::Released;
                self.shift_from_auto = false;
                true
            }
            _ => false,
        }
    }

    /// 레이어 전환 키 라벨 — 순정 관례: 문자면 복귀는 스크립트에 맞게(ABC/한글),
    /// 심볼 1·2면은 123 / #+=. 비밀번호 필드에서는 순정처럼 `.?123`이 된다.
    fn layer_switch_label(&self, target: u8) -> String {
        match target {
            1 if self.traits.kind == FieldKind::Password => ".?123".to_string(),
            0 => if self.uses_hangul_letters() {
                "한글"
            } else {
                "ABC"
            }
            .to_string(),
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

    /// 리턴키가 시키는 동작 — 순정은 필드마다 다른 낱말을 적는다. 낱말 자체는 셸이
    /// 화면 언어로 옮긴다.
    fn key_legend(&self, action: &KeyAction) -> Option<KeyLegend> {
        if *action != KeyAction::Enter {
            return None;
        }
        Some(KeyLegend::from(self.traits.return_key))
    }

    fn key_label(&self, action: &KeyAction) -> String {
        match action {
            KeyAction::Character { .. } => {
                crate::lang::keycap_form(self.key_character(action).unwrap()).to_string()
            }
            KeyAction::Text(text) => text.clone(),
            // 주기에 든 글자를 모두 적는다 — 몇 번 눌러야 무엇이 나오는지가 키에 보여야 한다
            KeyAction::Multitap(cycle) => cycle
                .iter()
                .copied()
                .map(crate::lang::keycap_form)
                .collect(),
            // 고정된 shift는 순정처럼 다른 기호로 알린다 — 한 번 누르면 풀린다는 뜻이
            // 라벨에서 드러나야 한다
            KeyAction::Shift => if self.shift == ShiftState::Locked {
                "⇪"
            } else {
                "⇧"
            }
            .to_string(),
            KeyAction::Backspace => "⌫".to_string(),
            KeyAction::Space => self.language.display_name.clone(),
            KeyAction::Enter => "⏎".to_string(),
            KeyAction::LayerSwitch { target } => self.layer_switch_label(*target),
            KeyAction::LanguageSwitch => self.language.keycap_label.clone(),
            KeyAction::Blank => String::new(),
        }
    }

    fn key_role(&self, action: &KeyAction) -> KeyRole {
        match action {
            KeyAction::Character { .. } | KeyAction::Text(_) | KeyAction::Multitap(_) => {
                KeyRole::Character
            }
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

    /// 길게 눌러 고를 수 있는 것들 — 배열이 밝힌 변형 앞에 지금 누르고 있는 글자를 세운다.
    /// 그래야 팝업이 열린 뒤에도 손을 그대로 떼면 치던 글자가 들어간다(순정 관례).
    ///
    /// shift가 올라가 있으면 변형도 대문자로 나온다 — 순정이 그렇고, 그러지 않으면
    /// É·Ü·Ç 같은 글자에 닿을 길이 아예 없어진다. 독일어는 명사가 모두 대문자로 시작하고
    /// 프랑스어는 문장 첫 É가 흔하므로, QWERTZ·AZERTY에서는 이 길이 막히면 곤란하다.
    fn key_alternates(&self, action: &KeyAction, declared: &[char]) -> Vec<String> {
        if !self.preferences.key_alternates {
            return Vec::new();
        }
        let cased = |character: char| {
            if self.shifted() {
                crate::pack::layout::uppercase(character)
            } else {
                character
            }
        };
        let Some(character) = self.key_character(action) else {
            return declared.iter().map(|&c| cased(c).to_string()).collect();
        };
        if declared.is_empty() {
            return Vec::new();
        }
        // 누르고 있는 글자는 이미 shift가 반영된 것이라 다시 올리지 않는다
        std::iter::once(character)
            .chain(declared.iter().copied().map(cased))
            .map(|character| character.to_string())
            .collect()
    }

    pub fn frame(&self) -> KeyboardFrame {
        let rows = (0..self.layout().rows.len())
            .map(|row_index| {
                let bounds = self.row_bounds(row_index);
                self.layout().rows[row_index]
                    .keys
                    .iter()
                    .enumerate()
                    .map(|(key_index, key)| {
                        let label = self.key_label(&key.action);
                        FrameKey {
                            position: KeyPosition {
                                row: row_index,
                                index: key_index,
                            },
                            font_size: self.key_font_size(&key.action, &label),
                            label,
                            bounds: bounds[key_index],
                            legend: self.key_legend(&key.action),
                            shift_active: key.action == KeyAction::Shift && self.shifted(),
                            emphasized: key.action == KeyAction::Enter
                                && self.traits.kind == FieldKind::Search,
                            role: self.key_role(&key.action),
                            alternates: self.key_alternates(&key.action, &key.alternates),
                        }
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
        if let KeyAction::Multitap(cycle) = action {
            return self.press_multitap(position, &cycle, x, y);
        }
        // 주기를 끊는 것은 시각만이 아니다 — 다른 키로 손이 옮겨 가면 그 자리에서 끝난다
        self.multitap = None;
        match action {
            KeyAction::Shift => {
                self.shift = if self.shifted() {
                    ShiftState::Released
                } else {
                    ShiftState::Pressed
                };
                // 손으로 만든 상태는 문맥이 되돌리지 않는다
                self.shift_from_auto = false;
                PressOutcome {
                    event: None,
                    layout_changed: true,
                    request: None,
                    timer: None,
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
                // 고정된 shift는 글자를 넣어도 풀리지 않는다 — 그 외에는 한 글자만 받는다
                let layout_changed = self.shift == ShiftState::Pressed;
                if self.shift == ShiftState::Pressed {
                    self.shift = ShiftState::Released;
                    self.shift_from_auto = false;
                }
                PressOutcome {
                    event: Some(InputEvent::Key(signal)),
                    layout_changed,
                    request: None,
                    timer: None,
                }
            }
            KeyAction::LayerSwitch { target } => {
                self.active_layer = (target as usize).min(self.layout_set.layers.len() - 1);
                // 고정된 shift는 심볼면을 다녀와도 남는다(순정 관례)
                if self.shift == ShiftState::Pressed {
                    self.shift = ShiftState::Released;
                    self.shift_from_auto = false;
                }
                self.rebuild();
                PressOutcome {
                    event: None,
                    layout_changed: true,
                    request: None,
                    timer: None,
                }
            }
            KeyAction::Text(text) => PressOutcome {
                event: Some(InputEvent::Text(text)),
                layout_changed: false,
                request: None,
                timer: None,
            },
            KeyAction::Blank => PressOutcome {
                event: None,
                layout_changed: false,
                request: None,
                timer: None,
            },
            KeyAction::LanguageSwitch => PressOutcome {
                event: None,
                layout_changed: false,
                request: Some(ShellRequest::NextLanguage),
                timer: None,
            },
            KeyAction::Backspace => PressOutcome {
                event: Some(InputEvent::Backspace),
                layout_changed: false,
                request: None,
                timer: None,
            },
            KeyAction::Space => PressOutcome {
                event: Some(InputEvent::Separator(' ')),
                layout_changed: false,
                request: None,
                timer: None,
            },
            KeyAction::Enter => PressOutcome {
                event: Some(InputEvent::Separator('\n')),
                layout_changed: false,
                request: None,
                timer: None,
            },
            KeyAction::Multitap(_) => unreachable!("멀티탭 키는 앞에서 처리했다"),
        }
    }

    /// 이어 누르면 주기의 다음 글자로 갈아 끼운다. 같은 키인지는 자리로 보고, 주기가
    /// 아직 살아 있는지는 셸이 재는 시각(`expire_multitap`)이 정한다.
    fn press_multitap(&mut self, key: KeyPosition, cycle: &[char], x: f32, y: f32) -> PressOutcome {
        let Some(&first) = cycle.first() else {
            return PressOutcome {
                event: None,
                layout_changed: false,
                request: None,
                timer: None,
            };
        };
        // 이어 누른 것인지는 주기가 살아 있었는가로 정한다. 주기가 한 바퀴 돌아
        // 첫 글자로 되돌아온 것도 여전히 이어 누른 것이다 — 자리로 보지 않으면 그때
        // 갈아 끼우는 대신 글자가 하나 더 붙는다(ㄱ→ㅋ→ㄲ→ㄲㄱ).
        let continuing = matches!(&self.multitap, Some(current) if current.key == key);
        let index = match &self.multitap {
            Some(current) if current.key == key => (current.index + 1) % cycle.len(),
            _ => 0,
        };
        self.multitap = Some(Multitap { key, index });
        let character = cycle.get(index).copied().unwrap_or(first);
        PressOutcome {
            event: Some(match continuing {
                true => InputEvent::Retap(character),
                // 주기를 여는 첫 누름은 평범한 글자 입력이므로 이웃 확률도 그대로 싣는다
                false => InputEvent::Key(hit::key_signal_at(self.layout(), x, y, character)),
            }),
            layout_changed: false,
            request: None,
            timer: Some(MULTITAP_TIMEOUT_MILLISECONDS),
        }
    }

    /// 멀티탭 시한이 다 됐다 — 다음에 같은 키를 눌러도 새 글자로 시작한다.
    pub(crate) fn expire_multitap(&mut self) {
        self.multitap = None;
    }

    /// 길게 눌러 고를 수 있는 변형 문자를 실제 입력 이벤트로 바꾼다. 팝업에서 고른
    /// 결과도 일반 키 입력과 같은 경로로 흐른다.
    pub fn select_alternate(&mut self, alternate: &str) -> Option<InputEvent> {
        if self.shift == ShiftState::Pressed {
            self.shift = ShiftState::Released;
            self.shift_from_auto = false;
        }
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
        let step = CURSOR_DRAG_STEP_POINTS * self.preferences.cursor_sensitivity.step_scale()
            / self.metrics.width_points.max(1.0);
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
