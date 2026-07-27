//! 설정이 화면과 입력에 실제로 닿는지 — 값이 코어까지 흘러 들어와 결과를 바꾸는가.

use taza_engine::contract::{
    CursorSensitivity, EditorContext, Effect, FieldKind, FieldTraits, InputEvent, KeyboardHeight,
    UserPreferences,
};
use taza_engine::engine::Engine;
use taza_engine::keyboard::{FormFactor, KeySignal, KeyboardFrame, KeyboardMetrics};
use taza_engine::lang::LanguageDescriptor;

fn engine(preferences: UserPreferences) -> Engine {
    let mut engine = Engine::new(LanguageDescriptor::builtin("en").unwrap()).unwrap();
    engine.set_metrics(KeyboardMetrics {
        form_factor: FormFactor::PhonePortrait,
        width_points: 390.0,
        text_scale: 1.0,
    });
    engine.set_preferences(preferences);
    engine
}

fn context(text: &str) -> EditorContext {
    EditorContext {
        text_before_cursor: Some(text.to_string()),
        incognito: false,
        field: FieldKind::Text,
    }
}

fn key_center(frame: &KeyboardFrame, label: &str) -> (f32, f32) {
    for row in &frame.rows {
        for key in row {
            if key.label == label {
                return (
                    key.bounds.x + key.bounds.width / 2.0,
                    key.bounds.y + key.bounds.height / 2.0,
                );
            }
        }
    }
    panic!("키 {label:?} 없음");
}

fn has_key(frame: &KeyboardFrame, label: &str) -> bool {
    frame.rows.iter().flatten().any(|key| key.label == label)
}

fn key(character: char) -> InputEvent {
    InputEvent::Key(KeySignal::certain(character))
}

