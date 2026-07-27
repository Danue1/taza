//! shift 상태기계. 순정 관습이 이 작은 상태에 모여 있다 — 일회성 shift는 글자 하나로
//! 풀리고, 고정 shift는 심볼면을 다녀와도 남으며, 문맥이 올린 shift만 문맥이 내린다.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShiftState {
    #[default]
    Released,
    /// 한 글자 입력 후 자동 해제되는 일회성 shift (모바일 관습)
    Pressed,
    /// 다시 누를 때까지 유지되는 고정 shift — 대소문자가 있는 배열에서만 걸린다
    Locked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Shift {
    state: ShiftState,
    /// 지금 올라간 shift가 사용자가 올린 것이 아니라 자동 대문자화가 올린 것인가.
    /// 자동으로 올린 것만 자동으로 내린다 — 사용자가 올린 shift를 문맥이 내려 버리면
    /// 방금 누른 키가 없던 일이 된다.
    from_auto: bool,
}

impl Shift {
    pub(crate) fn state(&self) -> ShiftState {
        self.state
    }

    pub(crate) fn engaged(&self) -> bool {
        self.state != ShiftState::Released
    }

    pub(crate) fn locked(&self) -> bool {
        self.state == ShiftState::Locked
    }

    /// shift 키를 눌렀다 — 올라가 있으면 내리고 아니면 올린다. 손으로 만든 상태는
    /// 문맥이 되돌리지 않는다.
    pub(crate) fn press(&mut self) {
        self.state = match self.engaged() {
            true => ShiftState::Released,
            false => ShiftState::Pressed,
        };
        self.from_auto = false;
    }

    /// shift 고정을 걸거나 푼다. 걸 수 있는 배열인지는 부르는 쪽이 정한다.
    pub(crate) fn toggle_lock(&mut self) {
        self.state = match self.state {
            ShiftState::Locked => ShiftState::Released,
            _ => ShiftState::Locked,
        };
        self.from_auto = false;
    }

    /// 일회성 shift를 한 번 쓴다 — 고정된 shift는 남는다. 프레임을 다시 그려야 하면 true.
    pub(crate) fn consume(&mut self) -> bool {
        if self.state != ShiftState::Pressed {
            return false;
        }
        self.state = ShiftState::Released;
        self.from_auto = false;
        true
    }

    /// 문맥이 요구하는 자동 대문자화를 반영한다. 사용자가 손으로 만든 상태는 건드리지
    /// 않는다 — 자동으로 올린 것만 자동으로 내린다. 프레임을 다시 그려야 하면 true.
    pub(crate) fn set_auto(&mut self, engaged: bool) -> bool {
        match (engaged, self.state) {
            (true, ShiftState::Released) => {
                self.state = ShiftState::Pressed;
                self.from_auto = true;
                true
            }
            (false, ShiftState::Pressed) if self.from_auto => {
                self.state = ShiftState::Released;
                self.from_auto = false;
                true
            }
            _ => false,
        }
    }

    /// 필드가 바뀌는 것처럼 상태를 처음으로 되돌려야 할 때.
    pub(crate) fn reset(&mut self) {
        *self = Shift::default();
    }
}
