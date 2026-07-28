//! 천지인 합성기 — 모음은 하늘(ㆍ)·땅(ㅡ)·사람(ㅣ) 셋을 이어 쳐서 만든다.
//!
//! 음절을 쌓는 규칙은 두벌식과 똑같으므로 `HangulComposer`에 그대로 맡기고, 여기서는
//! **타건을 표준 모음으로 옮기는 일만** 한다. 자음 멀티탭은 이 합성기에 오지 않는다 —
//! 같은 키를 이어 눌렀는지는 자리와 시각의 문제라 키보드와 Engine이 판정하고, 여기에는
//! 이미 갈아 끼운 결과가 평범한 키 입력으로 들어온다.
//!
//! 모음 표를 두벌식과 나눠 갖지 않는 까닭: 두벌식에서 ㅏ 뒤의 ㅣ는 새 글자지만
//! 천지인에서는 ㅐ다. 한 표로 둘을 겸하면 둘 중 하나가 틀린다. 배열이 자기 입력 방식을
//! 밝히는 통로(`NamedLayoutSet::method`)가 이 때문에 생겼다.

use super::hangul::HangulComposer;
use crate::contract::{
    CommittedText, Composer, ComposerEvent, ComposerOutput, ComposerState, EditorContext,
};
use crate::keyboard::{NamedLayoutSet, layouts};
use crate::lang::InputMethod;

/// 천지인 입력 방식 — 자모 오토마타 위에 하늘·땅·사람 모음 조합을 얹는다. 자음 멀티탭은
/// 배열이 데이터로 적는 것이라 방식이 따로 맡지 않는다.
pub struct CheonjiinMethod;

pub static CHEONJIIN: CheonjiinMethod = CheonjiinMethod;

impl InputMethod for CheonjiinMethod {
    fn tag(&self) -> &'static str {
        "hangul-cheonjiin"
    }

    /// 두벌식과 같은 목록을 낸다 — 천지인으로 들어온 사람이 두벌식으로 갈아탈 길이
    /// 막히면 안 되고, 한국어로 칠 수 있는 배열은 어느 방식으로 들어오든 하나다.
    fn layouts(&self) -> Vec<NamedLayoutSet> {
        layouts::hangul::layouts()
    }

    fn composer(&self) -> Box<dyn Composer> {
        Box::new(CheonjiinComposer::new())
    }
}

/// 하늘·땅·사람 — 이 셋을 이어 친 것이 모음이 된다.
const SKY: char = 'ㆍ';
const GROUND: char = 'ㅡ';
const HUMAN: char = 'ㅣ';

/// 스냅샷에서 타건과 속 합성기의 상태를 가르는 글자 — 자모에도 두벌식 상태에도 없다.
const STATE_SEPARATOR: char = '\n';

/// 타건 순서 → 모음. 쌓인 타건 **전체**를 키로 삼으므로 늘어나는 도중의 모음이 다음
/// 모음을 가리지 않는다(ㅡㆍㆍ는 ㅠ이고 ㅡㆍㆍㅣ는 ㅝ다).
///
/// 하늘 하나(ㆍ)는 표에 없다 — 아직 글자가 아니기 때문이다. 그 상태에서 확정하면
/// 아래아가 문서에 남는 대신 아무것도 남지 않는다.
const VOWELS: [(&str, char); 21] = [
    ("ㅣ", 'ㅣ'),
    ("ㅡ", 'ㅡ'),
    ("ㅣㆍ", 'ㅏ'),
    ("ㅣㆍㆍ", 'ㅑ'),
    ("ㅣㆍㅣ", 'ㅐ'),
    ("ㅣㆍㆍㅣ", 'ㅒ'),
    ("ㆍㅣ", 'ㅓ'),
    ("ㆍㆍㅣ", 'ㅕ'),
    ("ㆍㅣㅣ", 'ㅔ'),
    ("ㆍㆍㅣㅣ", 'ㅖ'),
    ("ㆍㅡ", 'ㅗ'),
    ("ㆍㆍㅡ", 'ㅛ'),
    ("ㆍㅡㅣ", 'ㅚ'),
    ("ㆍㅡㅣㆍ", 'ㅘ'),
    ("ㆍㅡㅣㆍㅣ", 'ㅙ'),
    ("ㅡㆍ", 'ㅜ'),
    ("ㅡㆍㆍ", 'ㅠ'),
    ("ㅡㆍㅣ", 'ㅟ'),
    ("ㅡㆍㆍㅣ", 'ㅝ'),
    ("ㅡㆍㆍㅣㅣ", 'ㅞ'),
    ("ㅡㅣ", 'ㅢ'),
];

fn is_component(character: char) -> bool {
    matches!(character, SKY | GROUND | HUMAN)
}

