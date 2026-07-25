//! trie 위의 단일 탐색 — 완성과 교정을 한 번의 순회로 함께 찾는다.
//!
//! 나눠서 찾던 때의 문제는 두 가지였다. (1) 완성 조회가 접두 아래 하위 트리를 전수
//! 순회해 접두가 짧을수록 최악 지연이 커졌고, (2) 완성과 교정이 각각 정렬돼 나오니
//! 둘을 견주는 기준이 "거리 먼저"라는 사전식 비교밖에 없었다.
//!
//! 여기서는 분기 한정 DFS 하나로 둘을 함께 낸다. 자식을 하위 트리 최고 빈도 순으로
//! 훑어 좋은 후보를 일찍 확보하고, 확보한 후보보다 나아질 수 없는 가지는 통째로
//! 건너뛴다. 방문 예산은 사전이 아무리 커도 한 번의 조회가 끝나는 것을 보장하는 안전선이다.

use crate::pack::lexicon::{Lexicon, Node};
use crate::suggest::dictionary::{Entry, Query};
use crate::suggest::score;

/// 한 번의 조회에서 방문할 노드 수의 상한. 가지치기가 듣지 않는 병적인 입력에서도
/// 조회가 끝나게 하는 안전선이며, 정상 입력에서는 걸리지 않는다.
const VISIT_BUDGET: usize = 20_000;

/// 조회 키 길이에 따른 편집 예산. 완성(정확한 접두)에는 걸리지 않고 교정에만 걸린다.
/// 키 하나짜리 입력에서 편집 하나는 입력 전체를 바꾸는 것이라 아무 낱말이나 교정 후보가
/// 되므로 예산을 주지 않는다. 반대로 긴 입력은 편집 하나로 덮이지 않는 오타가 흔하다.
pub(crate) fn edit_budget(key_length: usize) -> u32 {
    match key_length {
        0..=1 => 0,
        2..=9 => 1,
        _ => 2,
    }
}

pub(crate) fn search(lexicon: &Lexicon<'_>, query: &Query<'_>, limit: usize) -> Vec<Entry> {
    let Some(root) = lexicon.root() else {
        return Vec::new();
    };
    if limit == 0 {
        return Vec::new();
    }
    let mut search = Search {
        lexicon,
        query: query.key.as_bytes(),
        max_distance: query.max_distance,
        extending: query.extending,
        limit,
        prefix: Vec::new(),
        children: Vec::new(),
        rows: Vec::new(),
        results: Vec::new(),
        visited: 0,
    };
    let initial: Vec<u32> = (0..=query.key.len() as u32).collect();
    search.visit(root, &initial, None, true);
    let mut results = search.results;
    results.sort_by(|left, right| {
        frequency_score(right)
            .cmp(&frequency_score(left))
            .then_with(|| left.key.cmp(&right.key))
    });
    results
}

/// 사전만 보고 매긴 점수 — 개인화·언어모델 항은 호출자가 더한다.
fn frequency_score(entry: &Entry) -> i64 {
    score::combine(entry.frequency, 0, 0, entry.distance)
}

struct Search<'call, 'bytes> {
    lexicon: &'call Lexicon<'bytes>,
    query: &'call [u8],
    max_distance: u32,
    extending: bool,
    limit: usize,
    prefix: Vec<u8>,
    /// 깊이별 자식 목록·DP 행 버퍼 — 노드마다 새로 할당하지 않도록 재사용한다
    children: Vec<Vec<(u8, Node)>>,
    rows: Vec<Vec<u32>>,
    results: Vec<Entry>,
    visited: usize,
}

impl Search<'_, '_> {
    /// 지금 확보한 후보들 중 가장 낮은 점수. 아직 limit을 못 채웠으면 하한이 없다.
    fn worst_kept(&self) -> Option<i64> {
        (self.results.len() >= self.limit)
            .then(|| self.results.iter().map(frequency_score).min())
            .flatten()
    }

