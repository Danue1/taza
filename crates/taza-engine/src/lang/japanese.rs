//! 일본어 입력 방식 — 읽기를 쌓고, 변환하고, 문절로 고른다.
//!
//! 이 방식만 **사전을 보고서야 조합 창을 그릴 수 있다**. 자모 오토마타는 친 것이 곧 글자라
//! 사전이 없어도 조합 창이 서지만, 「きしゃ」가 화면에 무엇으로 보일지는 사전만 안다.
//! 그래서 합성기에 변환 창구(`ComposerEnvironment::conversion`)가 들어온다.
//!
//! 상태는 셋이다. 읽기를 쌓는 중(변환 전) · 변환해 놓고 고르는 중 · 아무것도 없음.
//! 순정과 같은 규칙으로 오간다: 스페이스가 변환을 걸고 다시 누르면 다음 후보로 가며,
//! 엔터가 확정하고 백스페이스가 변환을 무른다.
//!
//! 로마자와 12키 가나가 **같은 합성기를 나눠 쓴다** — 다른 것은 타건이 읽기가 되는 길뿐이고
//! (한쪽은 로마자 오토마톤, 다른 쪽은 친 가나 그대로) 변환부터는 완전히 같기 때문이다.

use crate::contract::{
    CommittedText, Composer, ComposerEnvironment, ComposerEvent, ComposerOutput, ComposerState,
    ComposingDisplay, ComposingText, SuggestionRequest,
};
use crate::convert::Segment;
use crate::keyboard::{NamedLayoutSet, layouts};
use crate::lang::InputMethod;
use crate::lang::kana::{self, next_form};
use crate::lang::romaji::Romaji;

/// 문절 하나에 내놓는 표기 후보 수의 상한. 후보 바는 가로로 훑는 자리라 순정도 이 언저리에서
/// 접고 나머지는 확장 목록으로 미룬다.
const CANDIDATE_LIMIT: usize = 9;

/// 아직 변환하지 않은 읽기로 미리 내놓는 낱말 수.
const PREDICTION_LIMIT: usize = 6;

/// 미변환 상태의 스페이스가 내는 글자 — 일본어 자판은 전각을 낸다.
const IDEOGRAPHIC_SPACE: char = '\u{3000}';

/// 12키 가나 배열의 「小゛゜」 키가 내는 표식. 유니코드에 이 연산을 가리키는 글자가 없으므로
/// 사설 영역을 쓴다 — 나랏글의 획 추가와 같은 자리다.
pub const FORM_MARKER: char = '\u{E010}';

/// 로마자 입력 — 데스크톱에서 치던 손이 그대로 통한다.
pub struct RomajiMethod;

pub static ROMAJI: RomajiMethod = RomajiMethod;

impl InputMethod for RomajiMethod {
    fn tag(&self) -> &'static str {
        "japanese-romaji"
    }

    fn layouts(&self) -> Vec<NamedLayoutSet> {
        layouts::japanese::layouts()
    }

    fn composer(&self) -> Box<dyn Composer> {
        Box::new(JapaneseComposer::romaji())
    }

    fn space_inserts_text(&self) -> bool {
        false
    }

    fn auto_capitalizes(&self) -> bool {
        false
    }

    fn composing_display(&self) -> ComposingDisplay {
        ComposingDisplay::Marked
    }
}

/// 12키 가나 — 같은 키를 이어 눌러 행을 돌고(あ→い→う), 「小゛゜」가 직전 글자를 갈아 끼운다.
pub struct KanaMethod;

pub static KANA: KanaMethod = KanaMethod;