fn vowel(taps: &[char]) -> Option<char> {
    let typed: String = taps.iter().collect();
    VOWELS
        .iter()
        .find(|&&(sequence, _)| sequence == typed)
        .map(|&(_, vowel)| vowel)
}

/// 아직 글자는 아니지만 더 치면 글자가 될 수 있는 타건인가(하늘 하나, 하늘 둘).
/// 이 상태에서 손을 떼면 문서에는 아무것도 남지 않는다.
fn grows_into_vowel(taps: &[char]) -> bool {
    let typed: String = taps.iter().collect();
    VOWELS
        .iter()
        .any(|&(sequence, _)| sequence.starts_with(&typed))
}

#[derive(Debug, Default)]
pub struct CheonjiinComposer {
    inner: HangulComposer,
    /// 지금 쌓고 있는 모음의 타건들 — 다음 타건이 이 모음을 늘릴 수 있는지 본다
    taps: Vec<char>,
    /// 쌓던 모음이 이미 조합 창에 들어가 있는가. 하늘만 친 상태는 아직 글자가 아니라서
    /// 넣지 않았으므로, 다음 타건이 갈아 끼울 것도 없다.
    shown: bool,
}

impl CheonjiinComposer {
    pub fn new() -> Self {
        CheonjiinComposer::default()
    }

    /// 쌓던 모음을 새 모음으로 갈아 끼운다. 지우고 넣는 두 걸음을 속 합성기에 그대로
    /// 넘기므로 조합 창·어절 추적이 따로 볼 것이 없다.
    fn replace(&mut self, vowel: char, context: &EditorContext) -> ComposerOutput {
        let removed = self.inner.feed(ComposerEvent::Backspace, context);
        let mut output = self
            .inner
            .feed(ComposerEvent::Key(vowel), &context.unapplied());
        output.delete_before_commit += removed.delete_before_commit;
        output
    }

    /// 쌓인 타건이 이룬 모음을 조합 창에 반영한다. 아직 글자가 아니면(하늘만 친 상태)
    /// 조합 창은 그대로 두고 다음 타건을 기다린다.
    fn show(&mut self, context: &EditorContext) -> ComposerOutput {
        match (vowel(&self.taps), self.shown) {
            (Some(grown), true) => self.replace(grown, context),
            (Some(grown), false) => {
                self.shown = true;
                self.inner.feed(ComposerEvent::Key(grown), context)
            }
            (None, true) => {
                self.shown = false;
                self.inner.feed(ComposerEvent::Backspace, context)
            }
            (None, false) => self.inner.unchanged(),
        }
    }

    fn push_component(&mut self, component: char, context: &EditorContext) -> ComposerOutput {
        let mut extended = self.taps.clone();
        extended.push(component);
        // 늘어날 수 없으면 이 타건에서 새 모음이 시작한다
        self.taps = match grows_into_vowel(&extended) {
            true => extended,
            false => {
                self.shown = false;
                vec![component]
            }
        };
        self.show(context)
    }

    /// 모음 타건을 하나 무른다 — 지운 뒤에도 쌓던 모음이 남아 있으면 그것으로 돌아간다.
    fn pop_component(&mut self, context: &EditorContext) -> ComposerOutput {
        self.taps.pop();
        self.show(context)
    }
}

impl Composer for CheonjiinComposer {
    fn feed(&mut self, event: ComposerEvent, context: &EditorContext) -> ComposerOutput {
        match event {
            ComposerEvent::Key(character) if is_component(character) => {
                self.push_component(character, context)
            }
            ComposerEvent::Backspace if !self.taps.is_empty() => self.pop_component(context),
            event => {
                self.taps.clear();
                self.shown = false;
                self.inner.feed(event, context)
            }
        }
    }

    fn finalize(&mut self) -> Option<CommittedText> {
        self.taps.clear();
        self.shown = false;
        self.inner.finalize()
    }

    fn is_composing(&self) -> bool {
        self.inner.is_composing()
    }

    fn snapshot(&self) -> ComposerState {
        let taps: String = self.taps.iter().collect();
        let inner = self.inner.snapshot();
        let inner = inner.text().unwrap_or_default().to_string();
        ComposerState::from_text(&format!("{taps}{STATE_SEPARATOR}{inner}"))
    }

    fn restore(&mut self, state: ComposerState) {
        let Some((taps, inner)) = state
            .text()
            .and_then(|text| text.split_once(STATE_SEPARATOR))
        else {
            return;
        };
        self.taps = taps.chars().collect();
        // 되살린 타건이 이미 모음을 이루고 있으면 그 모음은 조합 창에도 들어가 있다
        self.shown = vowel(&self.taps).is_some();
        self.inner.restore(ComposerState::from_text(inner));
    }
}
