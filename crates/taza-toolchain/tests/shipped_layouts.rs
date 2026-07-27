//! 실제로 배송되는 배열 파일이 지켜야 할 것들.
//!
//! 배열은 코드가 아니라 데이터라 컴파일러가 지켜 주지 않는다. 배열이 늘 때마다 글자
//! 배치를 손으로 다시 적는 DSL이라, 한 배열에만 변형 문자가 빠지거나(Colemak의 `ł`)
//! 행 폭이 어긋나는 드리프트가 조용히 배송된다. 여기서 막는다.

use std::collections::BTreeMap;

use taza_engine::pack::layout::{KeyAction, KeyboardLayout, NamedLayoutSet};

/// 행 폭 비교 허용 오차 — 폭은 f32 비율의 합이다.
const WIDTH_EPSILON: f32 = 1e-4;

fn shipped(file: &str) -> Vec<NamedLayoutSet> {
    let path = format!("{}/../../data/{file}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path}: {error}"));
    taza_toolchain::section::layout::parse(&text).unwrap_or_else(|error| panic!("{path}: {error}"))
}

fn letters(layout: &KeyboardLayout) -> BTreeMap<char, Vec<char>> {
    layout
        .rows
        .iter()
        .flat_map(|row| &row.keys)
        .filter_map(|key| match key.action {
            KeyAction::Character { base, .. } => Some((base, key.alternates.clone())),
            _ => None,
        })
        .collect()
}

fn row_width(row: &taza_engine::pack::layout::LayoutRow) -> f32 {
    row.keys.iter().map(|key| key.width_ratio).sum()
}

/// 레이어가 딱 하나인 옛 팩은 맨 앞 바이트가 1이라 새 형식의 표시(marker=1)와 겹친다.
/// 표시 바이트만 보고 갈랐다면 여기서 배열이 통째로 어그러진다.
#[test]
fn a_single_layer_legacy_pack_is_not_mistaken_for_the_new_format() {
    let shipped = shipped("english-layout.txt");
    let one_layer = taza_engine::pack::layout::KeyboardLayoutSet {
        layers: shipped[0].layouts.layers[..1].to_vec(),
    };
    // 배열이 하나뿐이던 시절의 형식은 이름도 표시 바이트도 없이 layer_count부터
    // 시작한다 — 지금 형식에서 그 머리말만 걷어내면 같은 바이트열이 된다.
    let named = vec![NamedLayoutSet {
        name: String::new(),
        skeleton: None,
        layouts: one_layer.clone(),
    }];
    // marker · layout_count · name_length(0) · skeleton_length(0)
    let legacy = taza_toolchain::section::layout::serialize(&named)[4..].to_vec();
    assert_eq!(legacy.first(), Some(&1), "이 시험이 겨냥한 충돌 값이 아님");

    let decoded = taza_engine::pack::layout::deserialize(&legacy).expect("옛 팩을 읽지 못함");
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].name, "");
    assert_eq!(decoded[0].layouts, one_layer);
}

/// 배열이 갈려도 심볼면·검색면은 그대로 물려받는다 — 한 벌이라도 덜 물려받으면 그
/// 배열에서만 이모지 키가 아무 데도 닿지 않는다.
#[test]
fn every_shipped_layout_carries_all_four_layers() {
    for file in ["english-layout.txt", "korean-layout.txt"] {
        for entry in shipped(file) {
            assert_eq!(
                entry.layouts.layers.len(),
                4,
                "{file}: {} 레이어 수",
                entry.name
            );
        }
    }
}

/// shift·backspace가 있는 행은 가장자리에 닿아야 한다. 폭이 모자라면 `row_bounds`가
/// 행을 가운데로 밀어 두 키가 화면 끝에서 떨어지는데, 순정에는 없는 틈이다.
#[test]
fn control_rows_span_the_full_width() {
    for file in ["english-layout.txt", "korean-layout.txt"] {
        for entry in shipped(file) {
            for (layer_index, layer) in entry.layouts.layers.iter().enumerate() {
                for (row_index, row) in layer.rows.iter().enumerate() {
                    let width = row_width(row);
                    assert!(
                        width <= 1.0 + WIDTH_EPSILON,
                        "{file}: {} 레이어 {layer_index} {row_index}행이 폭 1을 넘음: {width}",
                        entry.name
                    );
                    let anchored = row
                        .keys
                        .iter()
                        .any(|key| matches!(key.action, KeyAction::Shift | KeyAction::Backspace));
                    if anchored {
                        assert!(
                            (width - 1.0).abs() < WIDTH_EPSILON,
                            "{file}: {} 레이어 {layer_index} {row_index}행이 가장자리에 닿지 않음: {width}",
                            entry.name
                        );
                    }
                }
            }
        }
    }
}

