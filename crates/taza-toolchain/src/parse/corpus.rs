//! 문장 코퍼스의 집계 — 낱말 빈도와 이웃한 낱말 짝.
//!
//! 원천이 문장을 어떻게 담고 있든(TSV 한 줄, XML 안의 wikitext, JSON 필드) 세는 방식은
//! 같으므로 여기 모은다. 어절을 어떻게 끊는지가 사전 품질을 좌우하는 자리다.
//!
//! 낱말은 표층 글자가 아니라 번호로 센다. 위키백과 규모의 코퍼스는 토큰이 수억 개라
//! 출현마다 낱말을 복제하면 그 복제가 파이프라인 시간의 대부분을 차지하고, 짝 표는
//! 열쇠에 낱말을 두 벌씩 더 들고 있게 된다. 번호로 세면 낱말 하나를 처음 만났을 때만
//! 복제하고, 짝은 8바이트 열쇠가 된다.

use super::Signal;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// 짝 표가 이 항목 수를 넘으면 한 번짜리를 쳐낸다. 한국어 위키백과가 짝 6천만 종류를
/// 내므로 상한에 닿지 않지만, 원천이 더 커져도 빌드가 메모리에서 넘어지지는 않게 한다.
const PAIR_BUDGET: usize = 80_000_000;

/// 버린 마크업 자리에 남기는 표지. 글자도 숫자도 아니므로 이 표지가 닿은 어절은
/// 토큰화에서 통째로 버려진다.
///
/// 줄바꿈으로 표시하면 어절에 붙어 있던 마크업이 조사만 홀로 남긴다 — `{{일본어|…}}라고`가
/// "라고"라는 낱말을 만든다. 교착어에서 이 파편은 어떤 실제 낱말보다도 흔하다. 어절
/// 사이에 있던 마크업은 앞뒤 어절을 그대로 두고 사슬만 끊는다.
pub const DISCARDED_MARK: char = char::REPLACEMENT_CHARACTER;

/// 공백으로 끊은 조각에서 낱말을 꺼낸다. 양끝의 구두점·괄호는 떼어 내지만, 안쪽에
/// 글자가 아닌 것이 남아 있으면 그 어절을 통째로 버린다.
///
/// 안쪽에서 잘라 이으면 없던 낱말이 생긴다 — "2026년에"를 숫자에서 자르면 "년에"가,
/// "Windows는"을 라틴에서 자르면 "는"이 낱말로 둔갑한다. 한국어처럼 조사가 어절에
/// 붙는 언어에서는 이 파편이 어떤 실제 낱말보다도 흔해져 상위 어휘를 통째로 차지한다.
pub(crate) fn word_of(chunk: &str) -> Option<&str> {
    // 숫자와 버림 표지는 다듬지 않는다 — 떼어 내면 "2026년에"가 "년에"로, 버려진
    // 마크업에 붙어 있던 조사가 "라고"로 남는다. 어절 안에 그런 것이 섞여 있다는
    // 것은 그 어절이 통째로 낱말이 아니라는 뜻이다. 그래서 다듬을 수 없는 글자를
    // 어절 안에서 만나면 그 자리에서 어절을 버린다.
    let mut characters = chunk.char_indices();
    let start = loop {
        let (at, character) = characters.next()?;
        if !is_trimmable(character) {
            // 다듬을 수 없는 것이 글자가 아니면 그 어절은 낱말이 아니다
            if !is_letter(character) {
                return None;
            }
            break at;
        }
    };
    let mut end = chunk.len();
    for (at, character) in characters {
        if is_letter(character) {
            // 글자가 아닌 것을 지나 다시 글자가 나왔다면 그것은 어절 안이다
            if end != chunk.len() {
                return None;
            }
            continue;
        }
        if !is_trimmable(character) {
            return None;
        }
        if end == chunk.len() {
            end = at;
        }
    }
    Some(&chunk[start..end])
}

/// 한글 음절과 ASCII 글자가 코퍼스의 거의 전부다 — 유니코드 표를 뒤지기 전에 먼저 가른다.
fn is_letter(character: char) -> bool {
    matches!(character, '가'..='힣' | 'a'..='z' | 'A'..='Z' | '\'') || character.is_alphabetic()
}

