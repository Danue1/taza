//! 격자를 세우고 가장 싼 길을 고른다 — Viterbi.
//!
//! 마디는 (시작 자리, 끝 자리, 표기) 하나다. 같은 자리에서 끝나는 마디가 여럿이고 이음
//! 비용이 **앞말이 무엇이었는지**에 달려 있으므로, 자리마다 최선 하나가 아니라 그 자리에서
//! 끝나는 마디 전부를 들고 간다. 뒤에 오는 마디가 그중 자기에게 가장 싼 앞말을 고른다.
//!
//! 사전에 없는 자리도 반드시 마디를 얻는다(글자 하나짜리 미등록 마디). 그러지 않으면
//! 사전에 없는 이름 하나가 문장 전체의 변환을 없애 버린다.
//!
//! 문장의 앞뒤에도 이음이 있다. 사전은 문장 첫머리(BOS)와 끝(EOS)을 문맥 id 0으로 두고
//! 그 자리의 이음 비용까지 함께 학습하는데, 그것을 세지 않으면 **어디서 시작하고 끝나도
//! 값이 같아진다** — 첫머리에 서기 어려운 말과 그렇지 않은 말이 구분되지 않는다.

use super::{Conversion, Segment};
use crate::pack::connection::DEFAULT_CONNECTION_COST;

/// 문장의 앞뒤를 가리키는 문맥 id. 사전이 BOS/EOS에 쓰는 자리다.
const BOUNDARY_ID: u16 = 0;

/// 사전에 없는 글자 하나를 세우는 값.
///
/// **재어서 정했다.** 처음에는 "어떤 표제어보다 비싸야 한다"고 보고 6000을 두었는데, 그것이
/// 오히려 품질을 깎았다 — 사람이 가나로 두는 자리가 생각보다 많아서(「そのことからも
/// わかるように」) 미등록 마디는 사전이 억지로 한자를 끼워 넣는 것을 막는 몫도 한다.
/// mozc 평가 셋 564문장에서 문장 정확도는 1000에서 0.105, 3500에서 0.683, 6000에서 0.638,
/// 12000 이상에서 0.606으로 3500 언저리가 봉우리다. 사례가 564개뿐이라 정점을 그대로 쓰지
/// 않고 봉우리에 걸친 어림수를 골랐다.
pub const UNKNOWN_COST: i64 = 3500;

/// 격자의 마디 하나.
struct Node {
    start: usize,
    end: usize,
    surface: String,
    left_id: u16,
    right_id: u16,
    cost: i64,
    dependent: bool,
    /// 사전이 모르는 글자 하나짜리 마디인가 — 이어진 것끼리 한 문절로 묶는 근거다
    unknown: bool,
    /// 여기까지 오는 가장 싼 길의 값
    total: i64,
    /// 그 길에서의 앞 마디
    previous: Option<usize>,
}