impl InputMethod for KanaMethod {
    fn tag(&self) -> &'static str {
        "japanese-kana"
    }

    /// 로마자와 같은 목록을 낸다 — 일본어로 칠 수 있는 배열은 어느 방식으로 들어오든 하나다.
    fn layouts(&self) -> Vec<NamedLayoutSet> {
        layouts::japanese::layouts()
    }

    fn composer(&self) -> Box<dyn Composer> {
        Box::new(JapaneseComposer::kana())
    }

    fn space_inserts_text(&self) -> bool {
        false
    }

    fn auto_capitalizes(&self) -> bool {
        false
    }

    fn composing_display(&self) -> ComposingDisplay {
        ComposingDisplay::Marked
    }
}

/// 변환해 놓고 고르는 중의 상태.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Converted {
    segments: Vec<Segment>,
    /// 지금 사람이 보고 있는 문절
    focus: usize,
    /// 그 문절에서 몇 번째 표기를 보고 있는가 — 스페이스를 누를 때마다 늘어난다
    choice: usize,
}

impl Converted {
    fn surface(&self) -> String {
        self.segments
            .iter()
            .map(|segment| segment.surface.as_str())
            .collect()
    }

    fn reading(&self) -> String {
        self.segments
            .iter()
            .map(|segment| segment.reading.as_str())
            .collect()
    }

    /// 주목 문절이 조합 창에서 차지하는 코드포인트 구간.
    fn focus_range(&self) -> (usize, usize) {
        let start: usize = self.segments[..self.focus]
            .iter()
            .map(|segment| segment.surface.chars().count())
            .sum();
        let length = self.segments[self.focus].surface.chars().count();
        (start, start + length)
    }
}

/// 타건을 읽기로 옮기는 길 — 방식이 다른 것은 여기까지다.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Input {
    /// 로마자를 가나로 옮기는 오토마톤
    Romaji(Romaji),
    /// 친 가나가 곧 읽기 — 12키가 배열에서 이미 가나를 낸다
    Kana,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JapaneseComposer {
    input: Input,
    /// 아직 확정되지 않은 읽기 — 히라가나 정규형이다
    reading: String,
    converted: Option<Converted>,
}

impl JapaneseComposer {
    pub fn romaji() -> Self {
        JapaneseComposer {
            input: Input::Romaji(Romaji::new()),
            reading: String::new(),
            converted: None,
        }
    }

    pub fn kana() -> Self {
        JapaneseComposer {
            input: Input::Kana,
            reading: String::new(),
            converted: None,
        }
    }

    /// 조합 창에 보이는 글자 — 변환 전에는 읽기(와 아직 가나가 못 된 로마자), 변환 중에는
    /// 표기다.
    fn composing_text(&self) -> Option<ComposingText> {
        if let Some(converted) = &self.converted {
            let text = converted.surface();
            let caret = text.chars().count();
            return Some(ComposingText {
                text,
                caret,
                focus: Some(converted.focus_range()),
            });
        }
        let pending = match &self.input {
            Input::Romaji(romaji) => romaji.pending(),
            Input::Kana => "",
        };
        let text = format!("{}{}", self.reading, pending);
        (!text.is_empty()).then(|| ComposingText::whole(text))
    }