/// 어절 양끝에서 떼어 낼 수 있는 글자인가 — 구두점·괄호가 그렇고, 글자·숫자·버림 표지는
/// 아니다.
fn is_trimmable(character: char) -> bool {
    !(is_letter(character) || character.is_numeric() || character == DISCARDED_MARK)
}

/// 집계 표가 쓰는 해시.
///
/// 표준 해시는 적대적인 열쇠로 표를 무너뜨리는 공격을 막으려고 SipHash를 쓴다. 그 대비는
/// 오프라인 빌드에서 값을 하지 못하는데, 값은 톡톡히 치른다 — 표본을 떠 보면 세는 시간의
/// 5분의 1이 해시에 들어간다. 여기서 다루는 열쇠는 우리가 만든 낱말과 번호뿐이므로
/// 곱셈 한 번으로 끝나는 해시로 바꾼다.
#[derive(Default)]
struct FastHasher {
    hash: u64,
}

/// 황금비에서 온 홀수 곱수 — 낮은 자리의 변화가 높은 자리로 퍼진다.
const SCATTER: u64 = 0x517c_c1b7_2722_0a95;

impl FastHasher {
    fn absorb(&mut self, value: u64) {
        self.hash = (self.hash.rotate_left(5) ^ value).wrapping_mul(SCATTER);
    }
}

impl Hasher for FastHasher {
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            self.absorb(u64::from_le_bytes(chunk.try_into().unwrap_or_default()));
        }
        let mut tail = 0u64;
        for (at, &byte) in chunks.remainder().iter().enumerate() {
            tail |= u64::from(byte) << (at * 8);
        }
        self.absorb(tail ^ bytes.len() as u64);
    }

    fn write_u32(&mut self, value: u32) {
        self.absorb(u64::from(value));
    }

    fn write_u64(&mut self, value: u64) {
        self.absorb(value);
    }

    fn finish(&self) -> u64 {
        self.hash
    }
}

type FastMap<Key, Value> = HashMap<Key, Value, BuildHasherDefault<FastHasher>>;

/// 문장 코퍼스의 집계 — 낱말 빈도와 이웃한 낱말 짝. 원천이 문장을 어떻게 담고
/// 있든(TSV 한 줄, XML 안의 wikitext) 세는 방식은 같으므로 여기 모은다.
#[derive(Default)]
pub struct CorpusCounts {
    /// 낱말 → 번호. 낱말이 복제되는 자리는 여기 하나뿐이다.
    numbers: FastMap<Box<str>, u32>,
    /// 번호 → 관측 횟수
    counts: Vec<u64>,
    /// 짝을 세는 자리
    pairs: PairCounts,
}

/// (앞말 번호, 뒷말 번호) → 관측 횟수. 횟수를 32비트로 두는 것은 아끼려는 것이 아니라
/// 표의 한 줄을 짧게 하려는 것이다 — 항목이 수천만이라 한 줄의 네 바이트가 수백
/// 메가바이트가 된다. 짝 하나가 42억 번을 넘게 나오는 코퍼스는 없다.
type PairTable = FastMap<(u32, u32), u32>;

/// 짝을 세는 자리. 작은 원천은 그 자리에서 세고, 큰 원천은 짝을 열쇠로 갈라 다른 실들이
/// 나눠 센다 — 갈래마다 맡는 열쇠가 겹치지 않으므로 나중에 합칠 일이 없다.
///
/// 낱말에 번호를 매기는 일은 갈라 놓을 수 없다(번호가 하나의 표에서 나와야 한다). 그래서
/// 세는 실은 번호를 매기고, 짝을 표에 더하는 일만 갈래로 넘긴다.
enum PairCounts {
    Here(PairTable),
    Apart(ApartPairs),
}

impl Default for PairCounts {
    fn default() -> Self {
        PairCounts::Here(PairTable::default())
    }
}

/// 갈래 수. 짝을 표에 더하는 일이 세는 실이 하던 일의 절반쯤이므로 둘이면 족하다.
const APART_WAYS: usize = 2;

/// 한 번에 넘길 짝의 수. 짝 하나마다 실 사이를 오가면 그 값이 나눠 센 이득을 넘는다.
const APART_BATCH: usize = 8 * 1024;

