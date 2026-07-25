//! 레이아웃 기하 기반 오타 합성 — 터치 공간 모델이 없는 초기의 대체물.
//! 인접 키는 프레임 좌표에서 계산하므로 배열이 바뀌면 오타 분포도 따라간다.

use crate::EvaluationCase;
use std::collections::BTreeMap;
use taza_engine::keyboard::{KeyAction, KeyboardLayoutSet, row_bounds};

/// 결정론 보장용 xorshift64 — 시드가 같으면 평가 셋이 같다.
struct Random(u64);

impl Random {
    fn new(seed: u64) -> Self {
        Random(seed.max(1))
    }

    fn next(&mut self) -> u64 {
        let mut value = self.0;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.0 = value;
        value
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

// 오타 인접성은 문자 레이어(0) 기준 — 키 기하만 필요하므로 세션 없이 계산한다
fn adjacency(layout_set: &KeyboardLayoutSet) -> BTreeMap<char, Vec<char>> {
    let letters_layer = &layout_set.layers[0];
    let mut centers: Vec<(char, f32, f32)> = Vec::new();
    for row_index in 0..letters_layer.rows.len() {
        let bounds = row_bounds(letters_layer, row_index);
        for (key_index, key) in letters_layer.rows[row_index].keys.iter().enumerate() {
            if let KeyAction::Character { base, .. } = key.action {
                centers.push((
                    base,
                    bounds[key_index].x + bounds[key_index].width / 2.0,
                    bounds[key_index].y + bounds[key_index].height / 2.0,
                ));
            }
        }
    }
    let mut adjacency = BTreeMap::new();
    for &(character, x, y) in &centers {
        let mut neighbors: Vec<char> = centers
            .iter()
            .filter(|&&(other, other_x, other_y)| {
                other != character && {
                    let dx = ((other_x - x) / 0.12).powi(2);
                    let dy = ((other_y - y) / 0.27).powi(2);
                    dx + dy <= 1.0
                }
            })
            .map(|&(other, _, _)| other)
            .collect();
        neighbors.sort_unstable();
        adjacency.insert(character, neighbors);
    }
    adjacency
}

pub struct TypoSynthesizer {
    adjacency: BTreeMap<char, Vec<char>>,
    random: Random,
}

impl TypoSynthesizer {
    pub fn new(layout_set: &KeyboardLayoutSet, seed: u64) -> Self {
        TypoSynthesizer {
            adjacency: adjacency(layout_set),
            random: Random::new(seed),
        }
    }

    /// 단어 하나에서 오타 변형 하나를 만든다. 유형: 인접 키 치환·인접 전치·탈락·인접 삽입.
    /// 만들 수 없으면(너무 짧거나 인접 키 없음) None.
    pub fn synthesize(&mut self, word: &str) -> Option<String> {
        let characters: Vec<char> = word.chars().collect();
        for _ in 0..8 {
            let variant = match self.random.below(4) {
                0 => self.substitute(&characters),
                1 => transpose(&characters, &mut self.random),
                2 => delete(&characters, &mut self.random),
                _ => self.insert(&characters),
            };
            if let Some(variant) = variant
                && variant != word
            {
                return Some(variant);
            }
        }
        None
    }

    fn substitute(&mut self, characters: &[char]) -> Option<String> {
        let position = self.random.below(characters.len());
        let neighbors = self.adjacency.get(&characters[position])?;
        if neighbors.is_empty() {
            return None;
        }
        let replacement = neighbors[self.random.below(neighbors.len())];
        let mut variant: Vec<char> = characters.to_vec();
        variant[position] = replacement;
        Some(variant.into_iter().collect())
    }

    fn insert(&mut self, characters: &[char]) -> Option<String> {
        let position = self.random.below(characters.len());
        let neighbors = self.adjacency.get(&characters[position])?;
        if neighbors.is_empty() {
            return None;
        }
        let inserted = neighbors[self.random.below(neighbors.len())];
        let mut variant: Vec<char> = characters.to_vec();
        variant.insert(position + 1, inserted);
        Some(variant.into_iter().collect())
    }
}

fn transpose(characters: &[char], random: &mut Random) -> Option<String> {
    if characters.len() < 2 {
        return None;
    }
    let position = random.below(characters.len() - 1);
    if characters[position] == characters[position + 1] {
        return None;
    }
    let mut variant: Vec<char> = characters.to_vec();
    variant.swap(position, position + 1);
    Some(variant.into_iter().collect())
}

fn delete(characters: &[char], random: &mut Random) -> Option<String> {
    if characters.len() < 3 {
        return None;
    }
    let position = random.below(characters.len());
    let mut variant: Vec<char> = characters.to_vec();
    variant.remove(position);
    Some(variant.into_iter().collect())
}

/// 단어 목록에서 평가 셋을 합성한다. 단어마다 최대 per_word개.
pub fn synthesize_cases(
    layout_set: &KeyboardLayoutSet,
    words: &[&str],
    seed: u64,
    per_word: usize,
) -> Vec<EvaluationCase> {
    let mut synthesizer = TypoSynthesizer::new(layout_set, seed);
    let mut cases = Vec::new();
    for &word in words {
        for _ in 0..per_word {
            if let Some(typed) = synthesizer.synthesize(word) {
                cases.push(EvaluationCase {
                    typed,
                    intended: word.to_string(),
                });
            }
        }
    }
    cases
}