    /// 변환 전에는 아직 다 치지 않은 읽기로 낱말을 미리 내놓고, 변환 중에는 주목 문절의
    /// 표기들을 내놓는다. 후보의 조회 키는 **그것을 고르면 확정될 읽기**다.
    fn suggest(&self, environment: &ComposerEnvironment<'_>) -> SuggestionRequest {
        let Some(conversion) = environment.conversion() else {
            return SuggestionRequest::None;
        };
        match &self.converted {
            Some(converted) => {
                let reading = converted.segments[converted.focus].reading.clone();
                let candidates = self
                    .segment_candidates(environment, &reading)
                    .into_iter()
                    .map(|surface| (reading.clone(), surface))
                    .collect();
                SuggestionRequest::Ready { candidates }
            }
            None if self.reading.is_empty() => SuggestionRequest::None,
            None => {
                // 지금 읽기 그대로가 늘 첫 자리다 — 사전이 무엇을 대든 친 대로 두는 길이
                // 열려 있어야 한다
                let mut candidates = vec![(self.reading.clone(), self.reading.clone())];
                // 그 다음은 **지금까지 친 것을 변환한 결과**다. 이것이 없으면 「がっこうが」에
                // 学校側·学校帰り만 서고 정작 사람이 원한 学校が가 어디에도 없다 — 그것은
                // 낱말과 조사라 사전에 표제어로 실릴 수 없고, 표제어를 찾는 예측만으로는
                // 닿을 길이 없기 때문이다.
                let converted: String = conversion
                    .convert(&self.reading)
                    .iter()
                    .map(|segment| segment.surface.as_str())
                    .collect();
                if !converted.is_empty() {
                    candidates.push((self.reading.clone(), converted));
                }
                // 아직 다 치지 않았을 수 있으므로 더 긴 낱말도 함께 내놓는다
                candidates.extend(conversion.predictions(&self.reading, PREDICTION_LIMIT));
                // 같은 표기가 여러 길로 올라오면 앞선 자리만 남긴다
                let mut seen = Vec::new();
                candidates.retain(|(_, text)| match seen.contains(text) {
                    true => false,
                    false => {
                        seen.push(text.clone());
                        true
                    }
                });
                SuggestionRequest::Ready { candidates }
            }
        }
    }

    /// 문절 하나의 표기 후보 — 사전이 대는 것 뒤에 가나 그대로와 가타카나를 붙인다.
    /// 사전에 없는 외래어를 가타카나로 적는 일은 늘 있으므로 이 둘은 언제나 자리를 갖는다.
    fn segment_candidates(
        &self,
        environment: &ComposerEnvironment<'_>,
        reading: &str,
    ) -> Vec<String> {
        let mut candidates = environment
            .conversion()
            .map(|conversion| conversion.candidates(reading))
            .unwrap_or_default();
        for fallback in [reading.to_string(), kana::to_katakana(reading)] {
            if !fallback.is_empty() && !candidates.contains(&fallback) {
                candidates.push(fallback);
            }
        }
        candidates.truncate(CANDIDATE_LIMIT);
        candidates
    }

    /// 읽기를 격자에 올린다. 사전이 없으면 읽기 그대로가 한 문절이 된다 — 팩을 아직 받지
    /// 못한 사람에게도 가나 입력은 성립해야 한다.
    fn convert(&mut self, environment: &ComposerEnvironment<'_>) {
        let segments = environment
            .conversion()
            .map(|conversion| conversion.convert(&self.reading))
            .unwrap_or_default();
        let segments = match segments.is_empty() {
            true => vec![Segment {
                reading: self.reading.clone(),
                surface: self.reading.clone(),
            }],
            false => segments,
        };
        self.converted = Some(Converted {
            segments,
            focus: 0,
            choice: 0,
        });
    }

    /// 주목 문절을 다음 표기로 갈아 끼운다 — 스페이스를 거듭 누를 때의 일이다.
    fn cycle_focus(&mut self, environment: &ComposerEnvironment<'_>) {
        let Some(converted) = &self.converted else {
            return;
        };
        let reading = converted.segments[converted.focus].reading.clone();
        let candidates = self.segment_candidates(environment, &reading);
        let Some(converted) = &mut self.converted else {
            return;
        };
        if candidates.is_empty() {
            return;
        }
        converted.choice = (converted.choice + 1) % candidates.len();
        converted.segments[converted.focus].surface = candidates[converted.choice].clone();
    }

    /// 조합을 통째로 확정한다. 문절마다 고른 표기를 배우므로 다음 번 같은 읽기에서 그것이
    /// 앞선다.
    fn commit_all(&mut self) -> Option<CommittedText> {
        let (surface, reading) = match self.converted.take() {
            Some(converted) => (converted.surface(), converted.reading()),
            None => {
                let pending = match &mut self.input {
                    Input::Romaji(romaji) => romaji.flush(),
                    Input::Kana => String::new(),
                };
                (format!("{}{}", self.reading, pending), self.reading.clone())
            }
        };
        self.reading.clear();
        if let Input::Romaji(romaji) = &mut self.input {
            romaji.clear();
        }
        (!surface.is_empty()).then(|| CommittedText {
            surface,
            reading: (!reading.is_empty()).then_some(reading),
        })
    }

