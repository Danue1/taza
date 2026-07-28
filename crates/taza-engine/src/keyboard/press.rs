//! 판정 — 좌표와 시각이 상태를 움직이는 자리. 어느 키를 눌렀는지는 `hit`가 기하로
//! 정하고, 그 키가 무엇을 뜻하는지(레이어·shift·멀티탭 주기·커서 이동)는 여기서 정한다.
//! 표현 규칙은 `frame`에 있고 이 파일은 라벨을 짓지 않는다.

use crate::contract::InputEvent;
use crate::keyboard::layout::KeyAction;

use super::geometry::KeyPosition;
use super::hit::{self, KeySignal};
use super::{Keyboard, Multitap};

/// 커서를 한 칸 옮기는 데 필요한 가로 이동 거리(pt). 순정처럼 손가락을 따라 커서가
/// 흐르려면 이 값이 글자 하나의 폭에 가까워야 한다 — 크게 잡으면 커서가 손가락을
/// 뒤늦게 따라오며 툭툭 건너뛴다. 정규화 좌표가 아니라 물리 거리로 잡아야 화면이
/// 넓어져도 손가락이 같은 만큼 움직인다.
const CURSOR_DRAG_STEP_POINTS: f32 = 5.0;

/// 멀티탭 주기가 살아 있는 시간(밀리초). 짧으면 이어 누르려던 손이 새 글자를 내고,
/// 길면 같은 자음을 잇달아 치는 낱말("학교")에서 앞 글자가 갈린다.
const MULTITAP_TIMEOUT_MILLISECONDS: u32 = 700;

/// 코어가 판정할 수 없는, 셸이 소유한 상태에 대한 요청. 언어 목록·순서는 셸(설정)
/// 소관이므로 코어는 "다음 언어로" 같은 요청만 낸다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellRequest {
    NextLanguage,
    /// 태그가 가리키는 언어로 곧장. 그 언어를 쓰고 있지 않으면 셸이 흘려보낸다 —
    /// 무엇을 쓰고 있는지는 셸만 안다.
    Language(String),
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

impl PressOutcome {
    /// 상태만 바꾸고 입력을 내지 않는 누름 — shift·레이어처럼 화면만 달라지는 키.
    fn silent(layout_changed: bool) -> Self {
        PressOutcome {
            event: None,
            layout_changed,
            request: None,
            timer: None,
        }
    }

    /// 화면을 바꾸지 않고 입력만 내는 누름.
    fn emits(event: InputEvent) -> Self {
        PressOutcome {
            event: Some(event),
            layout_changed: false,
            request: None,
            timer: None,
        }
    }
}

/// 스페이스바를 길게 눌러 끄는 커서 이동의 진행 상태. 판정(몇 칸 옮길지)은 코어가,
/// 포인터 스트림 전달만 셸이 한다.
pub(super) struct CursorDrag {
    origin_x: f32,
    emitted_steps: i32,
}

