//! 표시 환경과 실측 치수. 셸은 플랫폼 값을 폼팩터로 옮겨 주기만 하고, 행 높이·글꼴
//! 크기 같은 수치는 여기서 정한다 — 폼팩터 갈래가 셸에 생기지 않게 하는 자리다.

/// 글자 배율의 상한. 이보다 키우면 라벨이 키 밖으로 번진다 — 접근성 크기 단계는
/// 본문 글꼴 기준 2배를 넘지만, 키 하나에 글자 하나가 들어가야 하는 자리에는 그대로
/// 쓸 수 없다.
const TEXT_SCALE_LIMIT: f32 = 1.4;

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
    pub(crate) fn key_row_height_points(self) -> f32 {
        match self {
            FormFactor::PhonePortrait => 54.0,
            FormFactor::PhoneLandscape => 40.0,
            FormFactor::Tablet => 62.0,
        }
    }

    /// 후보 바 높이 — 낱말 한 줄이 서는 자리다. 순정 예측 바는 키 한 행보다 눈에 띄게
    /// 낮아, 여기가 두꺼우면 위쪽이 비어 보인다.
    pub(crate) fn candidate_bar_height_points(self) -> f32 {
        match self {
            FormFactor::PhonePortrait => 30.0,
            FormFactor::PhoneLandscape => 27.0,
            FormFactor::Tablet => 38.0,
        }
    }

    /// 순정 문자 키 글자는 키 높이에 견줘 큼직하다 — 22pt로는 작아 보인다.
    pub(crate) fn letter_font_size_points(self) -> f32 {
        match self {
            FormFactor::PhonePortrait => 25.0,
            FormFactor::PhoneLandscape => 22.0,
            FormFactor::Tablet => 28.0,
        }
    }

    /// 기호 하나로 된 제어 키(⇧·⌫·⏎·☺) 글자 — 글자 키만큼은 아니어도 큼직해야 눈에 든다.
    pub(crate) fn control_font_size_points(self) -> f32 {
        match self {
            FormFactor::PhonePortrait => 22.0,
            FormFactor::PhoneLandscape => 20.0,
            FormFactor::Tablet => 24.0,
        }
    }

    /// 낱말로 된 제어 키(ABC·한글·123·#+=·검색) 글자 — 기호 한 자보다 자리를 많이 쓰므로
    /// 순정도 여기만 한 단계 작게 잡는다.
    pub(crate) fn word_font_size_points(self) -> f32 {
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

impl KeyboardMetrics {
    /// 실제로 곱하는 글자 배율. 아주 큰 설정에서도 라벨이 키를 넘지 않도록 위를 막는다 —
    /// 접근성 크기를 그대로 곱하면 글자가 옆 키를 침범한다.
    pub(crate) fn clamped_text_scale(&self) -> f32 {
        self.text_scale.clamp(1.0, TEXT_SCALE_LIMIT)
    }
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