    /// 타건 하나가 읽기에 보태는 것.
    fn push_key(&mut self, character: char) {
        match &mut self.input {
            Input::Romaji(romaji) => {
                let output = romaji.push(character);
                self.reading.push_str(&output.kana);
            }
            // 12키는 배열이 이미 가나를 낸다 — 정규화만 거친다
            Input::Kana => match kana::normalize(&character.to_string()) {
                Some(normalized) => self.reading.push_str(&normalized),
                None => self.reading.push(character),
            },
        }
    }

    /// 「小゛゜」 — 직전 가나를 작은 글자·탁점·반탁점으로 갈아 끼운다.
    fn cycle_form(&mut self) {
        let Some(last) = self.reading.chars().next_back() else {
            return;
        };
        let Some(next) = next_form(last) else {
            return;
        };
        self.reading.pop();
        self.reading.push(next);
    }

    fn output(&self, environment: &ComposerEnvironment<'_>) -> ComposerOutput {
        ComposerOutput {
            composing: self.composing_text(),
            suggest: self.suggest(environment),
            ..ComposerOutput::default()
        }
    }
}

impl Composer for JapaneseComposer {
    fn feed(
        &mut self,
        event: ComposerEvent,
        environment: &ComposerEnvironment<'_>,
    ) -> ComposerOutput {
        match event {
            // 스페이스가 변환을 건다. 변환 중에 다시 누르면 다음 표기로 넘어간다 —
            // 후보 바를 손으로 짚지 않고도 고를 수 있는 길이다.
            ComposerEvent::Separator(' ') if !self.reading.is_empty() => {
                match self.converted.is_some() {
                    true => self.cycle_focus(environment),
                    false => self.convert(environment),
                }
                self.output(environment)
            }
            // 엔터는 확정만 한다 — 조합이 있는 동안 줄을 바꾸지 않는 것이 순정이다.
            ComposerEvent::Separator('\n') if self.is_composing() => ComposerOutput {
                commit: self.commit_all(),
                ..ComposerOutput::default()
            },
            // 변환 중의 「小゛゜」는 갈아 끼울 가나가 없다 — 보고 있던 것을 확정만 한다
            ComposerEvent::Key(FORM_MARKER) if self.converted.is_some() => ComposerOutput {
                commit: self.commit_all(),
                ..ComposerOutput::default()
            },
            ComposerEvent::Key(FORM_MARKER) => {
                self.cycle_form();
                self.output(environment)
            }
            ComposerEvent::Key(character) => {
                // 변환 중에 글자를 치면 보고 있던 것이 확정되고 새 읽기가 시작한다
                let commit = self
                    .converted
                    .is_some()
                    .then(|| self.commit_all())
                    .flatten();
                self.push_key(character);
                let mut output = self.output(environment);
                output.commit = commit;
                output
            }
            ComposerEvent::Separator(character) => {
                let commit = self.commit_all();
                let separator = match character {
                    ' ' => IDEOGRAPHIC_SPACE,
                    other => other,
                };
                let surface = match commit {
                    Some(commit) => format!("{}{separator}", commit.surface),
                    None => separator.to_string(),
                };
                ComposerOutput {
                    commit: Some(CommittedText::plain(surface)),
                    ..ComposerOutput::default()
                }
            }
            // 변환을 무르면 읽기로 돌아간다 — 사람이 고르다 마음을 바꾸는 자리다
            ComposerEvent::Backspace if self.converted.is_some() => {
                self.converted = None;
                self.output(environment)
            }
            ComposerEvent::Backspace => {
                let removed = match &mut self.input {
                    Input::Romaji(romaji) => romaji.backspace(),
                    Input::Kana => false,
                };
                if !removed && self.reading.pop().is_none() {
                    // 조합에 무를 것이 없으면 문서에서 지운다
                    return ComposerOutput {
                        delete_before_commit: 1,
                        ..ComposerOutput::default()
                    };
                }
                self.output(environment)
            }
            ComposerEvent::CandidateSelected { text, key } => self.select(text, key, environment),
        }
    }

