//! 좌표 → 키 판정.
//!
//! 눌린 키를 하나로 붕괴시키면 "무엇을 노렸을까"라는 정보가 그 자리에서 사라진다.
//! 여기서는 좌표에서 눌렀을 법한 키들을 확률과 함께 내고, 교정은 그 확률을 편집 비용에
//! 반영한다 — 인접 키를 잘못 누른 오타가 무관한 낱말로의 교정보다 싸지는 근거다.
//!
//! 확률은 키 중심에서의 거리를 키 크기로 정규화한 가우시안이다. 손가락 접촉 면적·기울기
//! 같은 것을 반영하려면 실사용 로그가 필요하므로, 그 전까지의 기하 근사다.

use crate::pack::layout::{KeyAction, KeyboardLayout};

use super::geometry::{KeyBounds, KeyPosition, panel_height_ratio, row_bounds, row_heights};

/// 후보로 남길 키 수. 순정 키보드에서 한 번의 터치가 실제로 헷갈릴 만한 이웃은
/// 좌우와 위아래 정도다.
const SIGNAL_KEYS: usize = 4;

/// 이보다 확률이 낮은 이웃은 잡음이다 — 후보에 남겨 두면 교정만 헐거워진다.
const SIGNAL_FLOOR: f32 = 0.02;

/// 키 크기로 정규화한 거리의 표준편차. 값이 작을수록 판정이 단호해진다.
const SPREAD: f32 = 0.5;

/// 터치 한 번이 만든 키 신호. 확률 내림차순이며 첫 항목이 실제로 입력·표시되는 키다.
#[derive(Debug, Clone, PartialEq)]
pub struct KeySignal {
    keys: Vec<KeyProbability>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyProbability {
    pub character: char,
    pub probability: f32,
}

impl KeySignal {
    /// 물리 키보드·접근성 경로처럼 어느 키인지 확실한 입력. 확률 판정이 끼어들지 않는다.
    pub fn certain(character: char) -> Self {
        KeySignal {
            keys: vec![KeyProbability {
                character,
                probability: 1.0,
            }],
        }
    }

    /// 실제로 입력·표시되는 키.
    pub fn character(&self) -> char {
        self.keys[0].character
    }

    /// 이 좌표에서 눌렀을 법한 키들 — 확률 내림차순.
    pub fn candidates(&self) -> &[KeyProbability] {
        &self.keys
    }