fn committed(effects: &[Effect]) -> String {
    effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::CommitText(text) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn number_row_adds_a_row_and_raises_the_keyboard() {
    let plain = engine(UserPreferences::default());
    let plain_height = plain.frame_metrics().grid_height;
    assert!(!has_key(&plain.frame(), "1"));

    let with_row = engine(UserPreferences {
        number_row: true,
        ..UserPreferences::default()
    });
    let frame = with_row.frame();
    assert!(has_key(&frame, "1"));
    assert!(has_key(&frame, "0"));
    assert_eq!(frame.rows.len(), plain.frame().rows.len() + 1);
    assert!(with_row.frame_metrics().grid_height > plain_height);
}

#[test]
fn number_row_stays_off_the_number_pad() {
    let mut keyboard = engine(UserPreferences {
        number_row: true,
        ..UserPreferences::default()
    });
    keyboard.set_field(FieldTraits::of(FieldKind::Phone));
    // 숫자 패드에는 이미 숫자가 있다 — 행을 덧붙이면 같은 키가 두 벌이 된다
    assert_eq!(keyboard.frame().rows.len(), 4);
}

#[test]
fn keyboard_height_scales_the_grid_only() {
    let standard = engine(UserPreferences::default()).frame_metrics();
    let tall = engine(UserPreferences {
        keyboard_height: KeyboardHeight::Tall,
        ..UserPreferences::default()
    })
    .frame_metrics();
    assert!(tall.grid_height > standard.grid_height);
    assert_eq!(tall.candidate_bar_height, standard.candidate_bar_height);
}

#[test]
fn candidate_bar_can_be_kept_in_fields_that_drop_it() {
    let mut default = engine(UserPreferences::default());
    default.set_field(FieldTraits::of(FieldKind::Email));
    assert_eq!(default.frame_metrics().candidate_bar_height, 0.0);

    let mut always = engine(UserPreferences {
        candidate_bar_always: true,
        ..UserPreferences::default()
    });
    always.set_field(FieldTraits::of(FieldKind::Email));
    assert!(always.frame_metrics().candidate_bar_height > 0.0);
}

#[test]
fn turning_off_alternates_empties_the_long_press_list() {
    let with = engine(UserPreferences::default());
    let (x, y) = key_center(&with.frame(), "a");
    assert!(!with.key_at(x, y).alternates.is_empty());

    let without = engine(UserPreferences {
        key_alternates: false,
        ..UserPreferences::default()
    });
    assert!(without.key_at(x, y).alternates.is_empty());
}

#[test]
fn cursor_sensitivity_changes_how_far_a_drag_travels() {
    let steps = |sensitivity| {
        let mut engine = engine(UserPreferences {
            cursor_sensitivity: sensitivity,
            ..UserPreferences::default()
        });
        engine.begin_cursor_drag(0.0);
        engine
            .update_cursor_drag(0.2, &context(""))
            .into_iter()
            .find_map(|effect| match effect {
                Effect::MoveCursor(steps) => Some(steps),
                _ => None,
            })
            .unwrap_or(0)
    };
    assert!(steps(CursorSensitivity::High) > steps(CursorSensitivity::Standard));
    assert!(steps(CursorSensitivity::Standard) > steps(CursorSensitivity::Low));
}

#[test]
fn auto_capitalization_raises_shift_at_a_sentence_start() {
    let engine = engine(UserPreferences::default());
    let (x, y) = key_center(&engine.frame(), "a");

    let mut fresh = self::engine(UserPreferences::default());
    fresh.sync_auto_shift(&context(""));
    assert_eq!(fresh.key_at(x, y).label, "A");

    let mut mid_sentence = self::engine(UserPreferences::default());
    mid_sentence.sync_auto_shift(&context("still typing "));
    assert_eq!(mid_sentence.key_at(x, y).label, "a");
}

#[test]
fn typing_a_period_and_space_raises_shift_without_waiting_for_the_shell() {
    let mut engine = engine(UserPreferences::default());
    let frame = engine.frame();
    let (x, y) = key_center(&frame, "a");
    engine.sync_auto_shift(&context(""));
    // 문장 첫 글자를 치면 shift가 내려간다
    engine.press_at(x, y, &context(""));
    assert_eq!(engine.key_at(x, y).label, "a");

    // 마침표 뒤에 공백을 치면 그 자리에서 다시 올라간다 — 셸이 문맥을 다시 읽어
    // 오기를 기다리지 않는다. 스페이스바 라벨은 순정처럼 현재 언어명이다.
    let (space_x, space_y) = key_center(&frame, "␣");
    let result = engine.press_at(space_x, space_y, &context("Hi."));
    assert!(result.layout_changed);
    assert_eq!(engine.key_at(x, y).label, "A");
}

#[test]
fn auto_capitalization_leaves_email_fields_alone() {
    let mut engine = engine(UserPreferences::default());
    engine.set_field(FieldTraits::of(FieldKind::Email));
    let (x, y) = key_center(&engine.frame(), "a");
    engine.sync_auto_shift(&EditorContext {
        field: FieldKind::Email,
        ..context("")
    });
    assert_eq!(engine.key_at(x, y).label, "a");
}

#[test]
fn smart_punctuation_curls_quotes_by_what_precedes_them() {
    let mut engine = engine(UserPreferences::default());
    let opening = engine.handle(key('"'), &context("said "));
    assert_eq!(committed(&opening), "\u{201C}");
    let closing = engine.handle(key('"'), &context("said \u{201C}hi"));
    assert_eq!(committed(&closing), "\u{201D}");
}

#[test]
fn smart_punctuation_off_leaves_the_straight_quote() {
    let mut engine = engine(UserPreferences {
        smart_punctuation: false,
        ..UserPreferences::default()
    });
    let effects = engine.handle(key('"'), &context("said "));
    assert_eq!(committed(&effects), "\"");
}

#[test]
fn auto_pairing_inserts_the_closing_half_and_steps_back() {
    let mut engine = engine(UserPreferences {
        auto_pairing: true,
        ..UserPreferences::default()
    });
    let effects = engine.handle(key('('), &context("call"));
    assert_eq!(committed(&effects), "()");
    assert!(effects.contains(&Effect::MoveCursor(-1)));
}

/// 합성기를 건너뛰는 편집(자동 짝 넣기·줄표)도 어절을 끊는다. 남겨 두면 다음 경계에서
/// 그 어절 길이만큼 지우는데, 커서는 이미 괄호 안이라 엉뚱한 글자가 사라진다.
#[test]
fn auto_pairing_does_not_carry_the_word_across() {
    use std::collections::BTreeMap;
    let mut engine = engine(UserPreferences {
        auto_pairing: true,
        ..UserPreferences::default()
    });
    engine.set_shortcuts(BTreeMap::from([("hix".to_string(), "HIX".to_string())]));
    for character in "hi".chars() {
        engine.handle(key(character), &context(""));
    }
    engine.handle(key('('), &context("hi"));
    // 문맥을 못 받는 앱에서도 어절은 끊겨 있어야 한다
    let unavailable = EditorContext::unavailable();
    engine.handle(key('x'), &unavailable);
    let effects = engine.handle(InputEvent::Separator(' '), &unavailable);
    assert_eq!(committed(&effects), " ");
    assert!(
        !effects
            .iter()
            .any(|effect| matches!(effect, Effect::DeleteBackward(_))),
        "끊긴 어절을 되살려 괄호 너머까지 지웠다"
    );
}

#[test]
fn turning_off_annotations_rebuilds_the_ranking_policy() {
    // 팩이 없으면 곁들일 것 자체가 없으므로 여기서는 설정이 세션을 다시 세우는지만 본다 —
    // 실제 순위는 팩을 쓰는 골든 테스트가 본다
    let mut engine = engine(UserPreferences {
        annotation_candidates: false,
        ..UserPreferences::default()
    });
    engine.set_preferences(UserPreferences::default());
    assert!(engine.handle(key('a'), &context("")).len() <= 2);
}

#[test]
fn a_user_shortcut_replaces_the_typed_word_at_the_boundary() {
    use std::collections::BTreeMap;
    let mut engine = engine(UserPreferences::default());
    engine.set_shortcuts(BTreeMap::from([(
        "ttyl".to_string(),
        "talk to you later".to_string(),
    )]));
    for character in "ttyl".chars() {
        engine.handle(key(character), &context(""));
    }
    let effects = engine.handle(InputEvent::Separator(' '), &context("ttyl"));
    assert_eq!(committed(&effects), "talk to you later ");

    // 바로 뒤의 Backspace는 친 대로 되돌린다 — 자동교정 되돌리기와 같은 길이다
    let reverted = engine.handle(InputEvent::Backspace, &context("talk to you later "));
    assert_eq!(committed(&reverted), "ttyl");
}
