//! 파이프라인의 **순서**를 고정한다. 이 순서가 실행 파일에 있던 동안에는 문서로만
//! 남아 있었고, 단계 하나가 자리를 옮겨도 아무것도 깨지지 않았다.
//!
//! 원천은 로컬 낱말 목록이라 네트워크를 타지 않는다.

use std::path::{Path, PathBuf};

use taza_engine::pack::Pack;
use taza_packbuild::pipeline::{self, BuildOptions};

const RECIPE: &str = r#"
[pack]
language = "tl"
display_name = "Testish"
keycap_label = "T"
composer_skeleton = "latin"
pack_version = 7

[build.lexicon]
encoding = "utf8"
character_set = "latin-lowercase"
max_words = 10
minimum_word_length = 2

[build.language_model]
max_bigrams = 10
minimum_count = 1
"#;

const INVENTORY_SOURCE: &str = r#"
[[sources]]
name = "인벤토리"
version = "1"
license = "CC0"
attribution = "시험용"
file = "inventory.txt"
role = "inventory"
format = "word-list"
optional = false
"#;

const FREQUENCY_SOURCE: &str = r#"
[[sources]]
name = "빈도"
version = "1"
license = "CC0"
attribution = "시험용"
file = "corpus.txt"
role = "frequency"
weight = 0.5
format = "word-list"
minimum_count = 1
optional = false
"#;

fn scratch(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("taza-pipeline-{name}"));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(directory.join("data/languages")).expect("작업 디렉터리");
    directory
}

fn write(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
}

/// 언어 디렉터리 하나를 갖춰 놓는다 — `recipe.toml`과 그 옆의 `sources/`.
fn language(directory: &Path, inventory: &str, corpus: &str) -> PathBuf {
    let language = directory.join("data/languages/testlang");
    let sources = language.join("sources");
    std::fs::create_dir_all(&sources).expect("언어 디렉터리");
    write(&language.join("recipe.toml"), RECIPE);
    write(&sources.join("00-inventory.toml"), INVENTORY_SOURCE);
    write(&sources.join("10-frequency.toml"), FREQUENCY_SOURCE);
    write(&sources.join("inventory.txt"), inventory);
    write(&sources.join("corpus.txt"), corpus);
    language
}

fn options(directory: &Path, skip_archive: bool) -> BuildOptions {
    BuildOptions {
        data_directory: directory.join("data"),
        output_directory: directory.join("out"),
        skip_archive,
        use_cache: false,
    }
}

/// 조달 → 정규화 → 조립 → 배포가 한 번에 돌고, 각 단계가 지나 보낸 수가 보고에 남는다.
#[test]
fn recipe_becomes_a_pack_and_a_catalog_entry() {
    let directory = scratch("full");
    let language = language(
        &directory,
        "keyboard\nlanguage\nkey\na\n",
        "keyboard\t40\nlanguage\t9\nkeyboard language\t3\n",
    );

    let options = options(&directory, false);
    let outcome = pipeline::build(&language, &options).expect("빌드");
    let report = &outcome.report;

    // 조달: 두 원천이 모두 지나갔다
    assert_eq!(report.sources.len(), 2);
    assert_eq!(report.sources[0].name, "인벤토리");
    assert_eq!(report.sources[0].attested, 4);
    assert!(!report.sources[0].from_cache);

    // 정규화: 표제어 집합은 인벤토리가 정하고, 한 글자는 걸러진다
    assert_eq!(report.lexicon.inventory_size, 4);
    // 한 글자 낱말은 `minimum_word_length`에 걸린다
    assert_eq!(report.lexicon.dropped_by_filter, 1);
    assert_eq!(report.word_count, 3);

    // 조립: 팩이 실제로 열리고 방금 정규화한 표제어가 조회된다
    assert!(report.pack.path.exists());
    let bytes = std::fs::read(&report.pack.path).expect("팩 읽기");
    let pack = Pack::open(&bytes).expect("팩 열기");
    let lexicon = pack.lexicon().expect("lexicon 섹션");
    assert!(lexicon.contains("keyboard"));
    assert!(!lexicon.contains("a"));

    // 배포: 아카이브가 나오고 카탈로그 항목이 팩의 해시를 그대로 가리킨다
    let archive = report.archive.as_ref().expect("아카이브");
    assert!(archive.path.exists());
    assert_eq!(outcome.entry.name, "testlang");
    assert_eq!(outcome.entry.pack_version, 7);
    assert_eq!(outcome.entry.pack_size, bytes.len() as u64);
    assert_eq!(outcome.entry.archive_size, archive.bytes);

    // 사람이 눈으로 훑는 중간 표들도 같은 판에서 나온다
    let table =
        std::fs::read_to_string(report.build_directory.join("testlang-words.tsv")).expect("점수표");
    assert_eq!(table.lines().count(), report.word_count);

    let catalog = pipeline::publish(&options, outcome.entry).expect("카탈로그");
    let notice = pipeline::write_notice(&options, &catalog).expect("고지");
    assert!(pipeline::catalog_path(&options).exists());
    let notice_text = std::fs::read_to_string(&notice).expect("고지 읽기");
    assert!(notice_text.contains("시험용"));

    let _ = std::fs::remove_dir_all(&directory);
}

