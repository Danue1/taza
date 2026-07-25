//! 언어팩 파이프라인의 입구 — 레시피 하나로 조달부터 배포 산출물까지 한 번에 돈다.
//!
//! ```text
//! taza-packs [언어…] [--data <디렉터리>] [--skip-archive]
//! ```
//! 언어를 주지 않으면 `<데이터>/recipes/*.toml` 전부를 만든다. 산출물:
//! `<데이터>/build/<이름>-words.tsv`(중간 점수표, 사람이 들여다보는 용도),
//! `<데이터>/packs/<이름>.tazapack`, 같은 자리에 `.zst` 아카이브와 `catalog.json`,
//! 그리고 `<데이터>/sources/NOTICE.md`.
//!
//! 원천 파일은 `<데이터>/cache`에 sha256과 함께 남으므로 다시 실행해도 네트워크를
//! 타지 않는다. 원천이 조용히 바뀌면 해시 검증에서 멈춘다.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use taza_toolchain::distribute::{self, CatalogEntry};
use taza_toolchain::fetch::hex_digest;
use taza_toolchain::normalize::{SourceSignal, normalize, normalize_bigrams};
use taza_toolchain::recipe::Recipe;
use taza_toolchain::{assemble, extract, fetch};

const USAGE: &str = "사용법: taza-packs [언어…] [--data <디렉터리>] [--skip-archive]";

struct Options {
    names: Vec<String>,
    data_directory: PathBuf,
    skip_archive: bool,
}

fn parse_options(arguments: &[String]) -> Result<Options, String> {
    let mut names = Vec::new();
    let mut data_directory = PathBuf::from("data");
    let mut skip_archive = false;
    let mut iterator = arguments.iter();
    while let Some(argument) = iterator.next() {
        match argument.as_str() {
            "--data" => {
                data_directory = iterator
                    .next()
                    .map(PathBuf::from)
                    .ok_or_else(|| USAGE.to_string())?;
            }
            "--skip-archive" => skip_archive = true,
            other if other.starts_with("--") => return Err(USAGE.to_string()),
            other => names.push(other.to_string()),
        }
    }
    Ok(Options {
        names,
        data_directory,
        skip_archive,
    })
}

fn recipe_paths(directory: &Path, names: &[String]) -> Result<Vec<PathBuf>, String> {
    if !names.is_empty() {
        return names
            .iter()
            .map(|name| {
                let path = directory.join(format!("{name}.toml"));
                if path.exists() {
                    Ok(path)
                } else {
                    Err(format!("레시피가 없음: {}", path.display()))
                }
            })
            .collect();
    }
    let mut paths: Vec<PathBuf> = std::fs::read_dir(directory)
        .map_err(|error| format!("{} 읽기 실패: {error}", directory.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "toml")
        })
        .collect();
    paths.sort();
    if paths.is_empty() {
        return Err(format!("{}: 레시피가 없음", directory.display()));
    }
    Ok(paths)
}

