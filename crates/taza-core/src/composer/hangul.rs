use super::{
    Candidate, CandidateKind, CommittedText, Composer, ComposerEnvironment, ComposerEvent,
    ComposerOutput, ComposerState, ComposingText, EditorContext,
};

const SUGGESTION_LIMIT: usize = 3;

/// 두벌식 자판 대응 ASCII 인코딩 — 한국어 lexicon은 이 형태로 팩에 저장한다.
/// trie의 바이트 단위 편집거리(OSA)가 정확히 자모 단위 편집거리가 되게 하는 장치다.
const JAMO_TO_ASCII: [(char, char); 33] = [
    ('ㅂ', 'q'), ('ㅈ', 'w'), ('ㄷ', 'e'), ('ㄱ', 'r'), ('ㅅ', 't'),
    ('ㅛ', 'y'), ('ㅕ', 'u'), ('ㅑ', 'i'), ('ㅐ', 'o'), ('ㅔ', 'p'),
    ('ㅁ', 'a'), ('ㄴ', 's'), ('ㅇ', 'd'), ('ㄹ', 'f'), ('ㅎ', 'g'),
    ('ㅗ', 'h'), ('ㅓ', 'j'), ('ㅏ', 'k'), ('ㅣ', 'l'), ('ㅋ', 'z'),
    ('ㅌ', 'x'), ('ㅊ', 'c'), ('ㅍ', 'v'), ('ㅠ', 'b'), ('ㅜ', 'n'),
    ('ㅡ', 'm'), ('ㅃ', 'Q'), ('ㅉ', 'W'), ('ㄸ', 'E'), ('ㄲ', 'R'),
    ('ㅆ', 'T'), ('ㅒ', 'O'), ('ㅖ', 'P'),
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
    'ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ',
    'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
];

const JUNGSEONG: [char; 21] = [
    'ㅏ', 'ㅐ', 'ㅑ', 'ㅒ', 'ㅓ', 'ㅔ', 'ㅕ', 'ㅖ', 'ㅗ', 'ㅘ', 'ㅙ', 'ㅚ', 'ㅛ', 'ㅜ', 'ㅝ',
    'ㅞ', 'ㅟ', 'ㅠ', 'ㅡ', 'ㅢ', 'ㅣ',
];

