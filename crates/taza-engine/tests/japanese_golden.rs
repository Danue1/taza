//! 일본어 골든 — 타건에서 확정까지의 길을 통째로 본다.
//!
//! 다른 언어의 골든과 다른 점은 **사전이 있어야 조합 창이 선다**는 것이다. 그래서 여기서는
//! 작은 변환표를 손으로 지어 꽂는다. 표가 작으므로 무엇이 왜 그렇게 갈렸는지가 눈에 보인다.

use taza_engine::contract::{
    Composer, ComposerEnvironment, ComposerEvent, ComposerOutput, EditorContext, SuggestionRequest,
};
use taza_engine::convert::Conversion;
use taza_engine::lang::japanese::{FORM_MARKER, JapaneseComposer};
use taza_engine::pack::{Pack, SectionKind};
use taza_pack::PackWriter;
use taza_pack::section::conversion::{ConnectionBuilder, ConversionBuilder, Entry};

/// (읽기, 표기, 비용, 앞말에 붙는가)
const DICTIONARY: &[(&str, &str, u16, bool)] = &[
    ("きしゃ", "記者", 100, false),
    ("きしゃ", "汽車", 300, false),
    ("きしゃ", "貴社", 500, false),
    ("にわ", "庭", 200, false),
    ("にわ", "二羽", 900, false),
    ("に", "荷", 800, false),
    ("は", "は", 50, true),
    ("わ", "輪", 700, false),
    ("がっこう", "学校", 100, false),
    ("さくら", "桜", 120, false),
];

fn pack_bytes() -> Vec<u8> {
    let mut conversion = ConversionBuilder::new();
    for (reading, surface, cost, dependent) in DICTIONARY {
        conversion.insert(
            reading,
            Entry {
                surface: surface.to_string(),
                left_id: 1,
                right_id: 1,
                cost: *cost,
                dependent: *dependent,
            },
        );
    }
    let (trie, store) = conversion.build();
    // 이음마다 같은 값이 드는 표 — 문절이 갈리는 근거를 낱말 비용만으로 두어, 이 테스트가
    // 연접 데이터가 아니라 격자 자체를 보게 한다
    let connection = ConnectionBuilder::new(2, 2).build();
    let mut writer = PackWriter::new("ja");
    writer.add_section(SectionKind::Conversion, trie);
    writer.add_section(SectionKind::ConversionEntry, store);
    writer.add_section(SectionKind::Connection, connection);
    writer.finish()
}

/// 사전을 꽂아 두고 이벤트를 흘리는 자리. 팩 바이트를 세션이 들고 있어야 변환표(mmap 뷰)가
/// 이벤트마다 살아 있다.
struct Session {
    bytes: Vec<u8>,
    composer: JapaneseComposer,
    committed: Vec<String>,
    last: ComposerOutput,
}

impl Session {
    fn romaji() -> Self {
        Session::with(JapaneseComposer::romaji())
    }

    fn kana() -> Self {
        Session::with(JapaneseComposer::kana())
    }

    fn with(composer: JapaneseComposer) -> Self {
        Session {
            bytes: pack_bytes(),
            composer,
            committed: Vec::new(),
            last: ComposerOutput::default(),
        }
    }

    fn feed(&mut self, event: ComposerEvent) -> &mut Self {
        let pack = Pack::open(&self.bytes).unwrap();
        let conversion = Conversion::new(pack.conversion().unwrap(), pack.connection(), None);
        let context = EditorContext::unavailable();
        let environment = ComposerEnvironment::new(&context).with_conversion(Some(conversion));
        let output = self.composer.feed(event, &environment);
        if let Some(commit) = &output.commit {
            self.committed.push(commit.surface.clone());
        }
        self.last = output;
        self
    }

    /// 로마자를 한 글자씩 — 12키에서는 친 가나가 그대로 온다.
    fn type_all(&mut self, letters: &str) -> &mut Self {
        for letter in letters.chars() {
            self.feed(ComposerEvent::Key(letter));
        }
        self
    }

    fn convert(&mut self) -> &mut Self {
        self.feed(ComposerEvent::Separator(' '))
    }

    fn composing(&self) -> &str {
        self.last
            .composing
            .as_ref()
            .map(|composing| composing.text.as_str())
            .unwrap_or_default()
    }

    fn focus(&self) -> Option<(usize, usize)> {
        self.last
            .composing
            .as_ref()
            .and_then(|composing| composing.focus)
    }

    fn candidates(&self) -> Vec<&str> {
        match &self.last.suggest {
            SuggestionRequest::Ready { candidates } => {
                candidates.iter().map(|(_, text)| text.as_str()).collect()
            }
            _ => Vec::new(),
        }
    }
}

#[test]
fn 로마자가_가나가_되어_조합_창에_선다() {
    let mut session = Session::romaji();
    session.type_all("kisya");
    assert_eq!(session.composing(), "きしゃ");
    assert_eq!(session.focus(), None, "변환 전에는 주목 도막이 없다");
    assert!(session.committed.is_empty(), "변환 전에는 확정이 없다");
    assert!(session.composer.is_composing());
}

/// 아직 가나가 되지 못한 로마자는 조합 창에 그대로 보인다 — 순정 관례다.
#[test]
fn 미완성_로마자도_조합_창에_보인다() {
    let mut session = Session::romaji();
    session.type_all("kis");
    assert_eq!(session.composing(), "きs");
}