pub(super) fn best_path(conversion: &Conversion<'_>, reading: &str) -> Vec<Segment> {
    if reading.is_empty() {
        return Vec::new();
    }
    let mut nodes: Vec<Node> = Vec::new();
    // 자리마다 그 자리에서 끝나는 마디들 — 바이트 자리이므로 길이+1칸이다
    let mut ends_at: Vec<Vec<usize>> = vec![Vec::new(); reading.len() + 1];

    for start in 0..reading.len() {
        if !reading.is_char_boundary(start) {
            continue;
        }
        // 시작 자리에 이르는 길이 없으면 이 자리에서는 마디를 세울 수 없다
        if start > 0 && ends_at[start].is_empty() {
            continue;
        }
        let mut candidates: Vec<Node> = Vec::new();
        for (end, entries) in conversion.table.prefixes(reading, start) {
            for entry in entries.iter() {
                candidates.push(Node {
                    start,
                    end,
                    surface: entry.surface.to_string(),
                    left_id: entry.left_id,
                    right_id: entry.right_id,
                    cost: conversion.ranked_cost(&reading[start..end], entry.surface, entry.cost),
                    dependent: entry.dependent,
                    unknown: false,
                    total: i64::MAX,
                    previous: None,
                });
            }
        }
        // 미등록 마디는 글자 하나 — 사전이 아는 길이 있어도 함께 세운다. 사전이 짧은
        // 말로 잘못 덮는 자리를 이 마디가 되돌릴 수 있어야 하기 때문이다.
        let next_boundary = (start + 1..=reading.len())
            .find(|&index| reading.is_char_boundary(index))
            .unwrap_or(reading.len());
        candidates.push(Node {
            start,
            end: next_boundary,
            surface: reading[start..next_boundary].to_string(),
            left_id: 0,
            right_id: 0,
            cost: UNKNOWN_COST,
            dependent: false,
            unknown: true,
            total: i64::MAX,
            previous: None,
        });

        for mut node in candidates {
            let (total, previous) = cheapest_arrival(conversion, &nodes, &ends_at[start], &node);
            node.total = total;
            node.previous = previous;
            ends_at[node.end].push(nodes.len());
            nodes.push(node);
        }
    }

    // 끝에 이른 마디 가운데 가장 싼 것에서 되짚는다 — 문장 끝으로 나가는 이음까지 셈에
    // 넣는다. 그러지 않으면 끝맺기 어려운 말로 끝나는 길이 공짜가 된다.
    let Some(&last) = ends_at[reading.len()].iter().min_by_key(|&&index| {
        nodes[index].total + connection_cost(conversion, nodes[index].right_id, BOUNDARY_ID)
    }) else {
        return Vec::new();
    };
    let mut path = vec![last];
    while let Some(previous) = nodes[path[path.len() - 1]].previous {
        path.push(previous);
    }
    path.reverse();
    into_segments(reading, &nodes, &path)
}

/// 이 마디에 이르는 가장 싼 앞말과 그때의 값. 앞말이 없으면(첫 자리) 자기 값뿐이다.
fn cheapest_arrival(
    conversion: &Conversion<'_>,
    nodes: &[Node],
    incoming: &[usize],
    node: &Node,
) -> (i64, Option<usize>) {
    // 첫 자리의 앞말은 문장 첫머리다
    if node.start == 0 {
        let opening = connection_cost(conversion, BOUNDARY_ID, node.left_id);
        return (node.cost + opening, None);
    }
    incoming
        .iter()
        .map(|&index| {
            let previous = &nodes[index];
            let connection = connection_cost(conversion, previous.right_id, node.left_id);
            (previous.total + connection + node.cost, Some(index))
        })
        .min_by_key(|(total, _)| *total)
        .unwrap_or((node.cost, None))
}

fn connection_cost(conversion: &Conversion<'_>, previous_right: u16, next_left: u16) -> i64 {
    match &conversion.connection {
        Some(matrix) => matrix.cost(previous_right, next_left) as i64,
        None => DEFAULT_CONNECTION_COST as i64,
    }
}

/// 형태소를 문절로 묶는다.
///
/// 둘을 묶는다. 앞말에 붙는 말(조사·조동사)은 앞 문절에 얹히고, **사전이 모르는 글자끼리는
/// 서로 묶인다** — 미등록 마디는 글자 하나씩 서므로 묶지 않으면 「ぱそこん」이 네 문절이
/// 되고, 사람은 「パソコン」을 고를 자리조차 얻지 못한다.
fn into_segments(reading: &str, nodes: &[Node], path: &[usize]) -> Vec<Segment> {
    let mut segments: Vec<Segment> = Vec::new();
    let mut previous_unknown = false;
    for &index in path {
        let node = &nodes[index];
        let reading_part = &reading[node.start..node.end];
        let joins = node.dependent || (node.unknown && previous_unknown);
        match segments.last_mut() {
            Some(last) if joins => {
                last.reading.push_str(reading_part);
                last.surface.push_str(&node.surface);
            }
            _ => segments.push(Segment {
                reading: reading_part.to_string(),
                surface: node.surface.clone(),
            }),
        }
        previous_unknown = node.unknown;
    }
    segments
}
