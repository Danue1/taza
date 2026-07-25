//! 한국어 표기 지식 — 조사·파생 접미사·활용 축약.
//!
//! 추출기와 정규화가 저마다 갖고 있던 것을 여기로 모았다. 조사 목록을 손보는 일이
//! 코드 세 군데를 고치는 일이어서는 안 된다.

use std::collections::HashSet;
use taza_engine::lang::jamo::decompose_word;

/// 체언에 붙는 조사 — 앞말의 종성 유무로 이형태가 갈리는 것은 (종성 있음, 없음) 짝으로
/// 적는다. 교착어의 어절은 사전에 통째로 담으면 폭발하므로, 흔한 체언에만 이 목록을
/// 붙여 예산 안에서 실제로 타이핑되는 어절을 덮는다.
pub const PARTICLES: [(&str, &str); 24] = [
    ("은", "는"),
    ("이라고", "라고"),
    ("이라는", "라는"),
    ("이", "가"),
    ("을", "를"),
    ("과", "와"),
    ("으로", "로"),
    ("이나", "나"),
    ("이라", "라"),
    ("이야", "야"),
    ("이다", "다"),
    ("이에요", "예요"),
    ("입니다", "입니다"),
    ("도", "도"),
    ("만", "만"),
    ("에", "에"),
    ("에서", "에서"),
    ("에게", "에게"),
    ("한테", "한테"),
    ("까지", "까지"),
    ("부터", "부터"),
    ("처럼", "처럼"),
    ("보다", "보다"),
    ("의", "의"),
];

/// 체언에 붙어 용언을 만드는 접미사. 사전은 이것을 명사와 따로 싣기 때문에
/// "생각하", "걱정되" 같은 어간이 어디에도 없다 — 여기서 되살린다.
pub const DERIVATIONAL_SUFFIXES: [&str; 2] = ["하", "되"];

/// 이형태를 펼친 조사 목록 — 코어는 결합형을 알아보기만 하면 되므로 앞말의 종성으로
/// 짝을 가릴 필요가 없다.
pub fn particle_forms() -> Vec<String> {
    let mut forms: Vec<String> = PARTICLES
        .iter()
        .flat_map(|&(after_consonant, after_vowel)| [after_consonant, after_vowel])
        .map(str::to_string)
        .collect();
    forms.sort_unstable();
    forms.dedup();
    forms
}

/// 앞말이 종성으로 끝나는가 — 조사 이형태를 고르는 조건. 한글 음절이 아니면 답이 없다.
pub fn has_final_consonant(word: &str) -> Option<bool> {
    let last = word.chars().next_back()?;
    if !('가'..='힣').contains(&last) {
        return None;
    }
    Some(!(last as u32 - '가' as u32).is_multiple_of(28))
}

/// 활용 관계를 재는 단위. 한글은 자모로 풀고, 풀 수 없는 글자가 섞이면 표층 그대로 둔다.
///
/// 표층 음절로 재면 한국어 활용의 대부분을 놓친다 — 어미는 어간의 마지막 음절 **안으로**
/// 들어가 그 음절을 바꾸기 때문이다("하"→"했", "오"→"왔", "주"→"줘"). 자모로 풀면 이
/// 변화가 어미 자모의 덧붙음으로 드러나(ㅈㅜ → ㅈㅜㅓ) 접두 관계가 그대로 성립한다.
fn inflection_key(word: &str) -> Vec<char> {
    decompose_word(word).unwrap_or_else(|| word.chars().collect())
}

/// 어간 말모음이 어미와 축약돼 아예 다른 모음으로 나타나는 짝. 두벌식에서 두 키인
/// 축약(ㅗ+ㅏ→ㅘ, ㅜ+ㅓ→ㅝ)은 자모로 풀면 덧붙음으로 보여 이미 접두로 잡히므로,
/// 한 키짜리 모음으로 바뀌어 접두 관계가 끊기는 것만 적는다.
const VOWEL_CONTRACTIONS: [(char, &[char]); 3] = [
    ('ㅏ', &['ㅐ']),       // 하 + 여 → 해
    ('ㅣ', &['ㅕ']),       // 마시 + 어 → 마셔
    ('ㅡ', &['ㅓ', 'ㅏ']), // 쓰 + 어 → 써, 바쁘 + 아 → 바빠
];

/// 활용형을 알아보는 어간 색인. 축약형을 따로 두는 이유는 어절이 될 조건이 다르기
/// 때문이다 — 표층 어간은 어미가 붙어야 어절이 되지만("하"는 어절이 아니다),
/// 축약형은 이미 어미가 녹아든 형태라 그 자체로 어절이다("해", "미안해", "써").
#[derive(Default)]
pub struct InflectionStems {
    bare: HashSet<Vec<char>>,
    contracted: HashSet<Vec<char>>,
}

impl InflectionStems {
    pub fn insert(&mut self, stem: &str) {
        let key = inflection_key(stem);
        let Some(&last) = key.last() else {
            return;
        };
        for (vowel, contractions) in VOWEL_CONTRACTIONS {
            if vowel != last {
                continue;
            }
            for &replacement in contractions {
                let mut variant = key.clone();
                variant.pop();
                variant.push(replacement);
                self.contracted.insert(variant);
            }
        }
        self.bare.insert(key);
    }

    /// 알려진 어간에서 뻗어 나온 낱말인가 — 활용형으로 볼 수 있는 최소 조건이다.
    /// 한국어 용언 어간은 대개 한 음절(있·하·같)이라 길이로 더 조이면 정작 흔한
    /// 활용형이 다 걸러진다. 어간 집합이 용언 파일에서만 오므로(고유명사는 애초에
    /// 빠져 있다) 이 조건만으로도 잡음이 새는 길은 좁다.
    pub fn grew(&self, word: &str) -> bool {
        let key = inflection_key(word);
        (1..key.len()).any(|length| self.bare.contains(&key[..length]))
            || (1..=key.len()).any(|length| self.contracted.contains(&key[..length]))
    }
}