impl Keyboard {
    pub fn press_at(&mut self, x: f32, y: f32) -> PressOutcome {
        let position = self.key_position_at(x, y);
        let action = self.active.rows[position.row].keys[position.index]
            .action
            .clone();
        if let KeyAction::Multitap(cycle) = action {
            return self.press_multitap(position, &cycle, x, y);
        }
        // 주기를 끊는 것은 시각만이 아니다 — 다른 키로 손이 옮겨 가면 그 자리에서 끝난다
        self.multitap = None;
        match action {
            KeyAction::Shift => {
                self.shift.press();
                PressOutcome::silent(true)
            }
            KeyAction::Character { .. } => {
                let character = self.view().key_character(&action).unwrap();
                // shift 상태에서 나온 글자는 배열의 기본 글자와 다르므로 이웃 확률을
                // 그대로 쓸 수 없다 — 그때는 확실한 신호로 둔다
                let signal = match self.shift.engaged() {
                    true => KeySignal::certain(character),
                    false => hit::key_signal_at(&self.active, x, y, character),
                };
                // 고정된 shift는 글자를 넣어도 풀리지 않는다 — 그 외에는 한 글자만 받는다
                PressOutcome {
                    event: Some(InputEvent::Key(signal)),
                    layout_changed: self.shift.consume(),
                    request: None,
                    timer: None,
                }
            }
            KeyAction::LayerSwitch { target } => {
                self.active_layer = (target as usize).min(self.layout_set.layers.len() - 1);
                // 고정된 shift는 심볼면을 다녀와도 남는다(순정 관례)
                self.shift.consume();
                self.rebuild();
                PressOutcome::silent(true)
            }
            // 갈아 끼우는 키는 자기 글자를 내지 않으므로 이웃 확률에 뜻이 없다 — 이 자리를
            // 노려 빗나갔더라도 들어갔을 자모가 없고, marker가 조회 키에 섞이면 안 된다
            KeyAction::Compose { marker, .. } => {
                PressOutcome::emits(InputEvent::Key(KeySignal::certain(marker)))
            }
            KeyAction::Text(text) => PressOutcome::emits(InputEvent::Text(text)),
            KeyAction::Blank => PressOutcome::silent(false),
            KeyAction::LanguageSwitch => PressOutcome {
                event: None,
                layout_changed: false,
                request: Some(ShellRequest::NextLanguage),
                timer: None,
            },
            KeyAction::LanguageSelect { tag, .. } => PressOutcome {
                event: None,
                layout_changed: false,
                request: Some(ShellRequest::Language(tag)),
                timer: None,
            },
            KeyAction::Backspace => PressOutcome::emits(InputEvent::Backspace),
            // 커서를 옮기기 전에 조합이 확정된다(`InputEvent::CursorDrag`) — 그것이
            // 이 키로 같은 자음을 잇달아 칠 수 있는 까닭이다
            KeyAction::CursorRight => PressOutcome::emits(InputEvent::CursorDrag(1)),
            KeyAction::Space => PressOutcome::emits(InputEvent::Separator(' ')),
            KeyAction::Enter => PressOutcome::emits(InputEvent::Separator('\n')),
            KeyAction::Multitap(_) => unreachable!("멀티탭 키는 앞에서 처리했다"),
        }
    }

    /// 이어 누르면 주기의 다음 글자로 갈아 끼운다. 같은 키인지는 자리로 보고, 주기가
    /// 아직 살아 있는지는 셸이 재는 시각(`expire_multitap`)이 정한다.
    fn press_multitap(&mut self, key: KeyPosition, cycle: &[char], x: f32, y: f32) -> PressOutcome {
        let Some(&first) = cycle.first() else {
            return PressOutcome::silent(false);
        };
        // 이어 누른 것인지는 주기가 살아 있었는가로 정한다. 주기가 한 바퀴 돌아
        // 첫 글자로 되돌아온 것도 여전히 이어 누른 것이다 — 자리로 보지 않으면 그때
        // 갈아 끼우는 대신 글자가 하나 더 붙는다(ㄱ→ㅋ→ㄲ→ㄲㄱ).
        let continuing = matches!(&self.multitap, Some(current) if current.key == key);
        let index = match continuing {
            true => (self.multitap.as_ref().unwrap().index + 1) % cycle.len(),
            false => 0,
        };
        self.multitap = Some(Multitap { key, index });
        let character = cycle.get(index).copied().unwrap_or(first);
        PressOutcome {
            event: Some(match continuing {
                true => InputEvent::Retap(character),
                // 주기를 여는 첫 누름은 평범한 글자 입력이므로 이웃 확률도 그대로 싣는다
                false => InputEvent::Key(hit::key_signal_at(&self.active, x, y, character)),
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
    /// 결과도 일반 키 입력과 같은 경로로 흐르므로, 일회성 shift가 풀린 것까지
    /// 누름과 같은 결과로 낸다 — 셸이 그때 판을 다시 그린다.
    pub fn select_alternate(&mut self, alternate: &str) -> PressOutcome {
        let layout_changed = self.shift.consume();
        match alternate.chars().next() {
            Some(character) => PressOutcome {
                event: Some(InputEvent::Key(KeySignal::certain(character))),
                layout_changed,
                request: None,
                timer: None,
            },
            None => PressOutcome::silent(layout_changed),
        }
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
