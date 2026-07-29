//! 읽기를 표기로 — 라티스 탐색. `suggest`가 랭킹을 언어와 직교하게 맡듯, 이쪽은
//! **변환**을 언어와 직교하게 맡는다.
//!
//! 여기 있는 것은 기계뿐이다: 읽기의 자리마다 표제어를 세워 격자를 만들고, 낱말 비용과
//! 이음 비용의 합이 가장 싼 길을 고른다. 무엇이 읽기가 되는지(로마자·가나 오토마톤)와
//! 그 결과를 조합 창에 어떻게 앉히는지는 언어의 일이라 `lang`에 있다.
//!
//! 병음(중국어)도 같은 격자를 쓴다 — 읽기 공간만 다르고 기계는 같다.

mod lattice;

use crate::pack::connection::ConnectionMatrix;
use crate::pack::conversion::ConversionTable;
use crate::personalization::PersonalizationStore;

pub use lattice::UNKNOWN_COST;

/// 변환이 참조하는 온디바이스 자료 묶음 — `suggest::SuggestionSources`의 짝이다.
/// 팩은 mmap 뷰라 이벤트마다 새로 만든다.
#[derive(Clone, Copy)]
pub struct Conversion<'call> {
    table: ConversionTable<'call>,
    /// 없으면 이음마다 같은 값이 든다 — 품질은 떨어지지만 변환은 선다.
    connection: Option<ConnectionMatrix<'call>>,
    /// 사람이 골라 온 표기 — 같은 읽기에서 이 표기가 앞선다. 학습이 꺼진 자리에서는 None.
    learned: Option<&'call PersonalizationStore>,
}

/// 변환 결과의 한 도막 — **문절**이다. 사람이 후보를 고르고 확정하는 단위가 형태소가
/// 아니라 이것이므로, 라티스가 낸 형태소는 여기로 묶여서 나간다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// 이 문절이 가져간 읽기 — 부분 확정과 학습이 이 값을 쓴다
    pub reading: String,
    pub surface: String,
}

impl<'call> Conversion<'call> {
    pub fn new(
        table: ConversionTable<'call>,
        connection: Option<ConnectionMatrix<'call>>,
        learned: Option<&'call PersonalizationStore>,
    ) -> Self {
        Conversion {
            table,
            connection,
            learned,
        }
    }

    /// 읽기 전체를 문절로 갈라 표기로 옮긴다. 읽기가 비면 빈 목록이다.
    pub fn convert(&self, reading: &str) -> Vec<Segment> {
        lattice::best_path(self, reading)
    }

    /// 이 읽기에 사전이 대는 표기들 — 싼 것부터, 배운 것이 앞선다. 사전에 없으면 비고,
    /// 그 자리를 무엇으로 채울지는 스크립트를 아는 쪽(합성기)이 정한다.
    pub fn candidates(&self, reading: &str) -> Vec<String> {
        let Some(entries) = self.table.lookup(reading) else {
            return Vec::new();
        };
        let mut ranked: Vec<(i64, &str)> = entries
            .iter()
            .map(|entry| {
                (
                    self.ranked_cost(reading, entry.surface, entry.cost),
                    entry.surface,
                )
            })
            .collect();
        ranked.sort_by_key(|(cost, surface)| (*cost, surface.len()));
        ranked
            .into_iter()
            .map(|(_, surface)| surface.to_string())
            .collect()
    }

    /// 아직 다 치지 않은 읽기로 미리 내놓는 낱말들 — 予測変換. (읽기, 표기) 짝이므로
    /// 고른 뒤에 무엇이 확정되는지가 함께 온다.
    pub fn predictions(&self, prefix: &str, limit: usize) -> Vec<(String, String)> {
        self.table
            .completions(prefix, limit)
            .into_iter()
            .filter_map(|(reading, entries)| {
                let surface = entries.best()?.surface.to_string();
                Some((reading, surface))
            })
            .collect()
    }

    /// 배운 표기를 앞으로 당긴 비용. 값을 빼는 쪽으로 두는 까닭은 변환이 **싼 것을
    /// 고르는** 셈이기 때문이다.
    fn ranked_cost(&self, reading: &str, surface: &str, cost: u16) -> i64 {
        let learned = self
            .learned
            .map_or(0, |store| store.conversion_weight(reading, surface));
        cost as i64 - learned as i64 * LEARNED_DISCOUNT
    }
}

/// 사람이 한 번 고른 표기가 앞서는 정도. 사전 비용의 눈금이 대략 수백~수천이므로,
/// 한 번의 선택이 흔함의 차이를 뒤집을 만큼은 되고 여러 번 쌓여야 확실해질 만큼은 크다.
const LEARNED_DISCOUNT: i64 = 400;
