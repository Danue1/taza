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
//!
//! 단계의 순서는 `taza_toolchain::pipeline`이 소유한다. 이 파일이 하는 일은 인자를 읽고
//! 각 단계가 낸 수를 사람이 읽는 말로 옮기는 것뿐이다.

use std::path::PathBuf;
use std::process::ExitCode;

use taza_toolchain::pipeline::{self, BuildOptions, BuildReport};

const USAGE: &str = "사용법: taza-packs [언어…] [--data <디렉터리>] [--skip-archive] [--no-cache]";

fn parse_options(arguments: &[String]) -> Result<(Vec<String>, BuildOptions), String> {
    let mut names = Vec::new();
    let mut data_directory = PathBuf::from("data");
    let mut skip_archive = false;
    let mut use_cache = true;
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
            "--no-cache" => use_cache = false,
            other if other.starts_with("--") => return Err(USAGE.to_string()),
            other => names.push(other.to_string()),
        }
    }
    Ok((
        names,
        BuildOptions {
            data_directory,
            skip_archive,
            use_cache,
        },
    ))
}

fn report(report: &BuildReport) {
    println!("[{}] {} 원천", report.name, report.sources.len());
    for source in &report.sources {
        println!(
            "  {} ({}): 보증 {} / 관측 {} / 이웃 짝 {}{}",
            source.name,
            source.license,
            source.attested,
            source.observed,
            source.bigrams,
            if source.from_cache { " (캐시)" } else { "" }
        );
    }
    let lexicon = &report.lexicon;
    println!(
        "  정규화: 인벤토리 {} / 코퍼스 관측 {} / 활용형 수용 {} / 승격 {} (기각 {}) / 필터 제외 {} / 예산 제외 {} → 표제어 {}",
        lexicon.inventory_size,
        lexicon.observed_in_corpus,
        lexicon.accepted_inflections,
        lexicon.promoted_words.len(),
        lexicon.rejected_candidates,
        lexicon.dropped_by_filter,
        lexicon.dropped_by_budget,
        report.word_count
    );
    let language_model = &report.language_model;
    println!(
        "  언어모델: 관측 {} / 표제어 밖 {} / 이득 없음 {} / 예산 제외 {} → bigram {}",
        language_model.observed,
        language_model.dropped_outside_lexicon,
        language_model.dropped_without_lift,
        language_model.dropped_by_budget,
        report.bigram_count
    );
    let pack = &report.pack;
    println!(
        "  팩: 표제어 {} / lexicon {} KB / 언어모델 {} KB / 곁들임 {}낱말 {} KB (목록 {}) / 전체 {} KB → {}",
        pack.word_count,
        pack.lexicon_bytes / 1024,
        pack.language_model_bytes / 1024,
        pack.annotation_key_count,
        pack.annotation_bytes / 1024,
        pack.catalog_item_count,
        pack.total_bytes / 1024,
        pack.path.display()
    );
    match &report.archive {
        None => println!("  아카이브 생략 (--skip-archive)"),
        Some(archive) => println!(
            "  아카이브: {} KB ({:.0}%) → {}",
            archive.bytes / 1024,
            100.0 * archive.bytes as f64 / pack.total_bytes as f64,
            archive.path.display()
        ),
    }
}

fn run(names: &[String], options: &BuildOptions) -> Result<(), String> {
    let paths = pipeline::recipe_paths(&options.data_directory.join("recipes"), names)?;
    let mut catalog = None;
    for path in &paths {
        let outcome = pipeline::build(path, options)?;
        report(&outcome.report);
        catalog = Some(pipeline::publish(options, outcome.entry)?);
    }
    if let Some(catalog) = catalog {
        let notice_path = pipeline::write_notice(options, &catalog)?;
        println!(
            "카탈로그 {} / 고지 {}",
            pipeline::catalog_path(options).display(),
            notice_path.display()
        );
    }
    Ok(())
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let (names, options) = match parse_options(&arguments) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };
    match run(&names, &options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("실패: {message}");
            ExitCode::FAILURE
        }
    }
}