/// `--skip-archive`는 팩까지만 굽는다 — 카탈로그 항목의 아카이브 자리는 비어 있다.
#[test]
fn skipping_the_archive_still_produces_a_pack() {
    let directory = scratch("skip");
    let language = language(&directory, "keyboard\nlanguage\n", "keyboard\t40\n");

    let options = options(&directory, true);
    let outcome = pipeline::build(&language, &options).expect("빌드");
    assert!(outcome.report.archive.is_none());
    assert!(outcome.report.pack.path.exists());
    assert_eq!(outcome.entry.archive_size, 0);
    assert!(outcome.entry.archive_sha256.is_empty());

    let _ = std::fs::remove_dir_all(&directory);
}

/// 이름을 주지 않으면 언어 디렉터리 전부를, 사전순으로. `recipe.toml`이 없는 디렉터리는
/// 언어가 아니다.
#[test]
fn language_directories_default_to_every_language_in_order() {
    let directory = scratch("paths");
    let languages = directory.join("data/languages");
    for name in ["zulu", "alpha", "mike"] {
        std::fs::create_dir_all(languages.join(name)).expect("언어 디렉터리");
        write(&languages.join(name).join("recipe.toml"), "");
    }
    std::fs::create_dir_all(languages.join("notes")).expect("언어가 아닌 디렉터리");

    let options = options(&directory, false);
    let all = pipeline::language_directories(&options, &[]).expect("전부");
    let names: Vec<String> = all
        .iter()
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["alpha", "mike", "zulu"]);

    let chosen = pipeline::language_directories(&options, &["mike".to_string()]).expect("고른 것");
    assert_eq!(chosen, vec![languages.join("mike")]);

    assert!(pipeline::language_directories(&options, &["없는것".to_string()]).is_err());

    let _ = std::fs::remove_dir_all(&directory);
}

/// 변환하는 언어의 팩은 **뼈대가 다르다** — lexicon 대신 읽기 trie와 표기 곳간이 서고,
/// 조회 키는 가나 정규형이다. 이 테스트가 고정하는 것은 사전 배포본 하나가 그 세 섹션으로
/// 옮겨 앉는 길이다.
mod japanese {
    use super::*;

    const RECIPE: &str = r#"
[pack]
language = "ja"
display_name = "日本語"
keycap_label = "あ"
composer_skeleton = "japanese-romaji"
pack_version = 1

[pack.script]
word_separated = false

[build.lexicon]
encoding = "kana"
character_set = "kana"
max_words = 0
minimum_word_length = 1
"#;

    const SOURCE: &str = r#"
[[sources]]
name = "사전"
version = "1"
license = "BSD-3-Clause"
attribution = "시험용"
file = "mozc.tar.gz"
role = "inventory"
format = "mozc-dictionary"
dictionary_files = ["dictionary00.txt"]
dependent_tags = ["助詞"]
optional = false
"#;

