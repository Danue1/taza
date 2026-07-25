//! 레이아웃 기하 기반 오타 합성.
//!
//! 오타는 글자만이 아니라 **터치 좌표**로 낸다. 인접 키를 잘못 누른 오타는 두 키 경계
//! 가까이를 누른 것이지 이웃 키 한복판을 정확히 누른 것이 아니며, 그 차이가 곧 코어의
//! 공간 모델이 읽는 신호다. 좌표까지 합성해야 평가 셋과 런타임이 같은 가정 위에 선다.
//!
//! 다루는 것은 문자 레이어(0)의 기본 글자뿐이다 — shift로만 닿는 글자(ㅃ·ㅆ 등)와
//! 배열에 없는 글자(어퍼스트로피)는 좌표를 만들 수 없으므로 그 낱말은 평가에서 빠진다.
//! 없는 키를 아무 자리나 눌러 친 것으로 꾸미면 평가가 엉뚱한 입력을 재게 된다.

use crate::EvaluationCase;
use std::collections::BTreeMap;
use taza_engine::keyboard::{KeyAction, KeyBounds, KeyboardLayoutSet, row_bounds};

/// 인접 키 오타의 터치를 원래 노리던 키 쪽으로 얼마나 당길지. 키 반폭보다 작아야
/// 터치가 실제로 잘못 눌린 키 안에 남는다.
const NEAR_MISS_BIAS: f32 = 0.3;

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

/// 실제로 친 글자열과 그때의 터치 좌표(정규화). 길이는 서로 같다.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedSequence {
    pub text: String,
    pub touches: Vec<(f32, f32)>,
}

fn key_centers(layout_set: &KeyboardLayoutSet) -> BTreeMap<char, (f32, f32)> {
    // 오타 인접성은 문자 레이어(0) 기준 — 키 기하만 필요하므로 세션 없이 계산한다
    let letters_layer = &layout_set.layers[0];
    let mut centers = BTreeMap::new();
    for row_index in 0..letters_layer.rows.len() {
        let bounds = row_bounds(letters_layer, row_index);
        for (key_index, key) in letters_layer.rows[row_index].keys.iter().enumerate() {
            if let KeyAction::Character { base, .. } = key.action {
                centers.insert(base, center_of(&bounds[key_index]));
            }
        }
    }
    centers
}

fn center_of(bounds: &KeyBounds) -> (f32, f32) {
    (
        bounds.x + bounds.width / 2.0,
        bounds.y + bounds.height / 2.0,
    )
}

fn adjacency(centers: &BTreeMap<char, (f32, f32)>) -> BTreeMap<char, Vec<char>> {
    let mut adjacency = BTreeMap::new();
    for (&character, &(x, y)) in centers {
        let mut neighbors: Vec<char> = centers
            .iter()
            .filter(|(other, (other_x, other_y))| {
                **other != character && {
                    let dx = ((other_x - x) / 0.12).powi(2);
                    let dy = ((other_y - y) / 0.27).powi(2);
                    dx + dy <= 1.0
                }
            })
            .map(|(other, _)| *other)
            .collect();
        neighbors.sort_unstable();
        adjacency.insert(character, neighbors);
    }
    adjacency
}

pub struct TypoSynthesizer {
    centers: BTreeMap<char, (f32, f32)>,
    adjacency: BTreeMap<char, Vec<char>>,
    random: Random,
}

impl TypoSynthesizer {
    pub fn new(layout_set: &KeyboardLayoutSet, seed: u64) -> Self {
        let centers = key_centers(layout_set);
        TypoSynthesizer {
            adjacency: adjacency(&centers),
            centers,
            random: Random::new(seed),
        }
    }

