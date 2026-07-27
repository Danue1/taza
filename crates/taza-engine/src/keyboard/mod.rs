//! 키보드 한 판 — 레이아웃 묶음 위에 지금 상태(레이어·shift·멀티탭·드래그)를 얹어
//! 두 가지를 낸다: 셸이 그대로 그리는 프레임과, 터치 좌표에 대한 판정.
//!
//! 두 갈래를 파일로 갈라 둔다. **표현**(`frame`)은 상태를 읽기만 하는 순수 계산이고,
//! **판정**(`hit`·`press`)은 좌표를 키로 옮기고 그 결과로 상태를 움직인다. 둘이 같은
//! `active` 레이아웃 하나를 보므로 눌린 자리와 그려진 자리가 어긋나지 않는다.

mod field;
mod frame;
mod geometry;
pub mod hit;
pub mod layouts;
mod metrics;
mod press;
mod shift;

pub use frame::{FrameKey, KeyLegend, KeyRole, KeyboardFrame};
pub use geometry::{KeyBounds, KeyPosition, row_bounds};
pub use hit::{KeyProbability, KeySignal};
pub use metrics::{FormFactor, FrameMetrics, KeyboardMetrics};
pub use press::{PressOutcome, ShellRequest};
pub use shift::ShiftState;

// 레이아웃 데이터 타입은 언어팩 와이어 타입이 원본이다 — 배열 추가는 팩 배포로 끝난다.
pub use crate::pack::layout::{KeyAction, KeyboardLayout, KeyboardLayoutSet, LayoutKey, LayoutRow};

use crate::contract::{Capitalization, FieldKind, FieldTraits, UserPreferences};
use crate::lang::LanguageDescriptor;

use frame::FrameView;
use geometry::number_row;
use press::CursorDrag;
use shift::Shift;

/// 레이아웃 묶음 + 레이어·shift 상태에서 프레임을 만들고, 터치 좌표를 InputEvent로
/// 판정한다.
pub struct Keyboard {
    layout_set: KeyboardLayoutSet,
    language: LanguageDescriptor,
    metrics: KeyboardMetrics,
    active_layer: usize,
    shift: Shift,
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
    /// 문자면이 한글로 짜였는가. 레이아웃 묶음은 키보드가 사는 동안 바뀌지 않으므로
    /// 한 번만 본다 — 문자면 복귀 라벨과 shift 고정 지원이 여기서 갈린다.
    hangul_letters: bool,
}

/// 이어 누르는 중인 멀티탭 키.
struct Multitap {
    key: KeyPosition,
    /// 지금 나와 있는 글자가 주기의 몇 번째인가
    index: usize,
}

impl Keyboard {
    pub fn new(layout_set: KeyboardLayoutSet, language: LanguageDescriptor) -> Self {
        assert!(!layout_set.layers.is_empty(), "레이어가 최소 1개 필요");
        let mut keyboard = Keyboard {
            hangul_letters: frame::uses_hangul_letters(&layout_set.layers[0]),
            layout_set,
            language,
            metrics: KeyboardMetrics::default(),
            active_layer: 0,
            shift: Shift::default(),
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
        self.shift.reset();
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

    /// shift 고정(캡스 락)을 걸 수 있는 배열인지. 한글처럼 shift가 대문자가 아니라
    /// 다른 자모를 내는 배열에서는 고정할 것이 없다.
    pub fn supports_shift_lock(&self) -> bool {
        !self.hangul_letters
    }

    /// shift 키를 두 번 눌렀을 때(순정 관례) 고정을 걸거나 푼다. 두 번 누름을 알아보는
    /// 것은 플랫폼 제스처라 셸이 하고, 걸 수 있는지와 그 뒤 상태는 코어가 정한다.
    pub fn toggle_shift_lock(&mut self) -> bool {
        if !self.supports_shift_lock() {
            return false;
        }
        self.shift.toggle_lock();
        true
    }

    /// 문장 시작 여부를 shift에 반영한다. 판단(지금이 문장 첫 자리인가)은 Engine이
    /// 문맥으로 하고, 여기서는 사용자가 손으로 만든 shift 상태를 지키며 반영만 한다.
    /// 프레임을 다시 그려야 하면 true.
    pub fn set_auto_shift(&mut self, engaged: bool) -> bool {
        if self.shift.locked() || !self.supports_shift_lock() {
            return false;
        }
        self.shift.set_auto(engaged)
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

    /// 지금 상태로 프레임을 짓는 데 필요한 것들. 표현 규칙은 이 뷰 너머에만 있다.
    fn view(&self) -> FrameView<'_> {
        FrameView {
            layout: &self.active,
            language: &self.language,
            preferences: &self.preferences,
            traits: self.traits,
            metrics: self.metrics,
            shift: self.shift,
            hangul_letters: self.hangul_letters,
        }
    }

    /// 지금 레이아웃·폼팩터에 맞는 실측 치수. 프레임을 다시 받지 않고 높이만
    /// 필요할 때(입력 뷰 높이 제약) 쓴다.
    pub fn frame_metrics(&self) -> FrameMetrics {
        self.view().frame_metrics()
    }

    pub fn frame(&self) -> KeyboardFrame {
        self.view().frame()
    }

    fn key_position_at(&self, x: f32, y: f32) -> KeyPosition {
        hit::key_position_at(&self.active, x, y)
    }

    /// 좌표에 해당하는 키의 프레임 정보. 셸이 길게 누르기 대상을 알아내는 통로다 —
    /// 판정 기준(스냅 규칙)이 탭과 같아야 하므로 코어가 제공한다.
    pub fn key_at(&self, x: f32, y: f32) -> FrameKey {
        let position = self.key_position_at(x, y);
        self.frame().rows[position.row][position.index].clone()
    }
}