#[test]
fn 스페이스가_변환을_걸고_주목_문절을_밝힌다() {
    let mut session = Session::romaji();
    session.type_all("kisya").convert();
    assert_eq!(session.composing(), "記者", "가장 싼 표기가 먼저 선다");
    assert_eq!(session.focus(), Some((0, 2)));
}

#[test]
fn 스페이스를_거듭_누르면_다음_표기로_간다() {
    let mut session = Session::romaji();
    session.type_all("kisya").convert().convert();
    assert_eq!(session.composing(), "汽車");
    session.convert();
    assert_eq!(session.composing(), "貴社");
}

/// 문절은 형태소가 아니다 — 앞말에 붙는 말(は)은 앞 문절에 얹힌다.
#[test]
fn 붙는_말은_앞_문절에_얹힌다() {
    let mut session = Session::romaji();
    session.type_all("niwaha").convert();
    assert_eq!(session.composing(), "庭は");
    assert_eq!(
        session.focus(),
        Some((0, 2)),
        "「庭は」가 통째로 한 문절이다"
    );
}

#[test]
fn 엔터가_변환_결과를_확정한다() {
    let mut session = Session::romaji();
    session
        .type_all("kisya")
        .convert()
        .feed(ComposerEvent::Separator('\n'));
    assert_eq!(session.committed, ["記者"]);
    assert!(!session.composer.is_composing());
}

#[test]
fn 백스페이스가_변환을_무른다() {
    let mut session = Session::romaji();
    session
        .type_all("kisya")
        .convert()
        .feed(ComposerEvent::Backspace);
    assert!(session.committed.is_empty());
    assert_eq!(session.composing(), "きしゃ", "읽기로 돌아온다");
    assert_eq!(session.focus(), None);
}

/// 후보를 고르면 그 후보의 읽기만큼만 확정되고 남은 읽기는 조합에 남아 다시 변환된다.
#[test]
fn 문절_하나만_골라도_나머지가_이어진다() {
    let mut session = Session::romaji();
    session
        .type_all("niwahagakkou")
        .convert()
        .feed(ComposerEvent::CandidateSelected {
            text: "庭は".to_string(),
            key: "にわは".to_string(),
        });
    assert_eq!(session.committed, ["庭は"]);
    assert_eq!(
        session.composing(),
        "学校",
        "남은 읽기가 곧바로 다시 변환된다"
    );
}

#[test]
fn 변환_중_후보는_주목_문절의_표기들이다() {
    let mut session = Session::romaji();
    session.type_all("kisya").convert();
    let candidates = session.candidates();
    assert_eq!(&candidates[..3], ["記者", "汽車", "貴社"]);
    assert!(
        candidates.contains(&"きしゃ"),
        "가나 그대로도 늘 자리를 갖는다"
    );
}

#[test]
fn 변환_전_후보는_친_읽기가_첫_자리다() {
    let mut session = Session::romaji();
    session.type_all("saku");
    let candidates = session.candidates();
    assert_eq!(candidates[0], "さく", "친 대로 두는 길이 늘 열려 있다");
    assert!(
        candidates.contains(&"桜"),
        "다 치지 않아도 낱말이 미리 선다"
    );
}

#[test]
fn 미변환_스페이스는_전각을_넣는다() {
    let mut session = Session::romaji();
    session.convert();
    assert_eq!(session.committed, ["\u{3000}"]);
}

#[test]
fn 사전에_없는_읽기는_가타카나로_적을_수_있다() {
    let mut session = Session::romaji();
    session.type_all("pasokonn").convert();
    let candidates = session.candidates();
    assert!(
        candidates.contains(&"パソコン"),
        "가타카나가 늘 자리를 갖는다: {candidates:?}"
    );
}

/// 사전에 없는 자리가 있어도 문장 전체의 변환이 사라지지 않는다.
#[test]
fn 모르는_말이_섞여도_아는_말은_변환된다() {
    let mut session = Session::romaji();
    session.type_all("pasokonnhagakkou").convert();
    assert!(
        session.composing().contains("学校"),
        "아는 말은 그대로 변환된다: {}",
        session.composing()
    );
}

/// 12키는 배열이 이미 가나를 내므로 합성기가 옮길 것이 없고, 「小゛゜」가 직전 가나를
/// 갈아 끼운다.
#[test]
fn 십이키의_작은_글자_키가_직전_가나를_갈아_끼운다() {
    let mut session = Session::kana();
    // が는 か에 탁점을 얹어 만들고, っ는 つ를 작은 글자로 갈아 만든다
    session
        .type_all("か")
        .feed(ComposerEvent::Key(FORM_MARKER))
        .type_all("つ")
        .feed(ComposerEvent::Key(FORM_MARKER))
        .type_all("こう");
    assert_eq!(session.composing(), "がっこう");
    session.convert();
    assert_eq!(session.composing(), "学校");
}

#[test]
fn 스냅샷이_변환_상태까지_되살린다() {
    let mut session = Session::romaji();
    session.type_all("kisya").convert().convert();
    let state = session.composer.snapshot();

    let mut restored = Session::romaji();
    restored.composer.restore(state);
    // 되살린 자리에서 스페이스를 한 번 더 누르면 고르던 자리의 다음으로 간다
    restored.convert();
    assert_eq!(restored.composing(), "貴社");
}