    fn keep(&mut self, frequency: u32, distance: u32) {
        let Ok(key) = std::str::from_utf8(&self.prefix) else {
            return;
        };
        self.results.push(Entry {
            key: key.to_string(),
            frequency,
            distance,
        });
        if self.results.len() > self.limit {
            let worst = self
                .results
                .iter()
                .enumerate()
                .min_by_key(|(_, entry)| frequency_score(entry))
                .map(|(index, _)| index);
            if let Some(index) = worst {
                self.results.swap_remove(index);
            }
        }
    }

    /// `row`는 지금 접두사에 대한 편집거리 DP 행, `previous`는 전치를 잡기 위한
    /// 한 단계 전의 행과 그때의 엣지 바이트다. `on_query_path`는 여기까지의 경로가
    /// 질의와 한 글자도 어긋나지 않았는지 — 완성인지 교정인지를 가르는 값이다.
    fn visit(&mut self, node: Node, row: &[u32], previous: Option<(&[u32], u8)>, on_query_path: bool) {
        if self.visited >= VISIT_BUDGET {
            return;
        }
        self.visited += 1;

        // 질의를 정확히 접두로 갖는 표제어는 완성이므로 거리가 0이고, 그렇지 않으면
        // 질의 전체와의 편집거리로 잰다. 어긋난 뒤에도 뒤를 공짜로 늘려 주면
        // "hepl"이 "hell"을 거쳐 "hello"에 닿아 정작 "help"를 밀어낸다.
        let completion = self.extending && on_query_path && self.prefix.len() >= self.query.len();
        let distance = if completion {
            0
        } else {
            *row.last().unwrap_or(&u32::MAX)
        };
        let frequency = self.lexicon.frequency_at(node);
        if frequency > 0 && distance <= self.max_distance {
            self.keep(frequency, distance);
        }

        // 이 아래에서 나올 수 있는 최선은 "하위 트리 최고 빈도 × 앞으로 가능한 최소 편집"이다.
        // 그것으로도 확보한 후보를 못 넘으면 하위 트리를 통째로 건너뛴다.
        let lower_bound = if self.extending && on_query_path {
            0
        } else {
            row.iter().copied().min().unwrap_or(0)
        };
        let best_possible = score::combine(
            self.lexicon.max_subtree_frequency(node),
            0,
            0,
            lower_bound,
        );
        if self.worst_kept().is_some_and(|worst| best_possible <= worst) {
            return;
        }

        let depth = self.prefix.len();
        while self.children.len() <= depth {
            self.children.push(Vec::new());
            self.rows.push(Vec::new());
        }
        let mut children = std::mem::take(&mut self.children[depth]);
        let mut next_row = std::mem::take(&mut self.rows[depth]);
        self.lexicon.children_into(node, &mut children);
        // 좋은 후보를 일찍 확보할수록 가지치기가 빨리 듣는다
        children.sort_by_key(|&(_, child)| {
            std::cmp::Reverse(self.lexicon.max_subtree_frequency(child))
        });

        for &(byte, child) in &children {
            next_row.clear();
            next_row.push(row[0] + 1);
            for column in 1..row.len() {
                let substitution = u32::from(self.query[column - 1] != byte);
                let mut cost = (next_row[column - 1] + 1)
                    .min(row[column] + 1)
                    .min(row[column - 1] + substitution);
                if let Some((row_before, previous_byte)) = previous
                    && column >= 2
                    && byte == self.query[column - 2]
                    && previous_byte == self.query[column - 1]
                {
                    cost = cost.min(row_before[column - 2] + 1);
                }
                next_row.push(cost);
            }
            // 질의 경로 위에 있는 한 완성이 나올 수 있으므로 DP 행만 보고 자르지 않는다 —
            // 행은 표제어가 길어질수록 커져서 "th"에서 "theme"에 닿지 못하게 된다.
            let child_on_path = on_query_path
                && (self.prefix.len() >= self.query.len() || byte == self.query[self.prefix.len()]);
            let reachable = self.extending && child_on_path
                || next_row.iter().copied().min().unwrap_or(u32::MAX) <= self.max_distance;
            if !reachable {
                continue;
            }
            self.prefix.push(byte);
            self.visit(child, &next_row, Some((row, byte)), child_on_path);
            self.prefix.pop();
            if self.visited >= VISIT_BUDGET {
                break;
            }
        }

        self.children[depth] = children;
        self.rows[depth] = next_row;
    }
}