/// 짝을 갈래별 실에 넘겨 세는 자리.
struct ApartPairs {
    senders: Vec<std::sync::mpsc::SyncSender<Vec<(u32, u32)>>>,
    batches: Vec<Vec<(u32, u32)>>,
    counters: Vec<std::thread::JoinHandle<PairTable>>,
}

impl ApartPairs {
    fn new(expected: usize) -> Self {
        let mut senders = Vec::with_capacity(APART_WAYS);
        let mut counters = Vec::with_capacity(APART_WAYS);
        for _ in 0..APART_WAYS {
            let (sender, receiver) = std::sync::mpsc::sync_channel::<Vec<(u32, u32)>>(4);
            senders.push(sender);
            counters.push(std::thread::spawn(move || {
                let mut table = PairTable::with_capacity_and_hasher(
                    expected / APART_WAYS,
                    BuildHasherDefault::default(),
                );
                for batch in receiver {
                    for pair in batch {
                        *table.entry(pair).or_insert(0) += 1;
                    }
                    if table.len() > PAIR_BUDGET / APART_WAYS {
                        table.retain(|_, count| *count > 1);
                    }
                }
                table
            }));
        }
        ApartPairs {
            senders,
            batches: (0..APART_WAYS)
                .map(|_| Vec::with_capacity(APART_BATCH))
                .collect(),
            counters,
        }
    }

    fn add(&mut self, pair: (u32, u32)) {
        let way = pair_way(pair);
        self.batches[way].push(pair);
        if self.batches[way].len() == APART_BATCH {
            let filled = std::mem::replace(&mut self.batches[way], Vec::with_capacity(APART_BATCH));
            // 받는 쪽이 사라지는 일은 세다 만 것뿐이라 여기서 할 수 있는 것이 없다
            let _ = self.senders[way].send(filled);
        }
    }

    /// 남은 것을 넘기고 갈래들이 센 표를 거둔다.
    fn tables(self) -> Vec<PairTable> {
        let ApartPairs {
            senders,
            batches,
            counters,
        } = self;
        for (sender, batch) in senders.iter().zip(batches) {
            if !batch.is_empty() {
                let _ = sender.send(batch);
            }
        }
        drop(senders);
        counters
            .into_iter()
            .map(|counter| counter.join().unwrap_or_default())
            .collect()
    }
}

/// 짝이 어느 갈래로 갈지 — 앞말과 뒷말을 함께 섞어 흔한 앞말이 한 갈래로 몰리지 않게 한다.
fn pair_way((left, right): (u32, u32)) -> usize {
    ((((u64::from(left) << 32) | u64::from(right)).wrapping_mul(SCATTER)) >> 32) as usize
        % APART_WAYS
}

impl PairCounts {
    fn add(&mut self, pair: (u32, u32)) {
        match self {
            PairCounts::Here(table) => *table.entry(pair).or_insert(0) += 1,
            PairCounts::Apart(apart) => apart.add(pair),
        }
    }

    /// 갈래로 나눠 셀 때는 갈래마다 제 표를 쳐낸다.
    fn prune_if_large(&mut self) {
        if let PairCounts::Here(table) = self
            && table.len() > PAIR_BUDGET
        {
            table.retain(|_, count| *count > 1);
        }
    }

    fn tables(self) -> Vec<PairTable> {
        match self {
            PairCounts::Here(table) => vec![table],
            PairCounts::Apart(apart) => apart.tables(),
        }
    }
}

impl CorpusCounts {
    pub fn new() -> Self {
        CorpusCounts::default()
    }

    /// 짝을 이만큼 담을 자리를 미리 잡아 두고, 짝 세는 일을 갈래로 나눠 맡긴 집계 표.
    ///
    /// 표는 차면 두 배로 키우면서 옛 표와 새 표를 잠깐 함께 들고 있는데, 항목이 수천만일
    /// 때는 그 잠깐이 최대 메모리를 정한다. 원천의 크기를 아는 파서가 미리 일러 주면
    /// 그 겹침도, 되풀이되는 재해싱도 없앨 수 있다.
    pub fn expecting_pairs(pairs: usize) -> Self {
        CorpusCounts {
            pairs: PairCounts::Apart(ApartPairs::new(pairs)),
            ..CorpusCounts::default()
        }
    }

