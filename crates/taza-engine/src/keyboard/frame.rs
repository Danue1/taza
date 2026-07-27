//! 화면 명세를 짓는 자리 — 표현만 있고 판정은 없다. 프레임은 지금 상태의 순수한 함수라
//! `FrameView`가 필요한 것을 모두 받아 만들며, 이 파일의 어떤 것도 상태를 바꾸지 않는다.
//! 좌표를 키로 옮기는 판정은 `hit`, 그 결과로 상태가 움직이는 것은 `press`의 몫이다.

use crate::contract::{FieldKind, FieldTraits, ReturnKey, UserPreferences};
use crate::lang::LanguageDescriptor;
use crate::pack::layout::{KeyAction, KeyboardLayout};

use super::field;
use super::geometry::{KeyBounds, KeyPosition, layer_rows, panel_height_ratio, row_bounds};
use super::metrics::{FrameMetrics, KeyboardMetrics};
use super::shift::{Shift, ShiftState};

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

/// 레이어 전환 키 라벨을 고르기 위한 스크립트 판별 — 렌더링 관심사이므로 언어별
/// 합성기(feature로 빠질 수 있다)에 의존하지 않고 코드포인트 범위로만 판단한다.
fn is_hangul_script(character: char) -> bool {
    matches!(character, '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}' | '\u{AC00}'..='\u{D7A3}')
}

/// 문자면이 한글로 짜였는가 — 문자면 복귀 라벨과 shift 고정 지원이 여기서 갈린다.
pub(crate) fn uses_hangul_letters(letters: &KeyboardLayout) -> bool {
    letters.rows.iter().flat_map(|row| &row.keys).any(
        |key| matches!(key.action, KeyAction::Character { base, .. } if is_hangul_script(base)),
    )
}

/// 프레임 한 장을 짓는 데 필요한 것 전부. 상태를 빌려만 오므로 키보드 없이도 프레임을
/// 지어 볼 수 있고, 표현 규칙이 상태를 건드릴 길이 애초에 없다.
pub(crate) struct FrameView<'keyboard> {
    /// 레이어와 필드를 모두 적용한 배열 — 히트 테스트가 보는 것과 같은 것이어야 한다
    pub layout: &'keyboard KeyboardLayout,
    pub language: &'keyboard LanguageDescriptor,
    pub preferences: &'keyboard UserPreferences,
    pub traits: FieldTraits,
    pub metrics: KeyboardMetrics,
    pub shift: Shift,
    pub hangul_letters: bool,
}

impl FrameView<'_> {
    pub(crate) fn frame(&self) -> KeyboardFrame {
        let rows = (0..self.layout.rows.len())
            .map(|row_index| {
                let bounds = row_bounds(self.layout, row_index);
                self.layout.rows[row_index]
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
                            shift_active: key.action == KeyAction::Shift && self.shift.engaged(),
                            emphasized: key.action == KeyAction::Enter
                                && self.traits.kind == FieldKind::Search,
                            role: key_role(&key.action),
                            alternates: self.key_alternates(&key.action, &key.alternates),
                        }
                    })
                    .collect()
            })
            .collect();
        KeyboardFrame {
            rows,
            metrics: self.frame_metrics(),
            panel_height_ratio: panel_height_ratio(self.layout),
        }
    }

    /// 지금 레이아웃·폼팩터에 맞는 실측 치수. 프레임을 다시 받지 않고 높이만
    /// 필요할 때(입력 뷰 높이 제약) 쓴다.
    pub(crate) fn frame_metrics(&self) -> FrameMetrics {
        let form_factor = self.metrics.form_factor;
        // 행 높이는 표준 행 대비 배수이므로, 그 합이 곧 몇 행치 높이인지가 된다
        let rows = layer_rows(self.layout);
        let scale = self.preferences.keyboard_height.scale();
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
            letter_font_size: form_factor.letter_font_size_points()
                * self.metrics.clamped_text_scale(),
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
        base * self.metrics.clamped_text_scale()
    }

    /// 이 키가 지금 내는 글자 — shift 상태가 반영된 것이다.
    pub(crate) fn key_character(&self, action: &KeyAction) -> Option<char> {
        match *action {
            KeyAction::Character { base, shifted } => Some(match self.shift.engaged() {
                true => shifted,
                false => base,
            }),
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
            KeyAction::Shift => match self.shift.state() {
                ShiftState::Locked => "⇪",
                _ => "⇧",
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

    /// 레이어 전환 키 라벨 — 순정 관례: 문자면 복귀는 스크립트에 맞게(ABC/한글),
    /// 심볼 1·2면은 123 / #+=. 비밀번호 필드에서는 순정처럼 `.?123`이 된다.
    fn layer_switch_label(&self, target: u8) -> String {
        match target {
            1 if self.traits.kind == FieldKind::Password => ".?123".to_string(),
            0 => match self.hangul_letters {
                true => "한글",
                false => "ABC",
            }
            .to_string(),
            1 => "123".to_string(),
            2 => "#+=".to_string(),
            // 통합 검색면 — 순정 이모지 키와 같은 웃는 얼굴
            _ => "☺".to_string(),
        }
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
        let cased = |character: char| match self.shift.engaged() {
            true => crate::pack::layout::uppercase(character),
            false => character,
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
}

fn key_role(action: &KeyAction) -> KeyRole {
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
