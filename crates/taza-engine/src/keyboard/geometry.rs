//! 배치 계산. 언어·상태와 무관한 순수 기하라 레이아웃만 있으면 값이 정해진다 —
//! 프레임을 만드는 쪽과 좌표를 판정하는 쪽이 같은 자리를 보도록 여기 한 곳에 둔다.

use crate::keyboard::layout::{KeyAction, KeyboardLayout, LayoutKey, LayoutRow};

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

/// 문자면 위에 얹는 숫자 행. 배열과 무관하게 같은 줄이므로(어느 언어든 숫자는 아라비아
/// 숫자다) 팩 데이터가 아니라 코어가 만든다 — 배열마다 이 줄을 다시 싣게 하지 않는다.
pub(crate) fn number_row() -> LayoutRow {
    LayoutRow {
        keys: NUMBER_ROW_KEYS
            .iter()
            .map(|(digit, alternates)| LayoutKey {
                action: KeyAction::Character {
                    base: *digit,
                    shifted: *digit,
                },
                width_ratio: NUMBER_ROW_KEY_WIDTH,
                row_span: 1,
                alternates: alternates.chars().collect(),
                label: None,
            })
            .collect(),
        height_ratio: NUMBER_ROW_HEIGHT,
    }
}

/// 레이어가 차지하는 높이 — 표준 행 몇 개분인가. 패널(통합 검색면)도 자기 높이를 갖는다.
pub(crate) fn layer_rows(layout: &KeyboardLayout) -> f32 {
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
    // 패널이 있는 레이어에서는 키 행이 패널 아래에서 시작한다
    let y: f32 = panel_height_ratio(layout) + heights[..row_index].iter().sum::<f32>();
    let total_ratio: f32 = row.keys.iter().map(|key| key.width_ratio).sum();
    // 행 폭이 1 미만이면 좌우 여백을 균등 분배해 가운데 정렬
    let mut x = (1.0 - total_ratio.min(1.0)) / 2.0;
    let mut bounds = Vec::with_capacity(row.keys.len());
    for key in &row.keys {
        // 아래로 잇는 키는 이어진 행들의 높이를 함께 갖는다
        let last = (row_index + key.row_span.max(1) as usize).min(heights.len());
        bounds.push(KeyBounds {
            x,
            y,
            width: key.width_ratio,
            height: heights[row_index..last].iter().sum(),
        });
        x += key.width_ratio;
    }
    bounds
}
