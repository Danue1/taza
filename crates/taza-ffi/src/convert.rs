//! 코어 계약 ↔ FFI 타입 번역. 이 파일에는 옮기는 일만 있고 판단은 없다 —
//! 어느 쪽 값이 옳은지는 언제나 코어가 정한다.

use taza_engine::contract::{
    Candidate, CandidateGroup, CandidateKind, Capitalization, CursorSensitivity, EditorContext,
    Effect, EmojiCategory, FieldKind, FieldTraits, InputEvent, KeySignal, KeyboardHeight,
    ReturnKey, UserPreferences,
};
use taza_engine::keyboard::{FormFactor, FrameKey, FrameMetrics, KeyLegend, KeyRole};

use crate::types::*;

pub(crate) fn convert_form_factor(form_factor: FfiFormFactor) -> FormFactor {
    match form_factor {
        FfiFormFactor::PhonePortrait => FormFactor::PhonePortrait,
        FfiFormFactor::PhoneLandscape => FormFactor::PhoneLandscape,
        FfiFormFactor::Tablet => FormFactor::Tablet,
    }
}

pub(crate) fn convert_candidate_group_in(group: FfiCandidateGroup) -> CandidateGroup {
    match group {
        FfiCandidateGroup::Word => CandidateGroup::Word,
        FfiCandidateGroup::Emoji => CandidateGroup::Emoji,
        FfiCandidateGroup::Symbol => CandidateGroup::Symbol,
        FfiCandidateGroup::Emoticon => CandidateGroup::Emoticon,
    }
}

pub(crate) fn convert_field_traits(traits: &FfiFieldTraits) -> FieldTraits {
    FieldTraits {
        kind: convert_field(&traits.kind),
        return_key: match traits.return_key {
            FfiReturnKey::Return => ReturnKey::Return,
            FfiReturnKey::Go => ReturnKey::Go,
            FfiReturnKey::Search => ReturnKey::Search,
            FfiReturnKey::Send => ReturnKey::Send,
            FfiReturnKey::Next => ReturnKey::Next,
            FfiReturnKey::Done => ReturnKey::Done,
            FfiReturnKey::Join => ReturnKey::Join,
            FfiReturnKey::Route => ReturnKey::Route,
            FfiReturnKey::Continue => ReturnKey::Continue,
        },
        capitalization: match traits.capitalization {
            FfiCapitalization::None => Capitalization::None,
            FfiCapitalization::Words => Capitalization::Words,
            FfiCapitalization::Sentences => Capitalization::Sentences,
            FfiCapitalization::AllCharacters => Capitalization::AllCharacters,
        },
        autocorrect: traits.autocorrect,
        smart_punctuation: traits.smart_punctuation,
    }
}

/// 아직 사용자가 건드리지 않은 설정의 값. 기본값의 주인은 코어이므로 셸은 자기 표를
/// 두지 않고 이 함수를 부른다.
#[uniffi::export]
pub fn default_user_preferences() -> FfiUserPreferences {
    convert_preferences_out(UserPreferences::default())
}

pub(crate) fn convert_preferences_out(preferences: UserPreferences) -> FfiUserPreferences {
    FfiUserPreferences {
        auto_correction: preferences.auto_correction,
        predictions: preferences.predictions,
        double_space_period: preferences.double_space_period,
        personalized_learning: preferences.personalized_learning,
        auto_capitalization: preferences.auto_capitalization,
        smart_punctuation: preferences.smart_punctuation,
        auto_pairing: preferences.auto_pairing,
        annotation_candidates: preferences.annotation_candidates,
        key_alternates: preferences.key_alternates,
        number_row: preferences.number_row,
        candidate_bar_always: preferences.candidate_bar_always,
        keyboard_height: match preferences.keyboard_height {
            KeyboardHeight::Compact => FfiKeyboardHeight::Compact,
            KeyboardHeight::Standard => FfiKeyboardHeight::Standard,
            KeyboardHeight::Tall => FfiKeyboardHeight::Tall,
        },
        cursor_sensitivity: match preferences.cursor_sensitivity {
            CursorSensitivity::Low => FfiCursorSensitivity::Low,
            CursorSensitivity::Standard => FfiCursorSensitivity::Standard,
            CursorSensitivity::High => FfiCursorSensitivity::High,
        },
    }
}

pub(crate) fn convert_preferences_in(preferences: FfiUserPreferences) -> UserPreferences {
    UserPreferences {
        auto_correction: preferences.auto_correction,
        predictions: preferences.predictions,
        double_space_period: preferences.double_space_period,
        personalized_learning: preferences.personalized_learning,
        auto_capitalization: preferences.auto_capitalization,
        smart_punctuation: preferences.smart_punctuation,
        auto_pairing: preferences.auto_pairing,
        annotation_candidates: preferences.annotation_candidates,
        key_alternates: preferences.key_alternates,
        number_row: preferences.number_row,
        candidate_bar_always: preferences.candidate_bar_always,
        keyboard_height: match preferences.keyboard_height {
            FfiKeyboardHeight::Compact => KeyboardHeight::Compact,
            FfiKeyboardHeight::Standard => KeyboardHeight::Standard,
            FfiKeyboardHeight::Tall => KeyboardHeight::Tall,
        },
        cursor_sensitivity: match preferences.cursor_sensitivity {
            FfiCursorSensitivity::Low => CursorSensitivity::Low,
            FfiCursorSensitivity::Standard => CursorSensitivity::Standard,
            FfiCursorSensitivity::High => CursorSensitivity::High,
        },
    }
}