    /// 대소문자가 있는 스크립트에서는 소문자로 나타난 출현만 센다 — 예문 인물 이름("Tom")이
    /// 극단적으로 흔해서 대문자 출현을 함께 세면 이름이 흔한 낱말을 밀어내고 상위권을
    /// 차지한다. 문장 첫머리 출현을 잃는 대신(흔한 낱말은 문장 중간에도 충분히 나타난다)
    /// 고유명사 편향이 사라진다. 걸러진 낱말은 짝의 사슬도 끊는다 — 건너뛰어 이으면
    /// 실제로 이웃하지 않은 낱말이 문맥으로 둔갑한다.
    pub fn read_sentence(&mut self, sentence: &str, cased: bool) {
        // 줄바꿈은 그 자체로 사슬을 끊는다 — 원천에서 버린 구간이 남긴 자리이거나
        // 문단 경계이지, 이웃한 낱말 사이가 아니다.
        for line in sentence.split('\n') {
            let mut previous: Option<u32> = None;
            for token in line.split_whitespace().map(word_of) {
                let Some(token) = token.filter(|token| {
                    !cased || !token.chars().next().is_some_and(char::is_uppercase)
                }) else {
                    previous = None;
                    continue;
                };
                previous = Some(self.count_word(token, previous));
            }
        }
    }

    /// 이미 가려낸 어절만 받아 센다. 어절을 가리는 일은 원천을 손질하는 실이 나눠 맡을 수
    /// 있지만 표를 만지는 일은 한 실이어야 하므로, 큰 원천은 둘을 갈라 놓는다. 줄바꿈은
    /// 사슬을 끊는 자리다 — 가려내며 버린 어절이 그 자리에 줄바꿈을 남긴다.
    pub fn read_words(&mut self, text: &str) {
        for line in text.split('\n') {
            let mut previous: Option<u32> = None;
            for token in line.split_whitespace() {
                previous = Some(self.count_word(token, previous));
            }
        }
    }

    /// 어절 하나를 세고 앞말과의 짝을 남긴다.
    fn count_word(&mut self, token: &str, previous: Option<u32>) -> u32 {
        let number = self.number_of(token);
        self.counts[number as usize] += 1;
        if let Some(left) = previous {
            self.pairs.add((left, number));
        }
        number
    }

    /// 낱말의 번호. 처음 만난 낱말만 복제한다.
    fn number_of(&mut self, word: &str) -> u32 {
        if let Some(&number) = self.numbers.get(word) {
            return number;
        }
        let number = self.counts.len() as u32;
        self.numbers.insert(word.into(), number);
        self.counts.push(0);
        number
    }

    /// 짝 표가 너무 커지면 한 번만 만난 짝을 쳐낸다. 위키백과 규모의 코퍼스는 짝의
    /// 종류가 수천만이라 상한이 없으면 기계에 따라 빌드가 메모리에서 넘어진다. 우리가
    /// 쓰는 것은 흔한 쪽이므로 한 번짜리를 버려도 상위 순위는 그대로다.
    ///
    /// 낱말은 쳐내지 않는다. 낱말을 표에서 지우면 같은 낱말이 다시 나타났을 때 새 번호를
    /// 받고, 그 앞에 세어 둔 짝은 사라진 번호를 가리켜 통째로 버려진다 — 흔한 낱말일수록
    /// 크게 잃는다. 낱말 표는 짝 표보다 한 자릿수 작아 굳이 줄일 것도 없다.
    pub fn prune_if_large(&mut self) {
        self.pairs.prune_if_large();
    }