    /// `읽기 · 좌id · 우id · 비용 · 표기` — 읽기가 이미 히라가나인 것이 분석 사전과 다르다.
    const DICTIONARY: &str = "\
きしゃ\t1852\t1852\t5000\t汽車
きしゃ\t1852\t1852\t4000\t記者
にわ\t1852\t1852\t3000\t庭
は\t368\t368\t100\tは
";

    /// 접미·어미도 같은 문맥 id 공간을 쓰므로 그대로 합쳐진다.
    const SUFFIX: &str = "です\t368\t368\t200\tです\n";

    /// `id 품사,세분류…`
    const IDS: &str =
        "0 BOS/EOS,*,*,*,*,*,*\n368 助詞,格助詞,一般,*,*,*,*\n1852 名詞,一般,*,*,*,*,*\n";

    /// 읽기 하나에 한자 여럿 — 붙여 쓴 차례가 곧 우선순위다.
    const SINGLE_KANJI: &str = "あい\t愛藍\n";

    /// 첫 줄이 축의 크기, 그 뒤로 크기² 개의 값. 여기서는 3×3이다.
    const CONNECTION: &str = "3\n0\n0\n0\n0\n0\n-500\n0\n300\n0\n";

    fn archive(path: &Path) {
        let file = std::fs::File::create(path).expect("사전 파일");
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
        let mut archive = tar::Builder::new(encoder);
        for (name, text) in [
            ("mozc/src/data/dictionary_oss/dictionary00.txt", DICTIONARY),
            ("mozc/src/data/dictionary_oss/suffix.txt", SUFFIX),
            ("mozc/src/data/dictionary_oss/id.def", IDS),
            (
                "mozc/src/data/dictionary_oss/connection_single_column.txt",
                CONNECTION,
            ),
            ("mozc/src/data/single_kanji/single_kanji.tsv", SINGLE_KANJI),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_size(text.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            archive
                .append_data(&mut header, name, text.as_bytes())
                .expect("항목 쓰기");
        }
        archive
            .into_inner()
            .expect("압축")
            .finish()
            .expect("마무리");
    }

    #[test]
    fn dictionary_becomes_conversion_sections() {
        let directory = scratch("japanese");
        let language = directory.join("data/languages/japanese");
        let sources = language.join("sources");
        std::fs::create_dir_all(&sources).expect("언어 디렉터리");
        write(&language.join("recipe.toml"), RECIPE);
        write(&sources.join("00-dictionary.toml"), SOURCE);
        archive(&sources.join("mozc.tar.gz"));

        let outcome = pipeline::build(&language, &options(&directory, true)).expect("빌드");
        let bytes = std::fs::read(&outcome.report.pack.path).expect("팩 읽기");
        let pack = Pack::open(&bytes).expect("팩 열기");

        // 표제어 목록이 아니라 변환표가 팩의 알맹이다
        assert!(
            pack.lexicon().is_none(),
            "변환하는 언어는 lexicon을 싣지 않는다"
        );
        let table = pack.conversion().expect("변환표");

        let surfaces: Vec<&str> = table
            .lookup("きしゃ")
            .expect("きしゃ")
            .iter()
            .map(|entry| entry.surface)
            .collect();
        assert_eq!(surfaces, ["記者", "汽車"], "싼 표기가 먼저 선다");

        // 조사는 앞말에 붙는 말로 실려야 문절이 갈린다
        assert!(
            table
                .lookup("は")
                .expect("は")
                .best()
                .expect("표기")
                .dependent
        );
        // 접미사는 주 어휘와 같은 표에 함께 선다
        assert!(table.lookup("です").is_some());
        // 단漢字는 사전이 적어 둔 차례대로, 낱말보다 비싸게
        let kanji: Vec<&str> = table
            .lookup("あい")
            .expect("あい")
            .iter()
            .map(|entry| entry.surface)
            .collect();
        assert_eq!(kanji, ["愛", "藍"]);

        // 연접 표가 그대로 옮겨 앉았다
        let matrix = pack.connection().expect("연접 표");
        assert_eq!(matrix.cost(1, 2), -500);
        assert_eq!(matrix.cost(2, 1), 300);
    }
}
