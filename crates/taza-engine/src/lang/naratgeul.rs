//! 나랏글 합성기 — 자음을 키로 고르지 않고 **만들어 간다**. 키에 서는 것은 밑글자
//! 여섯(ㄱㄴㄹㅁㅅㅇ)뿐이고, 획을 더하면 다음 차례로 간다(ㄴ→ㄷ→ㅌ). 된소리는 다른
//! 키가 맡으며(ㄱ→ㄲ), 모음도 ㅏ에 획을 더해 ㅑ가 된다.
//!
//! 음절을 쌓는 규칙은 두벌식과 똑같으므로 `HangulComposer`에 그대로 맡기고, 여기서는
//! **타건을 자모로 옮기는 일만** 한다. 획 추가·쌍자음 키는 자기 글자가 없으므로 배열이
//! 사설 영역의 표식(`STROKE`·`TENSE`)을 내고, 그 뜻은 이 합성기만 안다.
//!
//! 이어 친 단모음이 이루는 복합 모음은 베가와 나눠 갖는 표(`super::vowel`)가 안다 —
//! 단모음에 닿는 길만 다르고 그 뒤의 조합 규칙이 하나이기 때문이다.

use super::hangul::HangulComposer;
use super::vowel::vowel;
use crate::contract::{
    CommittedText, Composer, ComposerEvent, ComposerOutput, ComposerState, EditorContext,
};
use crate::keyboard::{NamedLayoutSet, layouts};
use crate::lang::InputMethod;

/// 나랏글 입력 방식 — 자모 오토마타 위에 획 추가·쌍자음을 얹는다.
pub struct NaratgeulMethod;

pub static NARATGEUL: NaratgeulMethod = NaratgeulMethod;

impl InputMethod for NaratgeulMethod {
    fn tag(&self) -> &'static str {
        "hangul-naratgeul"
    }

    /// 두벌식·천지인과 같은 목록을 낸다 — 한국어로 칠 수 있는 배열은 어느 방식으로
    /// 들어오든 하나다.
    fn layouts(&self) -> Vec<NamedLayoutSet> {
        layouts::hangul::layouts()
    }

    fn composer(&self) -> Box<dyn Composer> {
        Box::new(NaratgeulComposer::new())
    }
}

/// 획 추가 — 직전 자모에 획을 더한다(ㄴ→ㄷ→ㅌ, ㅏ→ㅑ). 고리를 한 바퀴 돌면
/// 밑글자로 되돌아온다. 이 연산을 가리키는 글자가 유니코드에 없으므로 사설 영역의 값을 쓴다.
pub const STROKE: char = '\u{E000}';
/// 쌍자음 — 직전 자음을 된소리로(ㄱ→ㄲ). 한 번 더 누르면 되돌아온다.
pub const TENSE: char = '\u{E001}';

/// 스냅샷에서 타건과 속 합성기의 상태를 가르는 글자 — 자모에도 표식에도 없다.
const STATE_SEPARATOR: char = '\n';

/// 자음 키가 내는 밑글자 여섯. 나머지 자음은 전부 여기에 획이나 된소리를 더해 만든다 —
/// 열아홉 초성이 이 여섯에서 나온다.
const BASE_CONSONANTS: [char; 6] = ['ㄱ', 'ㄴ', 'ㄹ', 'ㅁ', 'ㅅ', 'ㅇ'];

/// 획을 더해 도는 자음 고리. 획은 짝이 아니라 **차례**다(ㄴ에 획을 더하면 ㄷ이고 한 번
/// 더하면 ㅌ이다). 한 바퀴 돌면 밑글자로 되돌아오므로 잘못 더한 획을 같은 키로 물린다.
/// ㄹ은 획을 더할 자리가 없어 고리를 갖지 않는다.
const CONSONANT_STROKE: [&str; 5] = ["ㄱㅋ", "ㄴㄷㅌ", "ㅁㅂㅍ", "ㅅㅈㅊ", "ㅇㅎ"];

