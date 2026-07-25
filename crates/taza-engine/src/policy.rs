//! 언어를 가리지 않는 순정 키보드 관습 규칙. 계약(타입)이나 조립(engine)이 아니라
//! "무엇을 해야 하는가"의 판단만 둔다.

use crate::contract::{CommittedText, ComposerOutput, EditorContext, UserPreferences};

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