/// 아래로 이은 키가 덮는 자리는 그 행에서 비어 있어야 한다. 진짜 키를 그대로 두면 두
/// 키가 같은 자리를 놓고 겹치고, 아래쪽 절반을 누른 손이 어느 키에 닿을지가 데이터가
/// 아니라 판정 순서에 달리게 된다.
#[test]
fn keys_spanning_rows_leave_the_covered_cell_blank() {
    for file in ["english-layout.txt", "korean-layout.txt"] {
        for entry in shipped(file) {
            for (layer_index, layer) in entry.layouts.layers.iter().enumerate() {
                for (row_index, row) in layer.rows.iter().enumerate() {
                    let mut x = 0.0f32;
                    for key in &row.keys {
                        let left = x;
                        x += key.width_ratio;
                        if key.row_span <= 1 {
                            continue;
                        }
                        for covered in row_index + 1..row_index + key.row_span as usize {
                            let mut other = 0.0f32;
                            for below in &layer.rows[covered].keys {
                                let overlaps = other < x - WIDTH_EPSILON
                                    && other + below.width_ratio > left + WIDTH_EPSILON;
                                other += below.width_ratio;
                                assert!(
                                    !overlaps || below.action == KeyAction::Blank,
                                    "{file}: {} 레이어 {layer_index} {covered}행이 이어진 키에 덮인 자리를 비워 두지 않음",
                                    entry.name
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

/// 세로로 이은 키는 팩을 오갈 때도 그대로여야 한다 — 형식 표시(marker)가 그 표기를
/// 싣는지를 가르므로, 표시와 읽는 쪽이 어긋나면 배열이 통째로 어그러진다.
#[test]
fn row_spans_survive_the_pack_round_trip() {
    let shipped = shipped("korean-layout.txt");
    let bytes = taza_toolchain::section::layout::serialize(&shipped);
    let decoded = taza_engine::pack::layout::deserialize(&bytes).expect("팩을 읽지 못함");
    assert_eq!(decoded, shipped);
    assert!(
        decoded
            .iter()
            .flat_map(|entry| &entry.layouts.layers)
            .flat_map(|layer| &layer.rows)
            .flat_map(|row| &row.keys)
            .any(|key| key.row_span > 1),
        "이 시험이 겨냥한 세로 병합이 배송 배열에 없음"
    );
}

/// 배열이 달라지는 것은 글자 배치뿐이다 — 어느 배열로 치든 같은 글자에 닿을 수 있어야
/// 하고, 그 글자를 길게 눌렀을 때 나오는 것도 같아야 한다.
#[test]
fn latin_layouts_agree_on_letters_and_alternates() {
    let shipped = shipped("english-layout.txt");
    let (first, rest) = shipped.split_first().expect("배열이 하나도 없음");
    let expected = letters(&first.layouts.layers[0]);

    let alphabet: Vec<char> = ('a'..='z').collect();
    assert_eq!(
        expected.keys().copied().collect::<Vec<_>>(),
        alphabet,
        "{}: 문자면이 a–z를 정확히 한 번씩 덮지 않음",
        first.name
    );

    for entry in rest {
        let observed = letters(&entry.layouts.layers[0]);
        for (letter, alternates) in &expected {
            let found = observed.get(letter).unwrap_or_else(|| {
                panic!(
                    "{}: 문자면에 {letter}가 없음 ({} 기준)",
                    entry.name, first.name
                )
            });
            assert_eq!(
                found, alternates,
                "{}: {letter}의 변형 문자가 {}과 다름",
                entry.name, first.name
            );
        }
        assert_eq!(
            observed.len(),
            expected.len(),
            "{}: 문자면에 {}에 없는 글자가 있음",
            entry.name,
            first.name
        );
    }
}