    /// 글자열을 키 중심을 정확히 눌러 친 것으로 본 좌표열 — 오타가 아닌 입력이다.
    /// 이 배열로 칠 수 없는 글자가 섞이면 None — 없는 키를 아무 자리나 눌러 친 것으로
    /// 꾸미면 평가가 엉뚱한 입력을 재는 셈이 된다.
    pub fn touches_for(&self, text: &str) -> Option<Vec<(f32, f32)>> {
        text.chars()
            .map(|character| self.centers.get(&character).copied())
            .collect()
    }

    /// 이 배열로 칠 수 있는 낱말인가.
    pub fn can_type(&self, text: &str) -> bool {
        text.chars()
            .all(|character| self.centers.contains_key(&character))
    }

    /// 단어 하나에서 오타 변형 하나를 만든다. 유형: 인접 키 치환·인접 전치·탈락·인접 삽입.
    /// 만들 수 없으면(너무 짧거나 인접 키 없음) None.
    pub fn synthesize(&mut self, word: &str) -> Option<TypedSequence> {
        let characters: Vec<char> = word.chars().collect();
        for _ in 0..8 {
            let variant = match self.random.below(4) {
                0 => self.substitute(&characters),
                1 => self.transpose(&characters),
                2 => self.delete(&characters),
                _ => self.insert(&characters),
            };
            if let Some(variant) = variant
                && variant.text != word
            {
                return Some(variant);
            }
        }
        None
    }

    fn sequence(&self, characters: &[char]) -> Option<TypedSequence> {
        let text: String = characters.iter().collect();
        let touches = self.touches_for(&text)?;
        Some(TypedSequence { text, touches })
    }

    /// 잘못 눌린 키 안에서, 노리던 키 쪽으로 치우친 지점 — 실제 인접 키 오타의 모습이다.
    fn near_miss(&self, pressed: char, intended: char) -> Option<(f32, f32)> {
        let (pressed_x, pressed_y) = *self.centers.get(&pressed)?;
        let (intended_x, intended_y) = *self.centers.get(&intended)?;
        Some((
            pressed_x + (intended_x - pressed_x) * NEAR_MISS_BIAS,
            pressed_y + (intended_y - pressed_y) * NEAR_MISS_BIAS,
        ))
    }

    fn substitute(&mut self, characters: &[char]) -> Option<TypedSequence> {
        let position = self.random.below(characters.len());
        let intended = characters[position];
        let neighbors = self.adjacency.get(&intended)?;
        if neighbors.is_empty() {
            return None;
        }
        let replacement = neighbors[self.random.below(neighbors.len())];
        let mut variant: Vec<char> = characters.to_vec();
        variant[position] = replacement;
        let mut sequence = self.sequence(&variant)?;
        if let Some(touch) = self.near_miss(replacement, intended) {
            sequence.touches[position] = touch;
        }
        Some(sequence)
    }

    fn insert(&mut self, characters: &[char]) -> Option<TypedSequence> {
        let position = self.random.below(characters.len());
        let neighbors = self.adjacency.get(&characters[position])?;
        if neighbors.is_empty() {
            return None;
        }
        let inserted = neighbors[self.random.below(neighbors.len())];
        let mut variant: Vec<char> = characters.to_vec();
        variant.insert(position + 1, inserted);
        self.sequence(&variant)
    }

    fn transpose(&mut self, characters: &[char]) -> Option<TypedSequence> {
        if characters.len() < 2 {
            return None;
        }
        let position = self.random.below(characters.len() - 1);
        if characters[position] == characters[position + 1] {
            return None;
        }
        let mut variant: Vec<char> = characters.to_vec();
        variant.swap(position, position + 1);
        self.sequence(&variant)
    }

    fn delete(&mut self, characters: &[char]) -> Option<TypedSequence> {
        if characters.len() < 3 {
            return None;
        }
        let position = self.random.below(characters.len());
        let mut variant: Vec<char> = characters.to_vec();
        variant.remove(position);
        self.sequence(&variant)
    }
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
        if !synthesizer.can_type(word) {
            continue;
        }
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
