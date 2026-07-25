//! 한글 자모 코덱 — 음절↔자모 분해·조합과 두벌식 ASCII 인코딩. 합성기(hangul)와
//! 팩 컴파일러(taza-toolchain)가 같은 규칙을 써야 하므로 별도 모듈로 둔다.

/// 두벌식 자판 대응 ASCII 인코딩 — 한국어 lexicon은 이 형태로 팩에 저장한다.
/// trie의 바이트 단위 편집거리(OSA)가 정확히 자모 단위 편집거리가 되게 하는 장치다.
const JAMO_TO_ASCII: [(char, char); 33] = [
    ('ㅂ', 'q'),
    ('ㅈ', 'w'),
    ('ㄷ', 'e'),
    ('ㄱ', 'r'),
    ('ㅅ', 't'),
    ('ㅛ', 'y'),
    ('ㅕ', 'u'),
    ('ㅑ', 'i'),
    ('ㅐ', 'o'),
    ('ㅔ', 'p'),
    ('ㅁ', 'a'),
    ('ㄴ', 's'),
    ('ㅇ', 'd'),
    ('ㄹ', 'f'),
    ('ㅎ', 'g'),
    ('ㅗ', 'h'),
    ('ㅓ', 'j'),
    ('ㅏ', 'k'),
    ('ㅣ', 'l'),
    ('ㅋ', 'z'),
    ('ㅌ', 'x'),
    ('ㅊ', 'c'),
    ('ㅍ', 'v'),
    ('ㅠ', 'b'),
    ('ㅜ', 'n'),
    ('ㅡ', 'm'),
    ('ㅃ', 'Q'),
    ('ㅉ', 'W'),
    ('ㄸ', 'E'),
    ('ㄲ', 'R'),
    ('ㅆ', 'T'),
    ('ㅒ', 'O'),
    ('ㅖ', 'P'),
];

/// 단어 전체를 입력 자모 시퀀스로 분해한다 (복합 모음·겹받침은 타이핑 단위로 분리).
/// 한글이 아닌 문자가 섞이면 None.
pub fn decompose_word(word: &str) -> Option<Vec<char>> {
    let mut jamo_sequence = Vec::new();
    for character in word.chars() {
        jamo_sequence.extend(decompose(character)?);
    }
    Some(jamo_sequence)
}

/// 자모 시퀀스를 표시 문자열(완성형 음절)로 재구성한다.
pub fn compose_word(jamo_sequence: &[char]) -> String {
    render_all(&recompose(jamo_sequence))
}

pub fn encode_jamo_ascii(jamo_sequence: &[char]) -> Option<String> {
    jamo_sequence
        .iter()
        .map(|&jamo| {
            JAMO_TO_ASCII
                .iter()
                .find(|&&(candidate, _)| candidate == jamo)
                .map(|&(_, ascii)| ascii)
        })
        .collect()
}

pub fn decode_jamo_ascii(encoded: &str) -> Option<Vec<char>> {
    encoded
        .chars()
        .map(|ascii| {
            JAMO_TO_ASCII
                .iter()
                .find(|&&(_, candidate)| candidate == ascii)
                .map(|&(jamo, _)| jamo)
        })
        .collect()
}

const CHOSEONG: [char; 19] = [
    'ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ', 'ㅋ',
    'ㅌ', 'ㅍ', 'ㅎ',
];

const JUNGSEONG: [char; 21] = [
    'ㅏ', 'ㅐ', 'ㅑ', 'ㅒ', 'ㅓ', 'ㅔ', 'ㅕ', 'ㅖ', 'ㅗ', 'ㅘ', 'ㅙ', 'ㅚ', 'ㅛ', 'ㅜ', 'ㅝ', 'ㅞ',
    'ㅟ', 'ㅠ', 'ㅡ', 'ㅢ', 'ㅣ',
];

/// 종성 코드 1..=27에 대응 (0은 종성 없음)
const JONGSEONG: [char; 27] = [
    'ㄱ', 'ㄲ', 'ㄳ', 'ㄴ', 'ㄵ', 'ㄶ', 'ㄷ', 'ㄹ', 'ㄺ', 'ㄻ', 'ㄼ', 'ㄽ', 'ㄾ', 'ㄿ', 'ㅀ', 'ㅁ',
    'ㅂ', 'ㅄ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
];

