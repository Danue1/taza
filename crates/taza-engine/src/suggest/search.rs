//! trie 위의 단일 탐색 — 완성과 교정을 한 번의 순회로 함께 찾는다.
//!
//! 나눠서 찾던 때의 문제는 두 가지였다. (1) 완성 조회가 접두 아래 하위 트리를 전수
//! 순회해 접두가 짧을수록 최악 지연이 커졌고, (2) 완성과 교정이 각각 정렬돼 나오니
//! 둘을 견주는 기준이 "거리 먼저"라는 사전식 비교밖에 없었다.
//!
//! 여기서는 분기 한정 DFS 하나로 둘을 함께 낸다. 자식을 하위 트리 최고 빈도 순으로
//! 훑어 좋은 후보를 일찍 확보하고, 확보한 후보보다 나아질 수 없는 가지는 통째로
//! 건너뛴다. 방문 예산은 사전이 아무리 커도 한 번의 조회가 끝나는 것을 보장하는 안전선이다.

use crate::keyboard::KeySignal;
use crate::pack::lexicon::{Lexicon, Node};
use crate::suggest::dictionary::{Entry, Query};
use crate::suggest::encoding::KeyEncoding;
use crate::suggest::score;

/// 한 번의 조회에서 방문할 노드 수의 상한. 가지치기가 듣지 않는 병적인 입력에서도
/// 조회가 끝나게 하는 안전선이며, 정상 입력에서는 걸리지 않는다.
const VISIT_BUDGET: usize = 20_000;

/// 조회 키 길이에 따른 편집 예산 (`EDIT_UNIT` 눈금). 완성(정확한 접두)에는 걸리지 않고
/// 교정에만 걸린다. 키 하나짜리 입력에서 편집 하나는 입력 전체를 바꾸는 것이라 아무
/// 낱말이나 교정 후보가 되므로 예산을 주지 않는다. 반대로 긴 입력은 편집 하나로 덮이지
/// 않는 오타가 흔하다.
pub(crate) fn edit_budget(key_length: usize) -> u32 {
    match key_length {
        0..=1 => 0,
        2..=9 => score::EDIT_UNIT,
        _ => 2 * score::EDIT_UNIT,
    }
}

/// 무관한 글자로의 치환 비용. 편집 1회분 그대로다.
const SUBSTITUTION: u32 = score::EDIT_UNIT;

/// 터치가 그 키를 노렸을 확률이 이보다 높으면 "그럴듯한 오타"로 본다.
const PLAUSIBLE_TOUCH: f32 = 0.05;

/// 그럴듯한 인접 키 치환의 최소 비용. 확률이 높을수록 여기에 가까워진다.
/// 편집 1회의 절반보다 커야 한다 — 그보다 싸면 예산 하나에 인접 오타가 둘씩 들어가
/// 대사전에서 후보가 범람한다. 인접 오타도 여전히 "편집 하나"이고, 싸진 만큼은
/// 도달 범위가 아니라 순위에서 이득을 본다.
const NEAR_SUBSTITUTION_FLOOR: u32 = score::EDIT_UNIT * 3 / 5;

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
        touch_at_byte: touch_at_byte(query.key, query.touches.len()),
        touch_keys: touch_keys(query.touches, query.encoding),
        max_cost: query.max_cost,
        extending: query.extending,
        limit,
        prefix: Vec::new(),
        children: Vec::new(),
        rows: Vec::new(),
        results: Vec::new(),
        visited: 0,
    };
    let initial: Vec<u32> = (0..=query.key.len() as u32)
        .map(|column| column * score::EDIT_UNIT)
        .collect();
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
    score::combine(entry.frequency, 0, 0, entry.cost)
}

/// 조회 키의 바이트 자리마다 그 자리를 친 터치가 몇 번째인지. DP는 바이트 자리로 도는데
/// 터치는 **타건 하나에 하나**라, 순 ASCII가 아닌 키에서는 둘이 어긋난다(é는 2바이트).
/// 이어지는 바이트와 터치가 모자라는 앞자리는 None이며, 그 자리에서는 이웃 확률을 쓰지
/// 않고 평범한 치환 비용으로 돌아간다.
fn touch_at_byte(key: &str, touch_count: usize) -> Vec<Option<usize>> {
    // 타건이 키 글자보다 많으면 둘을 자리로 맞출 수 없다 — 천지인처럼 타건 여럿이 모여
    // 자모 하나가 되는 방식이 그렇다(ㅏ는 ㅣ와 ㆍ 두 번이다). 억지로 맞추면 엉뚱한
    // 타건의 이웃 확률로 교정 비용을 매기므로, 그때는 이웃을 아예 셈하지 않는다.
    if touch_count > key.chars().count() {
        return vec![None; key.len()];
    }
    // 터치는 키의 끝에서부터 맞춘다 — 커서를 옮겨 이어 친 어절은 앞부분의 터치가 없다
    let offset = key.chars().count().saturating_sub(touch_count);
    let mut map = vec![None; key.len()];
    for (character_index, (byte_index, _)) in key.char_indices().enumerate() {
        map[byte_index] = character_index.checked_sub(offset);
    }
    map
}

