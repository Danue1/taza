//! 단계를 잇는 자리 — 레시피 하나가 배포 산출물이 되기까지의 **순서**를 소유한다.
//!
//! 조달(`source`) → 정규화(`normalize`) → 조립(`assemble`) → 배포(`distribute`). 이 순서가
//! 실행 파일에 있으면 문서로만 남고 회귀 테스트로 고정할 수 없어서 여기에 둔다.
//!
//! 사람에게 보이는 말은 짓지 않는다. 각 단계가 지나 보낸 수를 `BuildReport`에 담아 돌려
//! 주고, 그것을 어떻게 적을지는 부르는 쪽(`taza-packs`)이 정한다.

use std::path::{Path, PathBuf};

use taza_engine::contract::EmojiCategory;

use crate::assemble::{self, AssembledPack};
use crate::distribute::{self, Catalog, CatalogEntry};
use crate::normalize::{BigramReport, NormalizeReport, SourceSignal, normalize, normalize_bigrams};
use crate::parse::Annotation;
use crate::recipe::{Recipe, Source};
use crate::source::{self, Prepared, acquire::hex_digest};

pub struct BuildOptions {
    /// 레시피·캐시·산출물이 모두 이 아래에 산다
    pub data_directory: PathBuf,
    /// 전송용 압축 아카이브를 만들지 않는다 — 팩만 확인하고 싶을 때
    pub skip_archive: bool,
    /// 추출 결과 캐시를 쓴다. 끄면 파서를 고치고 판 번호를 올리는 것을 잊었을 때의 탈출구다.
    pub use_cache: bool,
}

/// 원천 하나가 실제로 무엇을 냈는가.
pub struct SourceReport {
    pub name: String,
    pub license: String,
    pub attested: usize,
    pub observed: usize,
    pub bigrams: usize,
    pub from_cache: bool,
}

/// 구운 팩의 생김새 — 바이트는 이미 디스크에 있으므로 수치와 자리만 남긴다.
pub struct PackReport {
    pub path: PathBuf,
    pub word_count: usize,
    pub total_bytes: usize,
    pub lexicon_bytes: usize,
    pub language_model_bytes: usize,
    pub annotation_key_count: usize,
    pub annotation_bytes: usize,
    pub catalog_item_count: usize,
}

pub struct ArchiveReport {
    pub path: PathBuf,
    pub bytes: u64,
}

/// 한 판 도는 동안 각 단계가 지나 보낸 것.
pub struct BuildReport {
    pub name: String,
    pub sources: Vec<SourceReport>,
    pub lexicon: NormalizeReport,
    pub word_count: usize,
    pub language_model: BigramReport,
    pub bigram_count: usize,
    pub pack: PackReport,
    /// `skip_archive`면 None
    pub archive: Option<ArchiveReport>,
    /// 사람이 들여다보는 중간 표들이 놓인 자리
    pub build_directory: PathBuf,
}

pub struct BuildOutcome {
    pub entry: CatalogEntry,
    pub report: BuildReport,
}

