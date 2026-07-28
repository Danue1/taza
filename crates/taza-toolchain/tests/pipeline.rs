//! 파이프라인의 **순서**를 고정한다. 이 순서가 실행 파일에 있던 동안에는 문서로만
//! 남아 있었고, 단계 하나가 자리를 옮겨도 아무것도 깨지지 않았다.
//!
//! 원천은 로컬 낱말 목록이라 네트워크를 타지 않는다.

use std::path::{Path, PathBuf};

use taza_engine::pack::Pack;
use taza_toolchain::pipeline::{self, BuildOptions};

const RECIPE: &str = r#"
name = "testlang"
language = "tl"
display_name = "Testish"
keycap_label = "T"
composer_skeleton = "latin"
pack_version = 7

[lexicon]
encoding = "utf8"
character_set = "latin-lowercase"
max_words = 10
minimum_word_length = 2

[language_model]
max_bigrams = 10
minimum_count = 1

[[sources]]
name = "인벤토리"
version = "1"
license = "CC0"
attribution = "시험용"
file = "inventory.txt"
role = "inventory"
format = "word-list"
optional = false

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
    std::fs::create_dir_all(directory.join("recipes")).expect("작업 디렉터리");
    directory
}

fn write(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
}

/// 조달 → 정규화 → 조립 → 배포가 한 번에 돌고, 각 단계가 지나 보낸 수가 보고에 남는다.
#[test]
fn recipe_becomes_a_pack_and_a_catalog_entry() {
    let directory = scratch("full");
    let recipes = directory.join("recipes");
    write(&recipes.join("testlang.toml"), RECIPE);
    write(
        &recipes.join("inventory.txt"),
        "keyboard\nlanguage\nkey\na\n",
    );
    write(
        &recipes.join("corpus.txt"),
        "keyboard\t40\nlanguage\t9\nkeyboard language\t3\n",
    );

    let options = BuildOptions {
        data_directory: directory.clone(),
        skip_archive: false,
        use_cache: false,
    };
    let outcome = pipeline::build(&recipes.join("testlang.toml"), &options).expect("빌드");
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
    let recipes = directory.join("recipes");
    write(&recipes.join("testlang.toml"), RECIPE);
    write(&recipes.join("inventory.txt"), "keyboard\nlanguage\n");
    write(&recipes.join("corpus.txt"), "keyboard\t40\n");

    let options = BuildOptions {
        data_directory: directory.clone(),
        skip_archive: true,
        use_cache: false,
    };
    let outcome = pipeline::build(&recipes.join("testlang.toml"), &options).expect("빌드");
    assert!(outcome.report.archive.is_none());
    assert!(outcome.report.pack.path.exists());
    assert_eq!(outcome.entry.archive_size, 0);
    assert!(outcome.entry.archive_sha256.is_empty());

    let _ = std::fs::remove_dir_all(&directory);
}

/// 이름을 주지 않으면 디렉터리에 있는 레시피 전부를, 사전순으로.
#[test]
fn recipe_paths_default_to_every_recipe_in_order() {
    let directory = scratch("paths");
    let recipes = directory.join("recipes");
    for name in ["zulu", "alpha", "mike"] {
        write(&recipes.join(format!("{name}.toml")), "");
    }
    write(&recipes.join("notes.txt"), "");

    let all = pipeline::recipe_paths(&recipes, &[]).expect("전부");
    let names: Vec<String> = all
        .iter()
        .map(|path| path.file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["alpha", "mike", "zulu"]);

    let chosen = pipeline::recipe_paths(&recipes, &["mike".to_string()]).expect("고른 것");
    assert_eq!(chosen, vec![recipes.join("mike.toml")]);

    assert!(pipeline::recipe_paths(&recipes, &["없는것".to_string()]).is_err());

    let _ = std::fs::remove_dir_all(&directory);
}