pub(crate) fn convert_emoji_category(category: EmojiCategory) -> FfiEmojiCategory {
    match category {
        EmojiCategory::SmileysAndPeople => FfiEmojiCategory::SmileysAndPeople,
        EmojiCategory::AnimalsAndNature => FfiEmojiCategory::AnimalsAndNature,
        EmojiCategory::FoodAndDrink => FfiEmojiCategory::FoodAndDrink,
        EmojiCategory::Activities => FfiEmojiCategory::Activities,
        EmojiCategory::TravelAndPlaces => FfiEmojiCategory::TravelAndPlaces,
        EmojiCategory::Objects => FfiEmojiCategory::Objects,
        EmojiCategory::Symbols => FfiEmojiCategory::Symbols,
        EmojiCategory::Flags => FfiEmojiCategory::Flags,
    }
}

pub(crate) fn convert_event(event: FfiInputEvent) -> Option<InputEvent> {
    Some(match event {
        // 좌표 없이 오는 키는 물리 키보드·접근성 경로 — 어느 키인지 확실하다
        FfiInputEvent::Key { character } => {
            InputEvent::Key(KeySignal::certain(character.chars().next()?))
        }
        FfiInputEvent::Text { text } => InputEvent::Text(text),
        FfiInputEvent::Backspace => InputEvent::Backspace,
        FfiInputEvent::Separator { character } => InputEvent::Separator(character.chars().next()?),
        FfiInputEvent::CandidateSelected { index } => InputEvent::CandidateSelected(index as usize),
        FfiInputEvent::CursorMoved => InputEvent::CursorMoved,
        FfiInputEvent::FocusLost => InputEvent::FocusLost,
    })
}

pub(crate) fn convert_candidate(candidate: Candidate) -> FfiCandidate {
    FfiCandidate {
        text: candidate.text,
        kind: match candidate.kind {
            CandidateKind::Typed => FfiCandidateKind::Typed,
            CandidateKind::Prediction => FfiCandidateKind::Prediction,
            CandidateKind::Conversion => FfiCandidateKind::Conversion,
            CandidateKind::Correction => FfiCandidateKind::Correction,
        },
        group: convert_candidate_group(candidate.group),
    }
}

pub(crate) fn convert_candidate_group(group: CandidateGroup) -> FfiCandidateGroup {
    match group {
        CandidateGroup::Word => FfiCandidateGroup::Word,
        CandidateGroup::Emoji => FfiCandidateGroup::Emoji,
        CandidateGroup::Symbol => FfiCandidateGroup::Symbol,
        CandidateGroup::Emoticon => FfiCandidateGroup::Emoticon,
    }
}

pub(crate) fn convert_effect(effect: Effect) -> FfiEffect {
    match effect {
        Effect::CommitText(text) => FfiEffect::CommitText { text },
        Effect::SetComposing(composing) => FfiEffect::SetComposing {
            text: composing.text,
            caret: composing.caret as u32,
        },
        Effect::ClearComposing => FfiEffect::ClearComposing,
        Effect::DeleteBackward(count) => FfiEffect::DeleteBackward {
            code_points: count as u32,
        },
        Effect::UpdateCandidates(candidates) => FfiEffect::UpdateCandidates {
            candidates: candidates.into_iter().map(convert_candidate).collect(),
        },
        Effect::MoveCursor(offset) => FfiEffect::MoveCursor { offset },
        Effect::SetTimer(milliseconds) => FfiEffect::SetTimer { milliseconds },
    }
}

pub(crate) fn convert_frame_key(key: FrameKey) -> FfiFrameKey {
    FfiFrameKey {
        row: key.position.row as u32,
        index: key.position.index as u32,
        label: key.label,
        legend: key.legend.map(|legend| match legend {
            KeyLegend::Return => FfiKeyLegend::Return,
            KeyLegend::Go => FfiKeyLegend::Go,
            KeyLegend::Search => FfiKeyLegend::Search,
            KeyLegend::Send => FfiKeyLegend::Send,
            KeyLegend::Next => FfiKeyLegend::Next,
            KeyLegend::Done => FfiKeyLegend::Done,
            KeyLegend::Join => FfiKeyLegend::Join,
            KeyLegend::Route => FfiKeyLegend::Route,
            KeyLegend::Continue => FfiKeyLegend::Continue,
        }),
        bounds: FfiKeyBounds {
            x: key.bounds.x,
            y: key.bounds.y,
            width: key.bounds.width,
            height: key.bounds.height,
        },
        shift_active: key.shift_active,
        emphasized: key.emphasized,
        role: match key.role {
            KeyRole::Character => FfiKeyRole::Character,
            KeyRole::Shift => FfiKeyRole::Shift,
            KeyRole::Backspace => FfiKeyRole::Backspace,
            KeyRole::Space => FfiKeyRole::Space,
            KeyRole::Enter => FfiKeyRole::Enter,
            KeyRole::LayerSwitch => FfiKeyRole::LayerSwitch,
            KeyRole::LanguageSwitch => FfiKeyRole::LanguageSwitch,
            KeyRole::LanguageSelect => FfiKeyRole::LanguageSelect,
            KeyRole::CursorRight => FfiKeyRole::CursorRight,
            KeyRole::Blank => FfiKeyRole::Blank,
        },
        font_size: key.font_size,
        alternates: key.alternates,
    }
}