/// 된소리로 갈리는 자음 짝. 획을 더해 만든 자음에도 걸리므로(ㄴ→ㄷ→ㄸ) 밑글자가 아니라
/// 지금 만들어져 있는 자음을 본다.
const CONSONANT_TENSE: [(char, char); 5] = [
    ('ㄱ', 'ㄲ'),
    ('ㄷ', 'ㄸ'),
    ('ㅂ', 'ㅃ'),
    ('ㅅ', 'ㅆ'),
    ('ㅈ', 'ㅉ'),
];

/// 획을 더해 도는 모음 고리 — 둘씩이라 한 번 더 누르면 되돌아온다. ㅡ·ㅣ에는 더할 획이 없다.
const VOWEL_STROKE: [&str; 4] = ["ㅏㅑ", "ㅓㅕ", "ㅗㅛ", "ㅜㅠ"];

/// 모음 키가 내는 밑글자 여섯 — 모음 타건은 반드시 이 중 하나로 시작한다.
const BASE_VOWELS: [char; 6] = ['ㅏ', 'ㅓ', 'ㅗ', 'ㅜ', 'ㅡ', 'ㅣ'];

/// 고리에서 다음 차례. 어느 고리에도 없는 글자에는 더할 획이 없다.
fn next_in_ring(rings: &[&str], current: char) -> Option<char> {
    rings.iter().find_map(|ring| {
        let letters: Vec<char> = ring.chars().collect();
        let index = letters.iter().position(|&letter| letter == current)?;
        Some(letters[(index + 1) % letters.len()])
    })
}

/// 짝을 양방향으로 읽는다 — 같은 키를 한 번 더 누르면 되돌아온다.
fn toggled(pairs: &[(char, char)], current: char) -> Option<char> {
    pairs.iter().find_map(|&(plain, marked)| match current {
        _ if current == plain => Some(marked),
        _ if current == marked => Some(plain),
        _ => None,
    })
}

/// 나랏글 배열이 내는 타건인가 — 밑글자 열둘과 표식 둘. 그 밖의 글자는 이 방식이 볼
/// 것이 없으므로 속 합성기로 그대로 흘러간다(구두점·기호가 어절을 끊는 길이다).
fn is_press(character: char) -> bool {
    matches!(character, STROKE | TENSE)
        || BASE_CONSONANTS.contains(&character)
        || BASE_VOWELS.contains(&character)
}

/// 쌓인 타건이 가리키는 자모. 첫 타건이 밑글자이고 그 뒤는 그것을 갈아 끼우는 표식이다.
/// 어느 자모도 가리키지 못하는 타건은 None — 그런 누름은 애초에 쌓지 않는다.
fn resolve(presses: &[char]) -> Option<char> {
    let (&base, rest) = presses.split_first()?;
    match BASE_VOWELS.contains(&base) {
        true => resolve_vowel(presses),
        false => resolve_consonant(base, rest),
    }
}

fn resolve_consonant(base: char, markers: &[char]) -> Option<char> {
    let mut current = BASE_CONSONANTS.contains(&base).then_some(base)?;
    for &marker in markers {
        current = match marker {
            STROKE => next_in_ring(&CONSONANT_STROKE, current)?,
            TENSE => toggled(&CONSONANT_TENSE, current)?,
            _ => return None,
        };
    }
    Some(current)
}

/// 모음 타건은 밑글자를 잇달아 놓는 일과 마지막 글자에 획을 더하는 일 둘로 이루어진다.
/// 된소리는 자음의 것이므로 모음 타건에 섞이면 그 누름은 아무 자모도 가리키지 않는다.
fn resolve_vowel(presses: &[char]) -> Option<char> {
    let mut taps: Vec<char> = Vec::new();
    for &press in presses {
        match press {
            STROKE => {
                let last = taps.last_mut()?;
                *last = next_in_ring(&VOWEL_STROKE, *last)?;
            }
            TENSE => return None,
            tap => taps.push(tap),
        }
    }
    vowel(&taps)
}