fn combine_jungseong(first: char, second: char) -> Option<char> {
    match (first, second) {
        ('ㅗ', 'ㅏ') => Some('ㅘ'),
        ('ㅗ', 'ㅐ') => Some('ㅙ'),
        ('ㅗ', 'ㅣ') => Some('ㅚ'),
        ('ㅜ', 'ㅓ') => Some('ㅝ'),
        ('ㅜ', 'ㅔ') => Some('ㅞ'),
        ('ㅜ', 'ㅣ') => Some('ㅟ'),
        ('ㅡ', 'ㅣ') => Some('ㅢ'),
        _ => None,
    }
}

fn split_jungseong(combined: char) -> Vec<char> {
    match combined {
        'ㅘ' => vec!['ㅗ', 'ㅏ'],
        'ㅙ' => vec!['ㅗ', 'ㅐ'],
        'ㅚ' => vec!['ㅗ', 'ㅣ'],
        'ㅝ' => vec!['ㅜ', 'ㅓ'],
        'ㅞ' => vec!['ㅜ', 'ㅔ'],
        'ㅟ' => vec!['ㅜ', 'ㅣ'],
        'ㅢ' => vec!['ㅡ', 'ㅣ'],
        other => vec![other],
    }
}

fn split_jongseong(combined: char) -> Vec<char> {
    match combined {
        'ㄳ' => vec!['ㄱ', 'ㅅ'],
        'ㄵ' => vec!['ㄴ', 'ㅈ'],
        'ㄶ' => vec!['ㄴ', 'ㅎ'],
        'ㄺ' => vec!['ㄹ', 'ㄱ'],
        'ㄻ' => vec!['ㄹ', 'ㅁ'],
        'ㄼ' => vec!['ㄹ', 'ㅂ'],
        'ㄽ' => vec!['ㄹ', 'ㅅ'],
        'ㄾ' => vec!['ㄹ', 'ㅌ'],
        'ㄿ' => vec!['ㄹ', 'ㅍ'],
        'ㅀ' => vec!['ㄹ', 'ㅎ'],
        'ㅄ' => vec!['ㅂ', 'ㅅ'],
        other => vec![other],
    }
}

/// 완성형 음절·호환 자모를 입력 자모 시퀀스로 되돌린다. 분해 불가면 None.
pub(crate) fn decompose(character: char) -> Option<Vec<char>> {
    let code = character as u32;
    if (0xAC00..=0xD7A3).contains(&code) {
        let offset = code - 0xAC00;
        let choseong = CHOSEONG[(offset / 588) as usize];
        let jungseong = JUNGSEONG[((offset / 28) % 21) as usize];
        let jongseong_index = offset % 28;
        let mut jamo = vec![choseong];
        jamo.extend(split_jungseong(jungseong));
        if jongseong_index > 0 {
            jamo.extend(split_jongseong(JONGSEONG[(jongseong_index - 1) as usize]));
        }
        return Some(jamo);
    }
    if is_consonant(character) {
        return Some(vec![character]);
    }
    if is_vowel(character) {
        return Some(split_jungseong(character));
    }
    // 겹받침·복합 모음이 단독으로 놓인 경우 (ㄳ, ㅘ 등)
    let split = split_jongseong(character);
    if split.len() == 2 && split.iter().all(|&c| is_consonant(c)) {
        return Some(split);
    }
    None
}

fn combine_jongseong(first: char, second: char) -> Option<char> {
    match (first, second) {
        ('ㄱ', 'ㅅ') => Some('ㄳ'),
        ('ㄴ', 'ㅈ') => Some('ㄵ'),
        ('ㄴ', 'ㅎ') => Some('ㄶ'),
        ('ㄹ', 'ㄱ') => Some('ㄺ'),
        ('ㄹ', 'ㅁ') => Some('ㄻ'),
        ('ㄹ', 'ㅂ') => Some('ㄼ'),
        ('ㄹ', 'ㅅ') => Some('ㄽ'),
        ('ㄹ', 'ㅌ') => Some('ㄾ'),
        ('ㄹ', 'ㅍ') => Some('ㄿ'),
        ('ㄹ', 'ㅎ') => Some('ㅀ'),
        ('ㅂ', 'ㅅ') => Some('ㅄ'),
        _ => None,
    }
}

fn is_consonant(jamo: char) -> bool {
    CHOSEONG.contains(&jamo)
}

fn is_vowel(jamo: char) -> bool {
    JUNGSEONG.contains(&jamo)
}

pub fn is_jamo(character: char) -> bool {
    is_consonant(character) || is_vowel(character)
}

