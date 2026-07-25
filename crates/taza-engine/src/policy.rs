//! 언어를 가리지 않는 순정 키보드 관습 규칙. 계약(타입)이나 조립(engine)이 아니라
//! "무엇을 해야 하는가"의 판단만 둔다.

use crate::contract::{CommittedText, ComposerOutput, EditorContext};

/// 더블 스페이스 → ". " 치환(순정 키보드 공통 관습). 직전이 "단어 문자 + 공백 1개"일
/// 때만 성립한다. composing이 없는 상태에서 공백 Separator를 받았을 때 호출한다.
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
