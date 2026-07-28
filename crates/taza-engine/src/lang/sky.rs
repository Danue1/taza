//! 베가(SKY-II) 합성기 — 자음도 모음도 키에 그대로 서 있고, 같은 키를 이어 누르면 다음
//! 글자로 간다. 그 갈아 끼우기는 배열이 데이터로 적는 멀티탭이라 여기까지 오지 않는다
//! (자음은 이미 갈아 끼운 결과가 평범한 키 입력으로 들어온다). 이 합성기가 하는 일은
//! **이어 친 단모음을 복합 모음으로 옮기는 것**뿐이다 — ㅜ 다음 ㅣ는 ㅟ다.
//!
//! 음절을 쌓는 규칙은 두벌식과 똑같으므로 `HangulComposer`에 그대로 맡긴다. 그런데도
//! 두벌식과 방식을 나눠 갖지 못하는 까닭은 천지인·나랏글과 같다: 두벌식에서 ㅏ 뒤의 ㅣ는
//! 새 글자지만 베가에서는 ㅐ다.

use super::hangul::HangulComposer;
use super::vowel::vowel;
use crate::contract::{
    CommittedText, Composer, ComposerEvent, ComposerOutput, ComposerState, EditorContext,
};
use crate::keyboard::{NamedLayoutSet, layouts};
use crate::lang::InputMethod;

/// 베가 입력 방식 — 자모 오토마타 위에 이어 친 모음의 조합을 얹는다.
pub struct SkyMethod;

pub static SKY: SkyMethod = SkyMethod;

impl InputMethod for SkyMethod {
    fn tag(&self) -> &'static str {
        "hangul-sky"
    }

    /// 두벌식·천지인·나랏글과 같은 목록을 낸다 — 한국어로 칠 수 있는 배열은 어느 방식으로
    /// 들어오든 하나다.
    fn layouts(&self) -> Vec<NamedLayoutSet> {
        layouts::hangul::layouts()
    }

    fn composer(&self) -> Box<dyn Composer> {
        Box::new(SkyComposer::new())
    }
}

/// 스냅샷에서 타건과 속 합성기의 상태를 가르는 글자 — 자모에는 없다.
const STATE_SEPARATOR: char = '\n';

/// 모음 키가 내는 단모음 열. 그 밖의 글자(자음·구두점)는 이 방식이 볼 것이 없으므로 속
/// 합성기로 그대로 흘러간다.
const VOWEL_TAPS: [char; 10] = ['ㅏ', 'ㅑ', 'ㅓ', 'ㅕ', 'ㅗ', 'ㅛ', 'ㅜ', 'ㅠ', 'ㅡ', 'ㅣ'];

fn is_vowel_tap(character: char) -> bool {
    VOWEL_TAPS.contains(&character)
}

/// 베가 오토마타. 지금 쌓고 있는 모음의 타건만 쥐고 있으면 되고, 그보다 앞은 속 합성기가
/// 갖는다 — 갈아 끼우기는 지우고 다시 넣는 두 걸음으로 넘어간다.
#[derive(Debug, Default)]
pub struct SkyComposer {
    inner: HangulComposer,
    /// 지금 쌓고 있는 모음의 타건들 — 다음 타건이 이 모음을 늘릴 수 있는지 본다
    taps: Vec<char>,
}

impl SkyComposer {
    pub fn new() -> Self {
        SkyComposer::default()
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

    /// 타건을 하나 쌓아 본다 — 쌓은 것이 모음을 이루면 갈아 끼우고, 아니면 이 타건에서
    /// 새 모음이 시작한다.
    fn push(&mut self, tap: char, context: &EditorContext) -> ComposerOutput {
        let mut extended = self.taps.clone();
        extended.push(tap);
        if !self.taps.is_empty()
            && let Some(grown) = vowel(&extended)
        {
            self.taps = extended;
            return self.replace(grown, context);
        }
        self.taps = vec![tap];
        self.inner.feed(ComposerEvent::Key(tap), context)
    }

    /// 모음 타건을 하나 무른다 — 지운 뒤에도 쌓던 모음이 남아 있으면 그것으로 돌아간다.
    fn pop(&mut self, context: &EditorContext) -> ComposerOutput {
        self.taps.pop();
        let stayed = vowel(&self.taps).expect("쌓인 타건의 앞부분도 모음을 이룬다");
        self.replace(stayed, context)
    }
}

impl Composer for SkyComposer {
    fn feed(&mut self, event: ComposerEvent, context: &EditorContext) -> ComposerOutput {
        match event {
            ComposerEvent::Key(tap) if is_vowel_tap(tap) => self.push(tap, context),
            ComposerEvent::Backspace if self.taps.len() > 1 => self.pop(context),
            event => {
                self.taps.clear();
                self.inner.feed(event, context)
            }
        }
    }

    fn finalize(&mut self) -> Option<CommittedText> {
        self.taps.clear();
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
        self.inner.restore(ComposerState::from_text(inner));
    }
}
