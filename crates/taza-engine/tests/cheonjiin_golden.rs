use taza_engine::contract::{Composer, ComposerEvent, EditorContext, Effect, InputEvent};
use taza_engine::engine::{Engine, PackBytes};
use taza_engine::keyboard::KeySignal;
use taza_engine::lang::LanguageDescriptor;
use taza_engine::lang::cheonjiin::CheonjiinComposer;

/// 이벤트 문자열: 하늘(ㆍ)·땅(ㅡ)·사람(ㅣ)과 자음은 Key, '<'는 Backspace, '_'는 공백.
/// 자음 멀티탭은 키보드가 판정하므로 여기서는 갈아 끼운 결과를 그대로 친다.
///
/// **문맥을 실어 보낸다**: 모음을 갈아 끼우는 중에는 지운 글자가 아직 문서에 남아 있어
/// 합성 재개(채택)가 그것을 도로 주워 올 수 있다. 문맥이 빈 하네스는 그 길을 열지 못한다.
fn run(events: &str) -> (String, Option<String>) {
    let mut composer = CheonjiinComposer::new();
    let mut committed = String::new();
    let mut composing: Option<String> = None;
    for character in events.chars() {
        let event = match character {
            '<' => ComposerEvent::Backspace,
            '_' => ComposerEvent::Separator(' '),
            _ => ComposerEvent::Key(character),
        };
        let context = EditorContext {
            text_before_cursor: Some(format!("{committed}{}", composing.as_deref().unwrap_or(""))),
            ..EditorContext::unavailable()
        };
        let output = composer.feed(event, &context);
        for _ in 0..output.delete_before_commit {
            committed.pop();
        }
        if let Some(commit) = output.commit {
            committed.push_str(&commit.surface);
        }
        composing = output.composing.map(|text| text.text);
    }
    (committed, composing)
}

#[track_caller]
fn assert_text(events: &str, expected: &str) {
    let (committed, composing) = run(events);
    assert_eq!(
        format!("{committed}{}", composing.unwrap_or_default()),
        expected,
        "for {events:?}"
    );
}

#[test]
fn builds_vowels_from_sky_ground_and_human() {
    assert_text("ㅇㅣㆍ", "아");
    assert_text("ㅇㅣㆍㆍ", "야");
    assert_text("ㅇㆍㅣ", "어");
    assert_text("ㅇㆍㆍㅣ", "여");
    assert_text("ㅇㆍㅡ", "오");
    assert_text("ㅇㆍㆍㅡ", "요");
    assert_text("ㅇㅡㆍ", "우");
    assert_text("ㅇㅡㆍㆍ", "유");
    assert_text("ㅇㅡ", "으");
    assert_text("ㅇㅣ", "이");
}

/// 사람(ㅣ)을 덧대면 겹모음이 된다 — 두벌식이라면 ㅏ 다음 ㅣ는 새 글자다.
#[test]
fn adds_human_to_make_compound_vowels() {
    assert_text("ㅇㅣㆍㅣ", "애");
    assert_text("ㅇㆍㅣㅣ", "에");
    assert_text("ㅇㅡㅣ", "의");
    assert_text("ㅇㆍㅡㅣ", "외");
    assert_text("ㅇㆍㅡㅣㆍ", "와");
    assert_text("ㅇㅡㆍㆍㅣ", "워");
}

/// 초성 없이 모음부터 치면 조합 창에 그 모음만 있다 — 갈아 끼울 때 창이 비므로,
/// 지운 글자를 문맥에서 도로 주워 오지 않는지가 여기서 드러난다.
#[test]
fn replaces_a_bare_vowel_without_readopting_it() {
    assert_text("ㅣㆍ", "ㅏ");
    assert_text("ㅣㆍㅣ", "ㅐ");
    assert_text("ㆍㅡ", "ㅗ");
}

/// 하늘 하나는 아직 글자가 아니다 — 무엇이 될지는 다음 타건이 정한다.
#[test]
fn a_lone_sky_tap_shows_nothing_yet() {
    assert_text("ㅇㆍ", "ㅇ");
    assert_text("ㅇㆍ<", "ㅇ");
    assert_text("ㅇㆍ<ㅣㆍ", "아");
}

/// 모음 타건은 하나씩 물러난다.
#[test]
fn backspace_unwinds_one_tap_at_a_time() {
    assert_text("ㄱㅣㆍㆍ<", "가");
    assert_text("ㄱㅣㆍㆍ<<", "기");
    assert_text("ㄱㅣㆍㆍ<<<", "ㄱ");
}

/// 종성과 도깨비불은 두벌식과 같은 규칙이다 — 음절을 쌓는 일은 속 합성기가 한다.
#[test]
fn syllables_stack_like_dubeolsik() {
    assert_text("ㄱㅣㆍㅂ", "갑");
    assert_text("ㄱㅣㆍㅂㅣㆍ", "가바");
    assert_text("ㅎㅣㆍㄱㄱㅛ", "학교");
}

/// 셸이 하는 일을 그대로 흉내 내는 문서 — 한국어 iOS 안전 모드에서는 조합 중 텍스트도
/// 문서에 들어가 있으므로(marked text를 쓰지 않는다), 무엇이 화면에 남는지는 Effect를
/// 순서대로 적용해 봐야 알 수 있다.
#[derive(Default)]
struct Document {
    text: String,
    composing: String,
}