fn build(recipe_path: &Path, options: &Options) -> Result<CatalogEntry, String> {
    let recipe = Recipe::load(recipe_path)?;
    println!("[{}] {} 원천", recipe.name, recipe.sources.len());

    let cache = options.data_directory.join("cache");
    let mut extracted = Vec::new();
    for source in &recipe.sources {
        let path = fetch::fetch(&source.url, &source.sha256, &cache)?;
        let signal = extract::extract(&source.extraction, &path, &recipe.language)?;
        println!(
            "  {} ({}): 낱말 {} / 이웃 짝 {}",
            source.name,
            source.license,
            signal.words.len(),
            signal.bigrams.len()
        );
        extracted.push((source, signal));
    }

    let signals: Vec<SourceSignal<'_>> = extracted
        .iter()
        .map(|(source, signal)| SourceSignal {
            role: source.role,
            weight: source.weight,
            entries: &signal.words,
            bigrams: &signal.bigrams,
        })
        .collect();
    let (words, report) = normalize(&signals, &recipe.lexicon);
    println!(
        "  정규화: 후보 {} / 코퍼스 관측 {} / 필터 제외 {} / 예산 제외 {} → 표제어 {}",
        report.inventory_size,
        report.observed_in_corpus,
        report.dropped_by_filter,
        report.dropped_by_budget,
        words.len()
    );
    let (bigrams, bigram_report) = normalize_bigrams(&signals, &words, &recipe.language_model);
    println!(
        "  언어모델: 관측 {} / 표제어 밖 {} / 이득 없음 {} / 예산 제외 {} → bigram {}",
        bigram_report.observed,
        bigram_report.dropped_outside_lexicon,
        bigram_report.dropped_without_lift,
        bigram_report.dropped_by_budget,
        bigrams.len()
    );

    let build_directory = options.data_directory.join("build");
    std::fs::create_dir_all(&build_directory)
        .map_err(|error| format!("{} 만들기 실패: {error}", build_directory.display()))?;
    let table_path = build_directory.join(format!("{}-words.tsv", recipe.name));
    let table: String = words
        .iter()
        .map(|(word, score)| format!("{word}\t{score}\n"))
        .collect();
    std::fs::write(&table_path, table)
        .map_err(|error| format!("{} 쓰기 실패: {error}", table_path.display()))?;

    // 예산에 밀려 빠진 낱말 — 오교정률 평가가 "사전에 없지만 올바른 말"로 쓴다
    let absent_path = build_directory.join(format!("{}-absent.tsv", recipe.name));
    let absent: String = report
        .absent_words
        .iter()
        .map(|word| format!("{word}\n"))
        .collect();
    std::fs::write(&absent_path, absent)
        .map_err(|error| format!("{} 쓰기 실패: {error}", absent_path.display()))?;

    let layout_text = recipe
        .layout
        .as_ref()
        .map(|path| {
            std::fs::read_to_string(path)
                .map_err(|error| format!("{} 읽기 실패: {error}", path.display()))
        })
        .transpose()?;
    let assembled = assemble::assemble(&recipe, &words, &bigrams, layout_text.as_deref())?;
    let packs_directory = options.data_directory.join("packs");
    std::fs::create_dir_all(&packs_directory)
        .map_err(|error| format!("{} 만들기 실패: {error}", packs_directory.display()))?;
    let pack_path = packs_directory.join(format!("{}.tazapack", recipe.name));
    std::fs::write(&pack_path, &assembled.bytes)
        .map_err(|error| format!("{} 쓰기 실패: {error}", pack_path.display()))?;
    println!(
        "  팩: 표제어 {} / lexicon {} KB / 언어모델 {} KB / 전체 {} KB → {}",
        assembled.word_count,
        assembled.lexicon_bytes / 1024,
        assembled.language_model_bytes / 1024,
        assembled.bytes.len() / 1024,
        pack_path.display()
    );

    let archive_file = format!("{}.tazapack.zst", recipe.name);
    let mut archive_size = 0u64;
    let mut archive_sha256 = String::new();
    if options.skip_archive {
        println!("  아카이브 생략 (--skip-archive)");
    } else {
        let archive = distribute::compress(&assembled.bytes)?;
        let archive_path = packs_directory.join(&archive_file);
        std::fs::write(&archive_path, &archive.bytes)
            .map_err(|error| format!("{} 쓰기 실패: {error}", archive_path.display()))?;
        archive_size = archive.bytes.len() as u64;
        archive_sha256 = archive.sha256;
        println!(
            "  아카이브: {} KB ({:.0}%) → {}",
            archive_size / 1024,
            100.0 * archive_size as f64 / assembled.bytes.len() as f64,
            archive_path.display()
        );
    }

    Ok(CatalogEntry {
        name: recipe.name.clone(),
        language: recipe.language.clone(),
        pack_version: recipe.pack_version,
        word_count: assembled.word_count,
        archive_file,
        archive_size,
        archive_sha256,
        pack_size: assembled.bytes.len() as u64,
        pack_sha256: hex_digest(&assembled.bytes),
        sources: assemble::source_lines(&recipe),
        attribution: assemble::attribution(&recipe),
    })
}

fn run(options: &Options) -> Result<(), String> {
    let paths = recipe_paths(&options.data_directory.join("recipes"), &options.names)?;
    let catalog_path = options.data_directory.join("packs/catalog.json");
    let mut catalog = None;
    for path in &paths {
        let entry = build(path, options)?;
        catalog = Some(distribute::update_catalog(&catalog_path, entry)?);
    }
    if let Some(catalog) = catalog {
        let notice_path = options.data_directory.join("sources/NOTICE.md");
        if let Some(parent) = notice_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("{} 만들기 실패: {error}", parent.display()))?;
        }
        distribute::write_notice(&notice_path, &catalog)?;
        println!(
            "카탈로그 {} / 고지 {}",
            catalog_path.display(),
            notice_path.display()
        );
    }
    Ok(())
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let options = match parse_options(&arguments) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };
    match run(&options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("실패: {message}");
            ExitCode::FAILURE
        }
    }
}