    /// 커서가 옮겨 가거나 초점을 잃으면 보고 있던 것을 그대로 확정한다 — 변환 중이면 그
    /// 표기가, 아니면 읽기가 남는다.
    fn finalize(&mut self) -> Option<CommittedText> {
        self.commit_all()
    }

    fn is_composing(&self) -> bool {
        if self.converted.is_some() || !self.reading.is_empty() {
            return true;
        }
        match &self.input {
            Input::Romaji(romaji) => !romaji.pending().is_empty(),
            Input::Kana => false,
        }
    }

    fn snapshot(&self) -> ComposerState {
        // 변환 결과까지 담는다 — 익스텐션이 되살아난 자리에서 사전을 다시 뒤지지 않는다
        let mut lines = vec![self.reading.clone()];
        if let Input::Romaji(romaji) = &self.input {
            lines.push(romaji.pending().to_string());
        } else {
            lines.push(String::new());
        }
        if let Some(converted) = &self.converted {
            lines.push(converted.focus.to_string());
            lines.push(converted.choice.to_string());
            for segment in &converted.segments {
                lines.push(format!("{}\t{}", segment.reading, segment.surface));
            }
        }
        ComposerState::from_text(&lines.join("\n"))
    }

    fn restore(&mut self, state: ComposerState) {
        let Some(text) = state.text() else { return };
        let mut lines = text.split('\n');
        let (Some(reading), Some(pending)) = (lines.next(), lines.next()) else {
            return;
        };
        self.reading = reading.to_string();
        if let Input::Romaji(romaji) = &mut self.input {
            romaji.clear();
            for letter in pending.chars() {
                romaji.push(letter);
            }
        }
        let (Some(focus), Some(choice)) = (lines.next(), lines.next()) else {
            self.converted = None;
            return;
        };
        let (Ok(focus), Ok(choice)) = (focus.parse(), choice.parse()) else {
            return;
        };
        let segments: Vec<Segment> = lines
            .filter_map(|line| {
                let (reading, surface) = line.split_once('\t')?;
                Some(Segment {
                    reading: reading.to_string(),
                    surface: surface.to_string(),
                })
            })
            .collect();
        self.converted = (!segments.is_empty()).then_some(Converted {
            segments,
            focus,
            choice,
        });
    }
}

impl JapaneseComposer {
    /// 후보를 골랐다. 고른 것이 조합 전체를 덮으면 그대로 확정하고, 앞 도막만 덮으면 그
    /// 만큼만 확정한 뒤 남은 읽기로 조합을 잇는다 — 문절 단위로 확정해 나가는 순정의 길이다.
    fn select(
        &mut self,
        text: String,
        key: String,
        environment: &ComposerEnvironment<'_>,
    ) -> ComposerOutput {
        let whole = match &self.converted {
            Some(converted) => converted.reading(),
            None => self.reading.clone(),
        };
        let remainder = whole.strip_prefix(&key).unwrap_or("").to_string();
        self.converted = None;
        self.reading = remainder.clone();
        if let Input::Romaji(romaji) = &mut self.input {
            romaji.clear();
        }
        // 남은 읽기가 있으면 곧바로 다시 변환해 보인다 — 사람이 스페이스를 다시 누르게
        // 하지 않는 것이 순정이다
        if !remainder.is_empty() {
            self.convert(environment);
        }
        let mut output = self.output(environment);
        output.commit = Some(CommittedText {
            surface: text,
            reading: Some(key),
        });
        output
    }
}