    /// 이 신호가 그 글자를 눌렀을 확률. 후보에 없으면 0이다.
    pub fn probability_of(&self, character: char) -> f32 {
        self.keys
            .iter()
            .find(|candidate| candidate.character == character)
            .map(|candidate| candidate.probability)
            .unwrap_or(0.0)
    }
}

impl PartialEq<char> for KeySignal {
    fn eq(&self, other: &char) -> bool {
        self.character() == *other
    }
}

/// 좌표에 해당하는 키의 자리. 키보드 영역 밖이어도 가장 가까운 행·키로 스냅한다 —
/// 가장자리 터치를 버리는 것보다 관대한 판정이 모바일 관습이다.
pub(crate) fn key_position_at(layout: &KeyboardLayout, x: f32, y: f32) -> KeyPosition {
    let heights = row_heights(layout);
    let mut row_index = heights.len() - 1;
    // 키 행은 패널(통합 검색면) 아래에서 시작한다 — 그리는 자리(`row_bounds`)와 같은
    // 기준에서 세지 않으면 눌린 행이 패널 높이만큼 밀린다
    let mut top = panel_height_ratio(layout);
    for (index, height) in heights.iter().enumerate() {
        if y < top + height {
            row_index = index;
            break;
        }
        top += height;
    }
    // 위 행에서 아래로 이어진 키(천지인의 큰 엔터)는 이 행의 자리도 자기 것이다 —
    // 그 자리를 비워 둔 `Blank`보다 먼저 본다
    let spanning = (0..row_index).rev().find_map(|above| {
        let bounds = row_bounds(layout, above);
        let index = layout.rows[above]
            .keys
            .iter()
            .zip(&bounds)
            .position(|(key, key_bounds)| {
                above + key.row_span.max(1) as usize > row_index
                    && x >= key_bounds.x
                    && x < key_bounds.x + key_bounds.width
            })?;
        Some(KeyPosition { row: above, index })
    });
    if let Some(position) = spanning {
        return position;
    }
    let bounds = row_bounds(layout, row_index);
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

/// 좌표가 만든 문자 키 신호. 눌린 키가 문자 키일 때만 뜻이 있으며, 첫 항목은 기하
/// 판정이 고른 키와 언제나 같다 — 화면에 찍히는 글자와 판정이 어긋나면 안 된다.
pub(crate) fn key_signal_at(layout: &KeyboardLayout, x: f32, y: f32, pressed: char) -> KeySignal {
    let mut scored: Vec<(f32, char)> = Vec::new();
    for (row_index, row) in layout.rows.iter().enumerate() {
        let bounds = row_bounds(layout, row_index);
        for (key_index, key) in row.keys.iter().enumerate() {
            let Some(base) = neighbor_character(&key.action) else {
                continue;
            };
            if !confusable(pressed, base) {
                continue;
            }
            let weight = gaussian_weight(&bounds[key_index], x, y);
            if weight > 0.0 {
                scored.push((weight, base));
            }
        }
    }
    let total: f32 = scored.iter().map(|(weight, _)| weight).sum();
    if total <= 0.0 {
        return KeySignal::certain(pressed);
    }
    scored.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.1.cmp(&right.1))
    });

    let mut keys: Vec<KeyProbability> = vec![KeyProbability {
        character: pressed,
        probability: 0.0,
    }];
    for (weight, character) in scored {
        let probability = weight / total;
        if character == pressed {
            keys[0].probability = probability;
        } else if probability >= SIGNAL_FLOOR && keys.len() < SIGNAL_KEYS {
            keys.push(KeyProbability {
                character,
                probability,
            });
        }
    }
    // 기하 판정이 고른 키가 확률에서 밀리더라도 첫 자리는 그 키다 — 표시와 판정의 일치가
    // 확률보다 앞선다. 확률이 0이면 이웃 정보만 남기고 신호를 확실한 것으로 되돌린다.
    if keys[0].probability <= 0.0 {
        return KeySignal::certain(pressed);
    }
    KeySignal { keys }
}

/// 이 키를 잘못 눌렀을 때 실제로 들어갔을 글자. 멀티탭 키는 처음 누르면 주기의 첫
/// 글자가 나오므로 그것이 이웃으로서의 값이다 — 빼놓으면 문자면이 통째로 멀티탭인
/// 배열에서 이웃 정보가 하나도 남지 않는다.
fn neighbor_character(action: &KeyAction) -> Option<char> {
    match action {
        KeyAction::Character { base, .. } => Some(*base),
        KeyAction::Multitap(cycle) => cycle.first().copied(),
        _ => None,
    }
}

/// 두 키를 헷갈릴 만한가. 이웃 후보는 **조회 키가 될 수 있는 같은 갈래**여야 뜻이 있다 —
/// 사전에 숫자로 된 표제어가 없으므로, 글자를 노리다 숫자 행을 스친 것을 후보에 남기면
/// 그 자리와 확률 몫이 통째로 버려지고 진짜 이웃 글자의 몫만 깎인다.
fn confusable(pressed: char, candidate: char) -> bool {
    pressed.is_alphabetic() == candidate.is_alphabetic()
}

/// 키 중심에서의 거리를 키 크기로 정규화한 가우시안. 키가 넓으면 그만큼 너그러워진다.
fn gaussian_weight(bounds: &KeyBounds, x: f32, y: f32) -> f32 {
    let dx = (x - bounds.center_x()) / (bounds.width.max(f32::EPSILON) * SPREAD);
    let dy = (y - (bounds.y + bounds.height / 2.0)) / (bounds.height.max(f32::EPSILON) * SPREAD);
    (-0.5 * (dx * dx + dy * dy)).exp()
}
