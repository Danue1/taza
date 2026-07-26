//! 언어를 가리지 않는 순정 키보드 관습 규칙. 계약(타입)이나 조립(engine)이 아니라
//! "무엇을 해야 하는가"의 판단만 둔다.

use crate::contract::{CommittedText, ComposerOutput, EditorContext, UserPreferences};

/// 문장을 끝내는 부호 — 이 뒤에 공백이 오면 다음 글자가 새 문장의 첫 글자다.
const SENTENCE_TERMINATORS: [char; 6] = ['.', '!', '?', '…', '。', '？'];

/// 여는 짝과 닫는 짝. 따옴표는 짝맞춤 형태로 바뀐 뒤에 이 표를 탄다.
const PAIRS: [(char, char); 6] = [
    ('(', ')'),
    ('[', ']'),
    ('{', '}'),
    ('\u{201C}', '\u{201D}'),
    ('\u{2018}', '\u{2019}'),
    ('<', '>'),
];

/// 이번 입력에서 실제로 켤 보조 기능. 사용자 설정과 입력 필드의 성격을 결합한 결과이며,
/// 셸에는 이 판단이 없다 — 셸은 설정 값과 필드 종류를 넘기기만 한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Assistance {
    pub correcting: bool,
    pub predicting: bool,
    /// 개인화 스토어를 쓸지. 끄면 기록도 조회도 하지 않는다 — 학습을 껐는데 예전에
    /// 배운 것이 계속 순위를 흔들면 설정이 한 일이 사용자 눈에 보이지 않는다.
    pub personalizing: bool,
}

pub(crate) fn assistance(preferences: &UserPreferences, context: &EditorContext) -> Assistance {
    // 비밀번호·이메일 같은 필드에서는 설정과 무관하게 전부 끈다 (순정 키보드 관습)
    let enabled = context.field.assistance_enabled();
    Assistance {
        correcting: enabled && preferences.auto_correction,
        predicting: enabled && preferences.predictions,
        personalizing: enabled && preferences.personalized_learning,
    }
}

/// 더블 스페이스 → ". " 치환(순정 키보드 공통 관습). 직전이 "단어 문자 + 공백 1개"일
/// 때만 성립한다. 언어와 무관한 규칙이므로 Engine이 공백 Separator를 합성기에 넘기기
/// 전에 먼저 시도한다 — 조합·어절이 진행 중이면 커서 앞 글자가 공백이 아니라
/// 여기서 저절로 성립하지 않는다.
pub(crate) fn double_space_period(context: &EditorContext) -> Option<ComposerOutput> {
    let text = context.text_before_cursor.as_ref()?;
    let mut characters = text.chars().rev();
    if characters.next()? != ' ' {
        return None;
    }
    if !characters.next()?.is_alphanumeric() {
        return None;
    }
    Some(ComposerOutput {
        delete_before_commit: 1,
        commit: Some(CommittedText::plain(". ".to_string())),
        ..ComposerOutput::default()
    })
}

/// 지금 자리가 문장의 첫 글자인가 — 자동 대문자화가 shift를 미리 올릴 조건이다.
/// 문맥을 못 받는 앱에서는 빈 문자열과 구분되지 않으므로 올려 둔다: 순정도 문맥을
/// 모를 때 문장 시작으로 보고, 틀렸을 때 사용자가 shift를 내리는 비용이 더 싸다.
pub(crate) fn sentence_start(text_before_cursor: Option<&str>) -> bool {
    let Some(text) = text_before_cursor else {
        return true;
    };
    let trimmed = text.trim_end_matches([' ', '\t']);
    if trimmed.is_empty() {
        return true;
    }
    // 줄이 바뀌면 공백 없이도 새 문장이다
    let last = trimmed.chars().next_back().unwrap();
    if last == '\n' {
        return true;
    }
    // 부호 바로 뒤(공백 없음)는 아직 같은 문장 — "e.g."의 g가 대문자가 되면 안 된다
    trimmed.len() < text.len() && SENTENCE_TERMINATORS.contains(&last)
}

/// 짝맞춤 부호·자동 짝 넣기가 만든 입력. 합성기를 거치지 않고 확정되므로 조합이
/// 진행 중이지 않을 때만 성립한다(호출자가 판단한다).
pub(crate) struct PunctuationOutcome {
    pub output: ComposerOutput,
    /// 넣은 뒤 커서를 되돌릴 칸수 — 자동 짝 넣기에서만 음수다
    pub cursor_offset: i32,
}