pub(crate) fn convert_frame_metrics(metrics: FrameMetrics) -> FfiFrameMetrics {
    FfiFrameMetrics {
        grid_height: metrics.grid_height,
        candidate_bar_height: metrics.candidate_bar_height,
        total_height: metrics.total_height(),
        letter_font_size: metrics.letter_font_size,
    }
}

pub(crate) fn convert_field(field: &FfiFieldKind) -> FieldKind {
    match field {
        FfiFieldKind::Text => FieldKind::Text,
        FfiFieldKind::Email => FieldKind::Email,
        FfiFieldKind::Url => FieldKind::Url,
        FfiFieldKind::Search => FieldKind::Search,
        FfiFieldKind::Number => FieldKind::Number,
        FfiFieldKind::Decimal => FieldKind::Decimal,
        FfiFieldKind::Phone => FieldKind::Phone,
        FfiFieldKind::Password => FieldKind::Password,
    }
}

pub(crate) fn convert_context(context: &FfiEditorContext) -> EditorContext {
    EditorContext {
        text_before_cursor: context.text_before_cursor.clone(),
        incognito: context.incognito,
        field: convert_field(&context.field),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 계약에 갈래가 늘면 여기서 컴파일이 깨진다. 셸이 값을 만들어 넘기기만 하는
    /// 갈래들(필드·리턴키·대문자화·폼팩터)은 반대 방향 번역이 없어서, 코어가 갈래를
    /// 늘려도 아무 데서도 걸리지 않는다 — 셸이 모르는 갈래가 조용히 생기는 것을 막는다.
    #[test]
    fn 셸이_넘기는_갈래는_계약과_일대일이다() {
        fn field(kind: FieldKind) -> FfiFieldKind {
            match kind {
                FieldKind::Text => FfiFieldKind::Text,
                FieldKind::Email => FfiFieldKind::Email,
                FieldKind::Url => FfiFieldKind::Url,
                FieldKind::Search => FfiFieldKind::Search,
                FieldKind::Number => FfiFieldKind::Number,
                FieldKind::Decimal => FfiFieldKind::Decimal,
                FieldKind::Phone => FfiFieldKind::Phone,
                FieldKind::Password => FfiFieldKind::Password,
            }
        }
        fn return_key(key: ReturnKey) -> FfiReturnKey {
            match key {
                ReturnKey::Return => FfiReturnKey::Return,
                ReturnKey::Go => FfiReturnKey::Go,
                ReturnKey::Search => FfiReturnKey::Search,
                ReturnKey::Send => FfiReturnKey::Send,
                ReturnKey::Next => FfiReturnKey::Next,
                ReturnKey::Done => FfiReturnKey::Done,
                ReturnKey::Join => FfiReturnKey::Join,
                ReturnKey::Route => FfiReturnKey::Route,
                ReturnKey::Continue => FfiReturnKey::Continue,
            }
        }
        fn capitalization(capitalization: Capitalization) -> FfiCapitalization {
            match capitalization {
                Capitalization::None => FfiCapitalization::None,
                Capitalization::Words => FfiCapitalization::Words,
                Capitalization::Sentences => FfiCapitalization::Sentences,
                Capitalization::AllCharacters => FfiCapitalization::AllCharacters,
            }
        }
        fn form_factor(form_factor: FormFactor) -> FfiFormFactor {
            match form_factor {
                FormFactor::PhonePortrait => FfiFormFactor::PhonePortrait,
                FormFactor::PhoneLandscape => FfiFormFactor::PhoneLandscape,
                FormFactor::Tablet => FfiFormFactor::Tablet,
            }
        }

        // 되돌려 보내면 제자리로 온다 — 두 표가 같은 짝을 가리키는지 확인한다
        assert_eq!(
            convert_field(&field(FieldKind::Password)),
            FieldKind::Password
        );
        assert_eq!(
            convert_form_factor(form_factor(FormFactor::Tablet)),
            FormFactor::Tablet
        );
        let traits = FfiFieldTraits {
            kind: field(FieldKind::Search),
            return_key: return_key(ReturnKey::Send),
            capitalization: capitalization(Capitalization::Words),
            autocorrect: true,
            smart_punctuation: true,
        };
        let converted = convert_field_traits(&traits);
        assert_eq!(converted.return_key, ReturnKey::Send);
        assert_eq!(converted.capitalization, Capitalization::Words);
    }
}