impl Document {
    fn truncate(&mut self, count: usize) {
        for _ in 0..count {
            self.text.pop();
        }
    }

    fn apply(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::CommitText(committed) => {
                    self.truncate(self.composing.chars().count());
                    self.composing.clear();
                    self.text.push_str(&committed);
                }
                Effect::SetComposing(text) => {
                    let common = self
                        .composing
                        .chars()
                        .zip(text.text.chars())
                        .take_while(|(left, right)| left == right)
                        .count();
                    self.truncate(self.composing.chars().count() - common);
                    self.text.extend(text.text.chars().skip(common));
                    self.composing = text.text;
                }
                Effect::ClearComposing => {
                    self.truncate(self.composing.chars().count());
                    self.composing.clear();
                }
                Effect::DeleteBackward(count) => self.truncate(count),
                _ => {}
            }
        }
    }

    fn context(&self) -> EditorContext {
        EditorContext {
            text_before_cursor: Some(self.text.clone()),
            ..EditorContext::unavailable()
        }
    }
}

fn cheonjiin_engine() -> Engine {
    use taza_engine::pack::SectionKind;
    use taza_toolchain::PackWriter;
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/korean-layout.txt"
    ))
    .unwrap();
    let mut writer = PackWriter::new("ko");
    writer.add_section(
        SectionKind::Layout,
        taza_toolchain::section::layout::serialize(
            &taza_toolchain::section::layout::parse(&text).unwrap(),
        ),
    );
    let mut engine = Engine::new(LanguageDescriptor::builtin("ko").unwrap()).unwrap();
    engine
        .load_pack(std::sync::Arc::new(writer.finish()) as std::sync::Arc<dyn PackBytes>)
        .unwrap();
    assert!(engine.select_layout("천지인"));
    engine
}

/// 같은 키를 이어 누르면 주기의 다음 글자로 **갈아 끼운다** — 덧붙이지 않는다.
/// 아무것도 치지 않은 자리에서 시작하는 것이 이 검증의 핵심이다.
#[test]
fn multitap_replaces_the_previous_letter_until_the_timer_ends_it() {
    let press = |engine: &mut Engine, document: &mut Document, row: usize, index: usize| {
        let bounds = engine.frame().rows[row][index].bounds;
        let result = engine.press_at(
            bounds.x + bounds.width / 2.0,
            bounds.y + bounds.height / 2.0,
            &document.context(),
        );
        let timer = result.effects.iter().find_map(|effect| match effect {
            Effect::SetTimer(milliseconds) => Some(*milliseconds),
            _ => None,
        });
        document.apply(result.effects);
        timer
    };

    let mut engine = cheonjiin_engine();
    let mut document = Document::default();
    assert!(
        press(&mut engine, &mut document, 1, 0).is_some(),
        "멀티탭은 주기를 끊을 시한을 요청한다"
    );
    assert_eq!(document.text, "ㄱ");
    press(&mut engine, &mut document, 1, 0);
    assert_eq!(document.text, "ㅋ", "이어 누르면 갈아 끼운다");
    press(&mut engine, &mut document, 1, 0);
    assert_eq!(document.text, "ㄲ");
    press(&mut engine, &mut document, 1, 0);
    assert_eq!(document.text, "ㄱ", "주기가 한 바퀴 돌아도 갈아 끼운다");

    // 시한이 다 되면 같은 키가 다시 첫 글자로 시작한다
    let mut engine = cheonjiin_engine();
    let mut document = Document::default();
    press(&mut engine, &mut document, 1, 0);
    engine.timer_fired();
    press(&mut engine, &mut document, 1, 0);
    assert_eq!(document.text, "ㄱㄱ");
}

/// 스냅샷은 쌓던 타건까지 담는다 — 익스텐션이 죽었다 살아나도 모음이 이어진다.
#[test]
fn snapshot_keeps_the_taps() {
    let context = EditorContext::unavailable();
    let mut composer = CheonjiinComposer::new();
    for character in "ㄱㅣㆍ".chars() {
        composer.feed(ComposerEvent::Key(character), &context);
    }
    let mut restored = CheonjiinComposer::new();
    restored.restore(composer.snapshot());
    let output = restored.feed(ComposerEvent::Key('ㆍ'), &context);
    assert_eq!(output.composing.unwrap().text, "갸");
}

/// 셸이 만들지 않는 이벤트다 — Engine이 멀티탭 판정에서 스스로 낸다.
#[test]
fn retap_deletes_before_inserting() {
    let mut engine = Engine::new(LanguageDescriptor::builtin("ko").unwrap()).unwrap();
    let context = EditorContext::unavailable();
    engine.handle(InputEvent::Key(KeySignal::certain('ㄱ')), &context);
    let effects = engine.handle(InputEvent::Retap('ㅋ'), &context);
    assert!(effects.contains(&Effect::SetComposing(
        taza_engine::contract::ComposingText {
            text: "ㅋ".to_string(),
            caret: 1,
        }
    )));
}