/// 터치 신호가 낸 글자들을 조회 키 공간의 바이트로 옮긴다 — (키 바이트, 확률).
///
/// 신호에는 배열이 내는 **표시 글자**가 담기고(한글 배열은 ㄱ을 낸다) trie에는 **조회
/// 키**가 담긴다(두벌식 ASCII로 'r'이다). 여기서 한 번 옮겨 두지 않으면 두 공간이 만날
/// 일이 없어, 어느 이웃도 이웃으로 보이지 않고 모든 치환이 같은 값이 된다.
///
/// 키 공간에서 한 바이트가 되지 않는 글자는 목록에서 빠진다 — 길게 눌러 고른 변형
/// 문자가 그런 것들이라 애초에 이웃이 없다.
fn touch_keys(touches: &[KeySignal], encoding: KeyEncoding) -> Vec<Vec<(u8, f32)>> {
    touches
        .iter()
        .map(|signal| {
            signal
                .candidates()
                .iter()
                .filter_map(|candidate| {
                    Some((
                        encoding.key_byte(candidate.character)?,
                        candidate.probability,
                    ))
                })
                .collect()
        })
        .collect()
}

struct Search<'call, 'bytes> {
    lexicon: &'call Lexicon<'bytes>,
    query: &'call [u8],
    /// 바이트 자리 → 그 자리를 친 터치 번호 (`touch_at_byte`)
    touch_at_byte: Vec<Option<usize>>,
    /// 터치 번호 → 그 터치가 노렸을 법한 키 바이트들 (`touch_keys`)
    touch_keys: Vec<Vec<(u8, f32)>>,
    max_cost: u32,
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

