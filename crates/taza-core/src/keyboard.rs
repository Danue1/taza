pub mod layouts;

use crate::session::InputEvent;

// 레이아웃 데이터 타입은 언어팩 와이어 타입이 원본이다 — 배열 추가는 팩 배포로 끝난다.
pub use taza_pack::layout::{
    KeyAction, KeyboardLayout, KeyboardLayoutSet, LayoutKey, LayoutRow,
};

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
    fn center_x(&self) -> f32 {
        self.x + self.width / 2.0
    }
}

/// 셸이 그대로 그리는 화면 명세. 셸은 px 변환과 렌더링만 하고 판단하지 않는다.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyboardFrame {
    pub rows: Vec<Vec<FrameKey>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrameKey {
    pub position: KeyPosition,
    pub label: String,
    pub bounds: KeyBounds,
    /// VoiceOver/TalkBack에 노출할 라벨 — 접근성은 비통일 영역이 아니라 계약의 일부
    pub accessibility_label: String,
    pub shift_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PressOutcome {
    pub event: Option<InputEvent>,
    /// shift 등 상태 변화로 프레임을 다시 그려야 하는지
    pub layout_changed: bool,
}

/// 레이아웃 묶음 + 레이어·shift 상태에서 프레임을 만들고, 터치 좌표를 InputEvent로
/// 판정한다. 히트 테스트는 v1 기하 판정(포함 → 같은 행 최근접) — 확률(공간) 모델은
/// 이 지점을 교체하는 방식으로 들어온다.
pub struct Keyboard {
    layout_set: KeyboardLayoutSet,
    active_layer: usize,
    shift: ShiftState,
}

impl Keyboard {
    pub fn new(layout_set: KeyboardLayoutSet) -> Self {
        assert!(!layout_set.layers.is_empty(), "레이어가 최소 1개 필요");
        Keyboard {
            layout_set,
            active_layer: 0,
            shift: ShiftState::Released,
        }
    }

    fn layout(&self) -> &KeyboardLayout {
        &self.layout_set.layers[self.active_layer]
    }

    fn shifted(&self) -> bool {
        self.shift == ShiftState::Pressed
    }

    /// 레이어 전환 키 라벨 — 순정 관례: 문자면 복귀는 스크립트에 맞게(ABC/한글),
    /// 심볼 1·2면은 123 / #+=
    fn layer_switch_label(&self, target: u8) -> String {
        match target {
            0 => {
                let uses_hangul = self.layout_set.layers[0].rows.iter().flat_map(|row| &row.keys).any(
                    |key| matches!(key.action, KeyAction::Character { base, .. }
                        if crate::composer::hangul::is_jamo(base)),
                );
                if uses_hangul { "한글" } else { "ABC" }.to_string()
            }
            1 => "123".to_string(),
            _ => "#+=".to_string(),
        }
    }

    fn key_character(&self, action: KeyAction) -> Option<char> {
        match action {
            KeyAction::Character { base, shifted } => {
                Some(if self.shifted() { shifted } else { base })
            }
            _ => None,
        }
    }

    fn key_label(&self, action: KeyAction) -> String {
        match action {
            KeyAction::Character { .. } => self.key_character(action).unwrap().to_string(),
            KeyAction::Shift => "⇧".to_string(),
            KeyAction::Backspace => "⌫".to_string(),
            KeyAction::Space => "␣".to_string(),
            KeyAction::Enter => "⏎".to_string(),
            KeyAction::LayerSwitch { target } => self.layer_switch_label(target),
        }
    }

    fn accessibility_label(&self, action: KeyAction) -> String {
        match action {
            KeyAction::Character { .. } => self.key_character(action).unwrap().to_string(),
            KeyAction::Shift => "shift".to_string(),
            KeyAction::Backspace => "backspace".to_string(),
            KeyAction::Space => "space".to_string(),
            KeyAction::Enter => "enter".to_string(),
            KeyAction::LayerSwitch { target } => match target {
                0 => "letters".to_string(),
                1 => "numbers".to_string(),
                _ => "symbols".to_string(),
            },
        }
    }

    fn row_bounds(&self, row_index: usize) -> Vec<KeyBounds> {
        let row_count = self.layout().rows.len();
        let row = &self.layout().rows[row_index];
        let height = 1.0 / row_count as f32;
        let y = row_index as f32 * height;
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
                        label: self.key_label(key.action),
                        bounds: bounds[key_index],
                        accessibility_label: self.accessibility_label(key.action),
                        shift_active: key.action == KeyAction::Shift && self.shifted(),
                    })
                    .collect()
            })
            .collect();
        KeyboardFrame { rows }
    }

    /// 좌표는 키보드 영역 밖이어도 가장 가까운 행·키로 스냅한다 — 가장자리 터치를
    /// 버리는 것보다 관대한 판정이 모바일 관습이다.
    fn key_position_at(&self, x: f32, y: f32) -> KeyPosition {
        let row_count = self.layout().rows.len();
        let row_index = ((y * row_count as f32).floor() as isize)
            .clamp(0, row_count as isize - 1) as usize;
        let bounds = self.row_bounds(row_index);
        let mut best_index = 0;
        let mut best_distance = f32::INFINITY;
        for (key_index, key_bounds) in bounds.iter().enumerate() {
            if x >= key_bounds.x && x < key_bounds.x + key_bounds.width {
                best_index = key_index;
                break;
            }
            let distance = (key_bounds.center_x() - x).abs();
            if distance < best_distance {
                best_distance = distance;
                best_index = key_index;
            }
        }
        KeyPosition {
            row: row_index,
            index: best_index,
        }
    }

    pub fn press_at(&mut self, x: f32, y: f32) -> PressOutcome {
        let position = self.key_position_at(x, y);
        let action = self.layout().rows[position.row].keys[position.index].action;
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
                }
            }
            KeyAction::Character { .. } => {
                let character = self.key_character(action).unwrap();
                let layout_changed = self.shifted();
                self.shift = ShiftState::Released;
                PressOutcome {
                    event: Some(InputEvent::Key(character)),
                    layout_changed,
                }
            }
            KeyAction::LayerSwitch { target } => {
                self.active_layer = (target as usize).min(self.layout_set.layers.len() - 1);
                self.shift = ShiftState::Released;
                PressOutcome {
                    event: None,
                    layout_changed: true,
                }
            }
            KeyAction::Backspace => PressOutcome {
                event: Some(InputEvent::Backspace),
                layout_changed: false,
            },
            KeyAction::Space => PressOutcome {
                event: Some(InputEvent::Separator(' ')),
                layout_changed: false,
            },
            KeyAction::Enter => PressOutcome {
                event: Some(InputEvent::Separator('\n')),
                layout_changed: false,
            },
        }
    }
}