    /// 센 것을 신호로 낸다. `minimum_count`보다 드문 낱말과 `pair_minimum`보다 드문 짝은
    /// 내지 않는다 — 덤프 규모의 코퍼스는 짝 종류의 넷 중 셋이 한 번짜리라, 그것까지
    /// 실어 내면 신호가 기가바이트 단위로 부푼다. 반대로 사전에서 온 짝은 한 번씩만
    /// 나타나는 것이 정상이므로, 무엇을 버릴지는 원천을 아는 파서가 정한다.
    pub fn finish(self, minimum_count: u64, pair_minimum: u64) -> Signal {
        let mut words: Vec<Option<Box<str>>> = vec![None; self.counts.len()];
        for (word, number) in self.numbers {
            words[number as usize] = Some(word);
        }
        // 신호에 실리는 낱말만 자리 번호를 받는다. 그 밖의 낱말이 낀 짝은 어차피 이
        // 원천의 빈도를 알 수 없어 뒤에서 버려지므로 여기서 접는다.
        let mut place = vec![u32::MAX; words.len()];
        let mut observed = Vec::new();
        for (number, (word, &count)) in words.into_iter().zip(&self.counts).enumerate() {
            let Some(word) = word.filter(|_| count >= minimum_count) else {
                continue;
            };
            place[number] = observed.len() as u32;
            observed.push((word.into_string(), count));
        }
        let bigrams = self
            .pairs
            .tables()
            .into_iter()
            .flatten()
            .filter(|(_, count)| u64::from(*count) >= pair_minimum)
            .filter_map(|((left, right), count)| {
                let (left, right) = (place[left as usize], place[right as usize]);
                (left != u32::MAX && right != u32::MAX).then_some((left, right, u64::from(count)))
            })
            .collect();
        Signal {
            observed,
            bigrams,
            ..Signal::default()
        }
    }

    /// 아무 낱말도 세지 못했는가 — 원천을 잘못 읽었는지 가리는 조건이다.
    pub fn is_empty(&self) -> bool {
        self.numbers.is_empty()
    }
}

#[cfg(test)]
impl CorpusCounts {
    pub(crate) fn count(&self, word: &str) -> Option<u64> {
        self.numbers
            .get(word)
            .map(|&number| self.counts[number as usize])
    }

    pub(crate) fn pair_count(&self, left: &str, right: &str) -> Option<u32> {
        let PairCounts::Here(table) = &self.pairs else {
            unreachable!("갈래로 나눠 세는 표는 다 세기 전에는 들여다볼 수 없다")
        };
        let left = *self.numbers.get(left)?;
        let right = *self.numbers.get(right)?;
        table.get(&(left, right)).copied()
    }

    pub(crate) fn pair_kinds(&self) -> usize {
        let PairCounts::Here(table) = &self.pairs else {
            unreachable!("갈래로 나눠 세는 표는 다 세기 전에는 들여다볼 수 없다")
        };
        table.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 어절 안쪽에서 자르면 조사·의존명사 파편이 낱말로 둔갑한다 — 교착어에서 이 파편은
    /// 어떤 실제 낱말보다도 흔해서 상위 어휘를 통째로 차지한다.
    #[test]
    fn words_are_whole_eojeol_not_fragments() {
        let mut corpus = CorpusCounts::new();
        corpus.read_sentence("2026년에 Windows는 나왔다.", false);
        assert_eq!(corpus.count("나왔다"), Some(1));
        // 숫자가 섞인 어절은 통째로 버린다. 스크립트가 섞인 어절은 온전히 남으므로
        // ("Windows는") 표제어 문자 집합 필터가 뒤에서 걸러 낸다 — 어느 쪽이든 조사
        // 파편은 생기지 않는다.
        for fragment in ["년에", "는", "2026년에"] {
            assert_eq!(corpus.count(fragment), None, "파편이 남음: {fragment}");
        }
        // 버린 어절은 사슬을 끊는다
        assert_eq!(corpus.pair_kinds(), 1);
        assert_eq!(corpus.pair_count("Windows는", "나왔다"), Some(1));
    }

    /// 같은 낱말이 같은 번호로 모이는가 — 번호가 어긋나면 짝이 다른 낱말을 가리킨다.
    #[test]
    fn repeated_words_share_a_number() {
        let mut corpus = CorpusCounts::new();
        corpus.read_sentence("바람 바람 소리", false);
        assert_eq!(corpus.count("바람"), Some(2));
        assert_eq!(corpus.pair_count("바람", "바람"), Some(1));
        assert_eq!(corpus.pair_count("바람", "소리"), Some(1));
        let signal = corpus.finish(1, 1);
        assert_eq!(signal.observed.len(), 2);
        assert_eq!(signal.bigrams.len(), 2);
    }
}