    fn keep(&mut self, frequency: u32, cost: u32) {
        let Ok(key) = std::str::from_utf8(&self.prefix) else {
            return;
        };
        self.results.push(Entry {
            key: key.to_string(),
            frequency,
            cost,
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

    /// 질의의 `position` 자리를 `byte`로 바꾸는 비용.
    ///
    /// 그 자리의 터치가 애초에 이 글자를 노렸을 법하면(이웃 키였다면) 무관한 치환보다
    /// 싸게 친다 — 손가락이 옆 키에 닿은 것과 전혀 다른 낱말을 친 것은 같은 오타가 아니다.
    /// 터치 신호는 조회 키의 끝에서부터 맞춘다.
    fn substitution_cost(&self, position: usize, byte: u8) -> u32 {
        if self.query[position] == byte {
            return 0;
        }
        // 이웃 확률은 키 하나가 바이트 하나인 자리에서만 뜻이 있다. 여러 바이트로 적히는
        // 글자는 어느 터치도 그 바이트를 내지 못하므로(0xC3은 키가 아니다) 아래에서
        // 저절로 평범한 치환으로 떨어진다.
        let Some(keys) = self
            .touch_at_byte
            .get(position)
            .copied()
            .flatten()
            .and_then(|index| self.touch_keys.get(index))
        else {
            return SUBSTITUTION;
        };
        let probability = keys
            .iter()
            .find(|&&(candidate, _)| candidate == byte)
            .map_or(0.0, |&(_, probability)| probability);
        if probability < PLAUSIBLE_TOUCH {
            return SUBSTITUTION;
        }
        // 확률이 높을수록 바닥값에 가까워진다
        let scaled = (1.0 - probability) * (SUBSTITUTION - NEAR_SUBSTITUTION_FLOOR) as f32;
        NEAR_SUBSTITUTION_FLOOR + scaled.round() as u32
    }

    /// `row`는 지금 접두사에 대한 편집거리 DP 행, `previous`는 전치를 잡기 위한
    /// 한 단계 전의 행과 그때의 엣지 바이트다. `on_query_path`는 여기까지의 경로가
    /// 질의와 한 글자도 어긋나지 않았는지 — 완성인지 교정인지를 가르는 값이다.
    fn visit(
        &mut self,
        node: Node,
        row: &[u32],
        previous: Option<(&[u32], u8)>,
        on_query_path: bool,
    ) {
        if self.visited >= VISIT_BUDGET {
            return;
        }
        self.visited += 1;

        // 질의를 정확히 접두로 갖는 표제어는 완성이므로 거리가 0이고, 그렇지 않으면
        // 질의 전체와의 편집거리로 잰다. 어긋난 뒤에도 뒤를 공짜로 늘려 주면
        // "hepl"이 "hell"을 거쳐 "hello"에 닿아 정작 "help"를 밀어낸다.
        let completion = self.extending && on_query_path && self.prefix.len() >= self.query.len();
        let cost = if completion {
            0
        } else {
            *row.last().unwrap_or(&u32::MAX)
        };
        let frequency = self.lexicon.frequency_at(node);
        if frequency > 0 && cost <= self.max_cost {
            self.keep(frequency, cost);
        }

        // 이 아래에서 나올 수 있는 최선은 "하위 트리 최고 빈도 × 앞으로 가능한 최소 편집"이다.
        // 그것으로도 확보한 후보를 못 넘으면 하위 트리를 통째로 건너뛴다.
        let lower_bound = if self.extending && on_query_path {
            0
        } else {
            row.iter().copied().min().unwrap_or(0)
        };
        let best_possible =
            score::combine(self.lexicon.max_subtree_frequency(node), 0, 0, lower_bound);
        if self
            .worst_kept()
            .is_some_and(|worst| best_possible <= worst)
        {
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
            next_row.push(row[0] + score::EDIT_UNIT);
            for column in 1..row.len() {
                let substitution = self.substitution_cost(column - 1, byte);
                let mut cost = (next_row[column - 1] + score::EDIT_UNIT)
                    .min(row[column] + score::EDIT_UNIT)
                    .min(row[column - 1] + substitution);
                if let Some((row_before, previous_byte)) = previous
                    && column >= 2
                    && byte == self.query[column - 2]
                    && previous_byte == self.query[column - 1]
                {
                    cost = cost.min(row_before[column - 2] + score::EDIT_UNIT);
                }
                next_row.push(cost);
            }
            // 질의 경로 위에 있는 한 완성이 나올 수 있으므로 DP 행만 보고 자르지 않는다 —
            // 행은 표제어가 길어질수록 커져서 "th"에서 "theme"에 닿지 못하게 된다.
            let child_on_path = on_query_path
                && (self.prefix.len() >= self.query.len() || byte == self.query[self.prefix.len()]);
            let reachable = self.extending && child_on_path
                || next_row.iter().copied().min().unwrap_or(u32::MAX) <= self.max_cost;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 타건과 조회 키 글자를 자리로 맞추는 규칙. 끝에서부터 맞추되, 타건이 더 많으면
    /// 맞출 방법이 없으므로 이웃을 아예 셈하지 않는다.
    #[test]
    fn touches_align_from_the_end_and_give_up_when_they_outnumber_the_key() {
        // 갓 친 어절 — 자리마다 타건이 하나씩 있다
        assert_eq!(touch_at_byte("rk", 2), vec![Some(0), Some(1)]);
        // 커서를 옮겨 이어 친 어절 — 앞부분에는 타건이 없다
        assert_eq!(touch_at_byte("rk", 1), vec![None, Some(0)]);
        // 천지인처럼 타건 여럿이 모여 자모 하나가 되는 방식 — 자리를 맞출 수 없다
        assert_eq!(touch_at_byte("rk", 4), vec![None, None]);
    }

    /// 이웃 확률은 조회 키 공간에서 견줘야 뜻이 있다 — 배열이 내는 것은 ㄱ이고
    /// trie가 담은 것은 'r'이다.
    #[test]
    fn touch_keys_move_signals_into_the_key_space() {
        let signal = KeySignal::certain('ㄱ');
        let keys = touch_keys(
            std::slice::from_ref(&signal),
            KeyEncoding::HangulJamoDubeolsik,
        );
        assert_eq!(keys, vec![vec![(b'r', 1.0)]]);

        // 라틴은 접힌 공간에서 견준다 — 문장 첫 글자가 매번 대문자로 들어오기 때문이다
        let signal = KeySignal::certain('T');
        let keys = touch_keys(std::slice::from_ref(&signal), KeyEncoding::Utf8);
        assert_eq!(keys, vec![vec![(b't', 1.0)]]);
    }
}