/// 곧은 따옴표를 짝맞춤 따옴표로 바꾼다. 여는 자리인지는 앞 글자가 정한다 —
/// 글자·숫자·닫는 부호 뒤면 닫는 따옴표이고, 그 밖에는 여는 따옴표다.
fn smart_quote(character: char, text_before_cursor: Option<&str>) -> Option<char> {
    let opening = match text_before_cursor.and_then(|text| text.chars().next_back()) {
        None => true,
        Some(previous) => !(previous.is_alphanumeric()
            || matches!(previous, ')' | ']' | '}' | '\u{201D}' | '\u{2019}')),
    };
    match (character, opening) {
        ('"', true) => Some('\u{201C}'),
        ('"', false) => Some('\u{201D}'),
        ('\'', true) => Some('\u{2018}'),
        ('\'', false) => Some('\u{2019}'),
        _ => None,
    }
}

fn closing_pair(character: char) -> Option<char> {
    PAIRS
        .iter()
        .find(|(open, _)| *open == character)
        .map(|(_, close)| *close)
}

/// 친 글자가 부호 규칙에 걸리는가. 걸리지 않으면 None이고, 그때 글자는 평소대로
/// 합성기로 간다 — 규칙이 꺼져 있을 때 입력 경로가 달라지지 않게 하는 조건이다.
pub(crate) fn punctuation(
    character: char,
    preferences: &UserPreferences,
    context: &EditorContext,
) -> Option<PunctuationOutcome> {
    let text = context.text_before_cursor.as_deref();
    let plain = |delete: usize, commit: String, cursor_offset: i32| PunctuationOutcome {
        output: ComposerOutput {
            delete_before_commit: delete,
            commit: Some(CommittedText::plain(commit)),
            ..ComposerOutput::default()
        },
        cursor_offset,
    };

    // `--` → 줄표. 셋째 하이픈까지 먹으면 `---`를 칠 길이 없어지므로 한 번만 바꾼다.
    if preferences.smart_punctuation && character == '-' {
        let tail: Vec<char> = text.unwrap_or("").chars().rev().take(2).collect();
        if tail.first() == Some(&'-') && tail.get(1) != Some(&'-') {
            return Some(plain(1, "\u{2014}".to_string(), 0));
        }
    }

    let substituted = preferences
        .smart_punctuation
        .then(|| smart_quote(character, text))
        .flatten();
    let effective = substituted.unwrap_or(character);

    if preferences.auto_pairing
        && let Some(closing) = closing_pair(effective)
    {
        return Some(plain(0, format!("{effective}{closing}"), -1));
    }
    substituted.map(|quote| plain(0, quote.to_string(), 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::FieldKind;

    fn context(text: &str) -> EditorContext {
        EditorContext {
            text_before_cursor: Some(text.to_string()),
            incognito: false,
            field: FieldKind::Text,
        }
    }

    #[test]
    fn sentence_start_needs_a_space_after_the_stop() {
        assert!(sentence_start(Some("")));
        assert!(sentence_start(None));
        assert!(sentence_start(Some("Done. ")));
        assert!(sentence_start(Some("Done.\n")));
        assert!(!sentence_start(Some("e.g.")));
        assert!(!sentence_start(Some("still typing")));
        assert!(!sentence_start(Some("a comma, ")));
    }

    #[test]
    fn quotes_open_and_close_by_what_precedes_them() {
        let preferences = UserPreferences::default();
        let opening = punctuation('"', &preferences, &context("said ")).unwrap();
        assert_eq!(
            opening.output.commit,
            Some(CommittedText::plain("\u{201C}".to_string()))
        );
        let closing = punctuation('"', &preferences, &context("said \u{201C}hi")).unwrap();
        assert_eq!(
            closing.output.commit,
            Some(CommittedText::plain("\u{201D}".to_string()))
        );
    }

    #[test]
    fn pairing_puts_the_caret_between_the_halves() {
        let preferences = UserPreferences {
            auto_pairing: true,
            ..UserPreferences::default()
        };
        let outcome = punctuation('(', &preferences, &context("call")).unwrap();
        assert_eq!(
            outcome.output.commit,
            Some(CommittedText::plain("()".to_string()))
        );
        assert_eq!(outcome.cursor_offset, -1);
    }

    #[test]
    fn plain_characters_stay_on_the_composer_path() {
        let preferences = UserPreferences::default();
        assert!(punctuation('a', &preferences, &context("")).is_none());
        assert!(punctuation('(', &preferences, &context("")).is_none());
    }

    #[test]
    fn double_hyphen_becomes_a_dash_once() {
        let preferences = UserPreferences::default();
        let outcome = punctuation('-', &preferences, &context("wait-")).unwrap();
        assert_eq!(outcome.output.delete_before_commit, 1);
        assert_eq!(
            outcome.output.commit,
            Some(CommittedText::plain("\u{2014}".to_string()))
        );
        assert!(punctuation('-', &preferences, &context("wait--")).is_none());
    }
}