/// 만들 레시피들. 이름을 주지 않으면 디렉터리에 있는 것 전부를 사전순으로.
pub fn recipe_paths(directory: &Path, names: &[String]) -> Result<Vec<PathBuf>, String> {
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

/// 레시피 하나를 팩과 배포 산출물까지 굽는다.
pub fn build(recipe_path: &Path, options: &BuildOptions) -> Result<BuildOutcome, String> {
    let recipe = Recipe::load(recipe_path)?;
    let extracted = extract(&recipe, options)?;
    let sources: Vec<SourceReport> = extracted
        .iter()
        .map(|(source, signal, from_cache)| SourceReport {
            name: source.name.clone(),
            license: source.license.clone(),
            attested: signal.attested.len(),
            observed: signal.observed.len(),
            bigrams: signal.bigrams.len(),
            from_cache: *from_cache,
        })
        .collect();

    let signals: Vec<SourceSignal<'_>> = extracted
        .iter()
        .map(|(source, signal, _)| SourceSignal {
            role: source.role,
            weight: source.weight,
            attested: &signal.attested,
            observed: &signal.observed,
            bigrams: &signal.bigrams,
            stems: &signal.stems,
            affixes: &signal.affixes,
        })
        .collect();
    let (words, lexicon_report) = normalize(&signals, &recipe.lexicon);
    let (bigrams, bigram_report) = normalize_bigrams(&signals, &words, &recipe.language_model);

    let build_directory = options.data_directory.join("build");
    write_tables(
        &build_directory,
        &recipe,
        &words,
        &lexicon_report,
        &extracted,
    )?;

    let used: Vec<&Source> = extracted.iter().map(|(source, _, _)| *source).collect();
    let assembled = assemble_pack(&recipe, &extracted, &used, &words, &bigrams)?;

    let packs_directory = options.data_directory.join("packs");
    create_directory(&packs_directory)?;
    let pack_path = packs_directory.join(format!("{}.tazapack", recipe.name));
    write(&pack_path, &assembled.bytes)?;

    let archive_file = format!("{}.tazapack.zst", recipe.name);
    let archive = match options.skip_archive {
        true => None,
        false => {
            let compressed = distribute::compress(&assembled.bytes)?;
            let archive_path = packs_directory.join(&archive_file);
            write(&archive_path, &compressed.bytes)?;
            Some((
                ArchiveReport {
                    path: archive_path,
                    bytes: compressed.bytes.len() as u64,
                },
                compressed.sha256,
            ))
        }
    };

    let entry = CatalogEntry {
        name: recipe.name.clone(),
        language: recipe.language.clone(),
        pack_version: recipe.pack_version,
        word_count: assembled.word_count,
        archive_file,
        archive_size: archive
            .as_ref()
            .map(|(report, _)| report.bytes)
            .unwrap_or(0),
        archive_sha256: archive
            .as_ref()
            .map(|(_, sha256)| sha256.clone())
            .unwrap_or_default(),
        pack_size: assembled.bytes.len() as u64,
        pack_sha256: hex_digest(&assembled.bytes),
        sources: assemble::source_lines(&used),
    };
    let report = BuildReport {
        name: recipe.name.clone(),
        sources,
        lexicon: lexicon_report,
        word_count: words.len(),
        language_model: bigram_report,
        bigram_count: bigrams.len(),
        pack: PackReport {
            path: pack_path,
            word_count: assembled.word_count,
            total_bytes: assembled.bytes.len(),
            lexicon_bytes: assembled.lexicon_bytes,
            language_model_bytes: assembled.language_model_bytes,
            annotation_key_count: assembled.annotation_key_count,
            annotation_bytes: assembled.annotation_bytes,
            catalog_item_count: assembled.catalog_item_count,
        },
        archive: archive.map(|(report, _)| report),
        build_directory,
    };
    Ok(BuildOutcome { entry, report })
}

/// 배포 카탈로그에 팩 하나를 올린다. 레시피 하나를 구울 때마다 부른다 — 여러 언어를
/// 만드는 도중 하나가 실패해도 앞서 구워진 것들은 카탈로그에 남아야 한다.
pub fn publish(options: &BuildOptions, entry: CatalogEntry) -> Result<Catalog, String> {
    distribute::update_catalog(&catalog_path(options), entry)
}

pub fn catalog_path(options: &BuildOptions) -> PathBuf {
    options.data_directory.join("packs/catalog.json")
}

/// 기기에 나가는 원천의 고지 — 카탈로그가 곧 무엇이 나가는지의 목록이다.
pub fn write_notice(options: &BuildOptions, catalog: &Catalog) -> Result<PathBuf, String> {
    let notice_path = options.data_directory.join("sources/NOTICE.md");
    if let Some(parent) = notice_path.parent() {
        create_directory(parent)?;
    }
    distribute::write_notice(&notice_path, catalog)?;
    Ok(notice_path)
}

/// 원천에서 뽑아 온 신호 — 어느 원천이 냈는지와 캐시를 탔는지를 함께 쥔다.
type Extraction<'recipe> = (&'recipe Source, crate::parse::Signal, bool);

fn extract<'recipe>(
    recipe: &'recipe Recipe,
    options: &BuildOptions,
) -> Result<Vec<Extraction<'recipe>>, String> {
    let cache = options.data_directory.join("cache");
    // 원천들은 서로를 보지 않으므로 함께 훑는다 — 파이프라인 시간의 거의 전부가 이
    // 단계이고, 큰 말뭉치 하나가 도는 동안 나머지 실이 놀 이유가 없다.
    let prepared: Vec<Result<Prepared, String>> = std::thread::scope(|scope| {
        let handles: Vec<_> = recipe
            .sources
            .iter()
            .map(|source| {
                let (cache, language) = (&cache, &recipe.language);
                scope.spawn(move || source::prepare(source, language, cache, options.use_cache))
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|_| Err("원천 처리가 끝나지 못했음".to_string()))
            })
            .collect()
    });
    let mut extracted = Vec::new();
    for (source, prepared) in recipe.sources.iter().zip(prepared) {
        let Prepared::Extracted { signal, from_cache } = prepared? else {
            continue;
        };
        extracted.push((source, signal, from_cache));
    }
    Ok(extracted)
}