fn is_single_jongseong(jamo: char) -> bool {
    !matches!(jamo, 'ㄸ' | 'ㅃ' | 'ㅉ') && is_consonant(jamo)
}

#[derive(Debug)]
pub(crate) struct Syllable {
    choseong: Option<char>,
    jungseong: Option<char>,
    jongseong: Vec<char>,
    /// 이 음절을 이루는 입력 자모 수 — 확정 시 버퍼에서 잘라 낼 길이
    pub(crate) jamo_count: usize,
}

impl Syllable {
    fn from_consonant(jamo: char) -> Self {
        Syllable {
            choseong: Some(jamo),
            jungseong: None,
            jongseong: Vec::new(),
            jamo_count: 1,
        }
    }

    fn from_vowel(jamo: char) -> Self {
        Syllable {
            choseong: None,
            jungseong: Some(jamo),
            jongseong: Vec::new(),
            jamo_count: 1,
        }
    }

    fn jongseong_character(&self) -> Option<char> {
        match self.jongseong.as_slice() {
            [] => None,
            [single] => Some(*single),
            [first, second] => combine_jongseong(*first, *second),
            _ => unreachable!("종성은 최대 2자모"),
        }
    }

    pub(crate) fn render(&self) -> char {
        match (self.choseong, self.jungseong) {
            (Some(choseong), Some(jungseong)) => {
                let choseong_index = CHOSEONG.iter().position(|&c| c == choseong).unwrap() as u32;
                let jungseong_index =
                    JUNGSEONG.iter().position(|&c| c == jungseong).unwrap() as u32;
                let jongseong_index = self
                    .jongseong_character()
                    .map(|jongseong| {
                        JONGSEONG.iter().position(|&c| c == jongseong).unwrap() as u32 + 1
                    })
                    .unwrap_or(0);
                char::from_u32(
                    0xAC00 + (choseong_index * 21 + jungseong_index) * 28 + jongseong_index,
                )
                .unwrap()
            }
            (Some(choseong), None) => choseong,
            (None, Some(jungseong)) => jungseong,
            (None, None) => unreachable!("빈 음절은 생성되지 않음"),
        }
    }
}

/// 자모 시퀀스에서 음절열을 재구성한다. Backspace(자모 pop)와 도깨비불(다음 모음이
/// 앞 음절의 마지막 종성을 가져가는 현상)이 같은 규칙에서 자연히 도출되도록,
/// 상태 전이 대신 매 이벤트마다 전체 재구성한다.
pub(crate) fn recompose(jamo_sequence: &[char]) -> Vec<Syllable> {
    let mut syllables: Vec<Syllable> = Vec::new();
    for &jamo in jamo_sequence {
        if is_vowel(jamo) {
            match syllables.last_mut() {
                Some(last) if !last.jongseong.is_empty() => {
                    let stolen = last.jongseong.pop().unwrap();
                    last.jamo_count -= 1;
                    let mut next = Syllable::from_consonant(stolen);
                    next.jungseong = Some(jamo);
                    next.jamo_count = 2;
                    syllables.push(next);
                }
                Some(last) if last.choseong.is_some() && last.jungseong.is_none() => {
                    last.jungseong = Some(jamo);
                    last.jamo_count += 1;
                }
                Some(last)
                    if last.jungseong.is_some()
                        && combine_jungseong(last.jungseong.unwrap(), jamo).is_some() =>
                {
                    last.jungseong = combine_jungseong(last.jungseong.unwrap(), jamo);
                    last.jamo_count += 1;
                }
                _ => syllables.push(Syllable::from_vowel(jamo)),
            }
        } else {
            match syllables.last_mut() {
                Some(last)
                    if last.choseong.is_some()
                        && last.jungseong.is_some()
                        && last.jongseong.is_empty()
                        && is_single_jongseong(jamo) =>
                {
                    last.jongseong.push(jamo);
                    last.jamo_count += 1;
                }
                Some(last)
                    if last.jongseong.len() == 1
                        && combine_jongseong(last.jongseong[0], jamo).is_some() =>
                {
                    last.jongseong.push(jamo);
                    last.jamo_count += 1;
                }
                _ => syllables.push(Syllable::from_consonant(jamo)),
            }
        }
    }
    syllables
}

pub(crate) fn render_all(syllables: &[Syllable]) -> String {
    syllables.iter().map(Syllable::render).collect()
}