/// 나랏글 오토마타. 지금 만들고 있는 자모 하나의 타건만 쥐고 있으면 되고, 그보다 앞은
/// 속 합성기가 갖는다 — 갈아 끼우기는 지우고 다시 넣는 두 걸음으로 넘어간다.
#[derive(Debug, Default)]
pub struct NaratgeulComposer {
    inner: HangulComposer,
    /// 지금 만들고 있는 자모의 타건들 — 첫 타건이 밑글자, 그 뒤가 표식이나 덧대는 모음
    presses: Vec<char>,
}

impl NaratgeulComposer {
    pub fn new() -> Self {
        NaratgeulComposer::default()
    }

    /// 만들던 자모를 새 자모로 갈아 끼운다. 지우고 넣는 두 걸음을 속 합성기에 그대로
    /// 넘기므로 조합 창·어절 추적이 따로 볼 것이 없다.
    fn replace(&mut self, jamo: char, context: &EditorContext) -> ComposerOutput {
        let removed = self.inner.feed(ComposerEvent::Backspace, context);
        let mut output = self
            .inner
            .feed(ComposerEvent::Key(jamo), &context.unapplied());
        output.delete_before_commit += removed.delete_before_commit;
        output
    }

    /// 타건을 하나 쌓아 본다 — 쌓은 것이 자모를 가리키면 갈아 끼우고, 아니면 이 타건에서
    /// 새 자모가 시작한다.
    fn push(&mut self, press: char, context: &EditorContext) -> ComposerOutput {
        let mut extended = self.presses.clone();
        extended.push(press);
        if !self.presses.is_empty()
            && let Some(jamo) = resolve(&extended)
        {
            self.presses = extended;
            return self.replace(jamo, context);
        }
        match resolve(&[press]) {
            Some(jamo) => {
                self.presses = vec![press];
                self.inner.feed(ComposerEvent::Key(jamo), context)
            }
            // 밑글자 없이 눌린 표식 — 더할 획이 없으므로 아무 일도 일어나지 않는다
            None => self.inner.unchanged(),
        }
    }

    /// 타건을 하나 무른다 — 획을 더해 만든 자모는 더하기 전으로 돌아간다. 표식도 누름
    /// 하나이므로 지우는 단위도 누름 하나다.
    fn pop(&mut self, context: &EditorContext) -> ComposerOutput {
        self.presses.pop();
        let jamo = resolve(&self.presses).expect("쌓인 타건의 앞부분도 자모를 가리킨다");
        self.replace(jamo, context)
    }
}

impl Composer for NaratgeulComposer {
    fn feed(&mut self, event: ComposerEvent, context: &EditorContext) -> ComposerOutput {
        match event {
            ComposerEvent::Key(press) if is_press(press) => self.push(press, context),
            ComposerEvent::Backspace if self.presses.len() > 1 => self.pop(context),
            event => {
                self.presses.clear();
                self.inner.feed(event, context)
            }
        }
    }

    fn finalize(&mut self) -> Option<CommittedText> {
        self.presses.clear();
        self.inner.finalize()
    }

    fn is_composing(&self) -> bool {
        self.inner.is_composing()
    }

    fn snapshot(&self) -> ComposerState {
        let presses: String = self.presses.iter().collect();
        let inner = self.inner.snapshot();
        let inner = inner.text().unwrap_or_default().to_string();
        ComposerState::from_text(&format!("{presses}{STATE_SEPARATOR}{inner}"))
    }

    fn restore(&mut self, state: ComposerState) {
        let Some((presses, inner)) = state
            .text()
            .and_then(|text| text.split_once(STATE_SEPARATOR))
        else {
            return;
        };
        self.presses = presses.chars().collect();
        self.inner.restore(ComposerState::from_text(inner));
    }
}