/// 종성 코드 1..=27에 대응 (0은 종성 없음)
const JONGSEONG: [char; 27] = [
    'ㄱ', 'ㄲ', 'ㄳ', 'ㄴ', 'ㄵ', 'ㄶ', 'ㄷ', 'ㄹ', 'ㄺ', 'ㄻ', 'ㄼ', 'ㄽ', 'ㄾ', 'ㄿ', 'ㅀ',
    'ㅁ', 'ㅂ', 'ㅄ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
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
fn decompose(character: char) -> Option<Vec<char>> {
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
struct Syllable {
    choseong: Option<char>,
    jungseong: Option<char>,
    jongseong: Vec<char>,
    jamo_count: usize,
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

    fn render(&self) -> char {
        match (self.choseong, self.jungseong) {
            (Some(choseong), Some(jungseong)) => {
                let choseong_index =
                    CHOSEONG.iter().position(|&c| c == choseong).unwrap() as u32;
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
fn recompose(jamo_sequence: &[char]) -> Vec<Syllable> {
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

fn render_all(syllables: &[Syllable]) -> String {
    syllables.iter().map(Syllable::render).collect()
}

/// 두벌식 자모 오토마타. composing 창은 최대 2음절(직전 + 현재) — 도깨비불 발생 후에도
/// Backspace로 이전 상태("가바" → "갑")로 복귀할 수 있게 marked text 안에 유지하고,
/// 세 번째 음절이 시작될 때 가장 오래된 음절을 확정한다.
#[derive(Debug, Default)]
pub struct HangulComposer {
    composing_jamo: Vec<char>,
    /// 현재 어절 전체의 자모 — composing 창(2음절)보다 길 수 있으며 제안의 기준이다
    word_jamo: Vec<char>,
    candidates: Vec<Candidate>,
}

impl HangulComposer {
    pub fn new() -> Self {
        HangulComposer::default()
    }

    /// composing이 없을 때 커서 앞을 분해해 합성을 재개한다. composing 창에는 마지막
    /// 1글자만 되가져오고(치환 1글자), 어절 자모는 커서 앞의 한글 연속 구간 전체를 채운다.
    /// 반환값은 치환을 위해 지워야 할 확정 글자 수.
    fn try_adopt(&mut self, context: &EditorContext) -> usize {
        if !self.composing_jamo.is_empty() {
            return 0;
        }
        let Some(text) = &context.text_before_cursor else {
            return 0;
        };
        let Some(last_character) = text.chars().last() else {
            return 0;
        };
        let Some(jamo) = decompose(last_character) else {
            return 0;
        };
        self.composing_jamo = jamo;
        let word_characters: Vec<char> = text
            .chars()
            .rev()
            .take_while(|&character| decompose(character).is_some())
            .collect();
        self.word_jamo = word_characters
            .into_iter()
            .rev()
            .flat_map(|character| decompose(character).unwrap())
            .collect();
        1
    }

    fn suggest(&mut self, environment: &ComposerEnvironment<'_>) {
        self.candidates.clear();
        if self.word_jamo.is_empty() || !environment.context.field.assistance_enabled() {
            return;
        }
        let Some(lexicon) = environment.pack.and_then(|pack| pack.lexicon()) else {
            return;
        };
        let Some(encoded) = encode_jamo_ascii(&self.word_jamo) else {
            return;
        };
        let personalization = &*environment.personalization;
        let mut ranked: Vec<(u32, u32, String)> = Vec::new();
        let mut push = |distance: u32, frequency: u32, encoded_word: &str| {
            let Some(jamo) = decode_jamo_ascii(encoded_word) else {
                return;
            };
            let display = compose_word(&jamo);
            let weight = frequency + personalization.weight(&display);
            ranked.push((distance, weight, display));
        };
        for completion in lexicon.complete(&encoded, SUGGESTION_LIMIT + 1) {
            push(0, completion.frequency, &completion.word);
        }
        for correction in lexicon.corrections(&encoded, 1, SUGGESTION_LIMIT + 1) {
            push(correction.distance, correction.frequency, &correction.word);
        }
        ranked.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.cmp(&a.1)).then_with(|| a.2.cmp(&b.2)));
        for (_, _, display) in ranked {
            if self.candidates.len() >= SUGGESTION_LIMIT {
                break;
            }
            if self.candidates.iter().any(|candidate| candidate.text == display) {
                continue;
            }
            self.candidates.push(Candidate {
                text: display,
                kind: CandidateKind::Prediction,
            });
        }
    }

    fn composing_output(&self, commit: Option<CommittedText>) -> ComposerOutput {
        let syllables = recompose(&self.composing_jamo);
        let text = render_all(&syllables);
        let composing = if text.is_empty() {
            None
        } else {
            let caret = text.chars().count();
            Some(ComposingText { text, caret })
        };
        ComposerOutput {
            commit,
            composing,
            candidates: self.candidates.clone(),
            ..ComposerOutput::default()
        }
    }

    fn push_jamo(&mut self, jamo: char) -> ComposerOutput {
        self.composing_jamo.push(jamo);
        self.word_jamo.push(jamo);
        let mut committed = String::new();
        loop {
            let syllables = recompose(&self.composing_jamo);
            if syllables.len() <= 2 {
                break;
            }
            let oldest_jamo_count = syllables[0].jamo_count;
            committed.push(syllables[0].render());
            self.composing_jamo.drain(..oldest_jamo_count);
        }
        let commit = (!committed.is_empty()).then(|| CommittedText::plain(committed));
        self.composing_output(commit)
    }

    fn record_word(&self, environment: &mut ComposerEnvironment<'_>) {
        if environment.context.incognito || !environment.context.field.assistance_enabled() {
            return;
        }
        let display = compose_word(&self.word_jamo);
        if !display.is_empty() {
            environment.personalization.record(&display);
        }
    }

    fn clear_word(&mut self) {
        self.word_jamo.clear();
        self.candidates.clear();
    }

    fn commit_all(&mut self, trailing: Option<char>) -> Option<CommittedText> {
        let mut text = render_all(&recompose(&self.composing_jamo));
        self.composing_jamo.clear();
        if let Some(character) = trailing {
            text.push(character);
        }
        (!text.is_empty()).then(|| CommittedText::plain(text))
    }
}

impl Composer for HangulComposer {
    fn feed(
        &mut self,
        event: ComposerEvent,
        environment: &mut ComposerEnvironment<'_>,
    ) -> ComposerOutput {
        match event {
            ComposerEvent::Key(character) if is_jamo(character) => {
                let adopted = self.try_adopt(environment.context);
                let mut output = self.push_jamo(character);
                self.suggest(environment);
                output.candidates = self.candidates.clone();
                output.delete_before_commit = adopted;
                output
            }
            ComposerEvent::Separator(' ') if self.composing_jamo.is_empty() => {
                self.clear_word();
                match super::double_space_period(environment.context) {
                    Some(output) => output,
                    None => ComposerOutput {
                        commit: Some(CommittedText::plain(" ".to_string())),
                        ..ComposerOutput::default()
                    },
                }
            }
            ComposerEvent::Key(character) | ComposerEvent::Separator(character) => {
                self.record_word(environment);
                self.clear_word();
                ComposerOutput {
                    commit: self.commit_all(Some(character)),
                    ..ComposerOutput::default()
                }
            }
            ComposerEvent::Backspace => {
                let adopted = self.try_adopt(environment.context);
                if adopted == 0 && self.composing_jamo.is_empty() {
                    self.clear_word();
                    return ComposerOutput {
                        delete_before_commit: 1,
                        ..ComposerOutput::default()
                    };
                }
                self.composing_jamo.pop();
                self.word_jamo.pop();
                self.suggest(environment);
                let mut output = self.composing_output(None);
                output.delete_before_commit = adopted;
                output
            }
            ComposerEvent::CandidateSelected(index) => {
                let Some(candidate) = self.candidates.get(index).cloned() else {
                    return ComposerOutput::default();
                };
                // 어절 중 composing 창 밖으로 이미 확정된 앞부분을 지우고, commit이
                // composing 구간을 치환한다. 선택 뒤 타이핑은 새 입력 시퀀스.
                let word_length = compose_word(&self.word_jamo).chars().count();
                let composing_length = render_all(&recompose(&self.composing_jamo))
                    .chars()
                    .count();
                if !environment.context.incognito {
                    environment.personalization.record(&candidate.text);
                }
                self.composing_jamo.clear();
                self.clear_word();
                ComposerOutput {
                    delete_before_commit: word_length - composing_length,
                    commit: Some(CommittedText::plain(format!("{} ", candidate.text))),
                    ..ComposerOutput::default()
                }
            }
        }
    }

    fn finalize(&mut self) -> Option<CommittedText> {
        self.clear_word();
        self.commit_all(None)
    }

    fn is_composing(&self) -> bool {
        !self.composing_jamo.is_empty()
    }

    fn snapshot(&self) -> ComposerState {
        ComposerState::Hangul {
            composing_jamo: self.composing_jamo.clone(),
            word_jamo: self.word_jamo.clone(),
        }
    }

    fn restore(&mut self, state: ComposerState) {
        if let ComposerState::Hangul {
            composing_jamo,
            word_jamo,
        } = state
        {
            self.composing_jamo = composing_jamo;
            self.word_jamo = word_jamo;
        }
    }
}