fn assemble_pack(
    recipe: &Recipe,
    extracted: &[Extraction<'_>],
    used: &[&Source],
    words: &[(String, u32)],
    bigrams: &[(String, String, u32)],
) -> Result<AssembledPack, String> {
    // 곁들일 것은 원천이 표시 형태로 낸다 — 조회 키로 옮기는 것은 조립 단계의 몫이다
    let mut annotations: Vec<Annotation> = extracted
        .iter()
        .flat_map(|(_, signal, _)| signal.annotations.iter().cloned())
        .collect();
    annotations.sort_by(|left, right| {
        (&left.word, left.group.tag(), &left.text).cmp(&(
            &right.word,
            right.group.tag(),
            &right.text,
        ))
    });
    annotations.dedup();
    // 이모지 차례는 원천에 실린 순서가 곧 의미라 정렬하지 않는다
    let emoji_order: Vec<(EmojiCategory, String)> = extracted
        .iter()
        .flat_map(|(_, signal, _)| signal.emoji_order.iter().cloned())
        .collect();
    // 원천이 밝힌 접사는 팩에 실려 코어가 학습 어휘의 결합형을 제안하는 데 쓰인다
    let mut affixes: Vec<String> = extracted
        .iter()
        .flat_map(|(_, signal, _)| signal.affixes.iter().cloned())
        .collect();
    affixes.sort_unstable();
    affixes.dedup();
    assemble::assemble(assemble::PackInputs {
        recipe,
        sources: used,
        words,
        bigrams,
        annotations: &annotations,
        emoji_order: &emoji_order,
        affixes: &affixes,
    })
}

/// 사람이 눈으로 훑는 중간 표들. 팩에는 들어가지 않지만 문턱을 조일지 풀지를
/// 판단하려면 무엇이 들어오고 무엇이 잘렸는지 목록으로 보여야 한다.
fn write_tables(
    build_directory: &Path,
    recipe: &Recipe,
    words: &[(String, u32)],
    report: &NormalizeReport,
    extracted: &[Extraction<'_>],
) -> Result<(), String> {
    create_directory(build_directory)?;
    let table: String = words
        .iter()
        .map(|(word, score)| format!("{word}\t{score}\n"))
        .collect();
    write(
        &build_directory.join(format!("{}-words.tsv", recipe.name)),
        table.as_bytes(),
    )?;

    // 예산에 밀려 빠진 낱말 — 오교정률 평가가 "사전에 없지만 올바른 말"로 쓴다
    let absent: String = report
        .absent_words
        .iter()
        .map(|word| format!("{word}\n"))
        .collect();
    write(
        &build_directory.join(format!("{}-absent.tsv", recipe.name)),
        absent.as_bytes(),
    )?;

    // 사전에 없다가 코퍼스 증거로 들어온 낱말 — 문턱이 느슨한지 빡빡한지는 이 목록을
    // 눈으로 훑어야 안다(외래어·신어가 들어왔는가, 인명·오타가 섞였는가).
    let promoted: String = report
        .promoted_words
        .iter()
        .map(|word| format!("{word}\n"))
        .collect();
    write(
        &build_directory.join(format!("{}-promoted.tsv", recipe.name)),
        promoted.as_bytes(),
    )?;

    // 원천별 기여 — 새 말뭉치를 꽂았을 때 그것이 사전을 실제로 바꿨는지는 전체 표제어
    // 수만 봐서는 알 수 없다. 큰 코퍼스를 더해도 이미 있던 낱말만 다시 보고 있을 수 있다.
    let mut contributions = String::from("원천\t표제어\t단독\t승격\n");
    for ((source, _, _), contribution) in extracted.iter().zip(&report.contributions) {
        contributions.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            source.name,
            contribution.in_lexicon,
            contribution.unique_to_source,
            contribution.promoted
        ));
    }
    write(
        &build_directory.join(format!("{}-sources.tsv", recipe.name)),
        contributions.as_bytes(),
    )
}

fn create_directory(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path)
        .map_err(|error| format!("{} 만들기 실패: {error}", path.display()))
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|error| format!("{} 쓰기 실패: {error}", path.display()))
}
