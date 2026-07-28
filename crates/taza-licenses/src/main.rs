//! 기기에 나가는 라이브러리의 라이선스를 모아 앱 자원과 고지 문서로 낸다.
//!
//! 목록을 손으로 쓰지 않는다 — 의존성을 하나 더하면 고지가 저절로 따라와야 한다.
//! 세는 기준은 **`taza-ffi`의 정상 의존성 closure**다: 데스크톱 전용 도구(`cli` 기능의
//! 바인딩 생성기, 팩 파이프라인, 평가 하네스)는 배포물에 링크되지 않으므로 빠진다.
//!
//! 이 도구는 워크스페이스의 어느 크레이트도 의존하지 않는다 — 읽는 것이 코드가 아니라
//! `cargo metadata`가 내는 그래프이기 때문이다. 그래서 iOS 빌드가 고지를 갱신하려고
//! 팩 파이프라인을 컴파일하지 않는다.
//!
//! ```text
//! cargo run -p taza-licenses
//! ```

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// 기기에 나가는 배포물의 뿌리. 여기서 닿는 것만 고지 대상이다.
const SHIPPED_ROOT: &str = "taza-ffi";
/// 이 목록을 만들 때 기준으로 삼는 대상 — 기기와 시뮬레이터의 의존성이 같다.
const TARGET: &str = "aarch64-apple-ios";

const LICENSE_FILE_STEMS: [&str; 4] = ["license", "licence", "copying", "unlicense"];

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
    resolve: Resolve,
    workspace_members: Vec<String>,
}

#[derive(Deserialize)]
struct Package {
    id: String,
    name: String,
    version: String,
    license: Option<String>,
    repository: Option<String>,
    manifest_path: PathBuf,
    targets: Vec<Target>,
    dependencies: Vec<Dependency>,
    /// 기능 이름 → 그 기능이 켜는 것들(다른 기능, `dep:이름`, `이름/기능`)
    features: BTreeMap<String, Vec<String>>,
}

#[derive(Deserialize)]
struct Dependency {
    name: String,
    #[serde(default)]
    optional: bool,
}

#[derive(Deserialize)]
struct Target {
    kind: Vec<String>,
}

impl Package {
    /// 절차적 매크로는 빌드 기계에서만 돌고 기기에 링크되지 않는다. 매크로가 만들어 낸
    /// 코드는 배포물에 들어가므로 크레이트 자신은 고지에 올리되, 그 크레이트의 의존성
    /// (askama·goblin 같은 것들)까지 따라가지는 않는다.
    fn is_proc_macro(&self) -> bool {
        self.targets
            .iter()
            .any(|target| target.kind.iter().any(|kind| kind == "proc-macro"))
    }

    /// 켜진 기능이 실제로 데려오는 선택적 의존성의 이름들.
    ///
    /// `cargo metadata`의 resolve 그래프는 **켜지지 않은 선택적 의존성까지** 간선으로
    /// 싣는다(uniffi의 `cli` 뒤에 있는 바인딩 생성기가 그렇게 딸려 온다). 그대로 따라가면
    /// 기기에 나가지도 않는 라이브러리가 고지에 오르므로 기능 표를 펴서 걸러 낸다.
    fn enabled_optional_dependencies(&self, enabled: &[String]) -> BTreeSet<&str> {
        let mut names = BTreeSet::new();
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut pending: Vec<&str> = enabled.iter().map(String::as_str).collect();
        while let Some(feature) = pending.pop() {
            if !seen.insert(feature) {
                continue;
            }
            let Some(entries) = self.features.get(feature) else {
                continue;
            };
            for entry in entries {
                if let Some(name) = entry.strip_prefix("dep:") {
                    names.insert(name);
                    continue;
                }
                // `이름?/기능`은 그 의존성이 이미 켜져 있을 때만 걸리므로 켜지 않는다
                if let Some((dependency, _)) = entry.split_once('/') {
                    if !dependency.ends_with('?') {
                        names.insert(dependency);
                    }
                    continue;
                }
                pending.push(entry);
            }
        }
        names
    }

    /// 이 의존성이 지금 켜진 기능으로 실제 빌드에 들어가는가.
    fn ships(&self, dependency_name: &str, enabled_optional: &BTreeSet<&str>) -> bool {
        let mut declared = self
            .dependencies
            .iter()
            .filter(|dependency| dependency.name == dependency_name)
            .peekable();
        if declared.peek().is_none() {
            return true;
        }
        declared
            .any(|dependency| !dependency.optional || enabled_optional.contains(dependency_name))
    }
}

#[derive(Deserialize)]
struct Resolve {
    nodes: Vec<Node>,
}

#[derive(Deserialize)]
struct Node {
    id: String,
    deps: Vec<NodeDep>,
    /// 이 패키지에서 실제로 켜진 기능들
    features: Vec<String>,
}

#[derive(Deserialize)]
struct NodeDep {
    pkg: String,
    dep_kinds: Vec<DepKind>,
}

#[derive(Deserialize)]
struct DepKind {
    /// null이 정상 의존성이고, "dev"·"build"는 배포물에 링크되지 않는다
    kind: Option<String>,
}

/// 앱이 읽는 형태. 라이선스 본문은 크레이트마다 거의 같으므로 한 번만 싣고 번호로 가리킨다.
#[derive(Serialize)]
struct LicenseCatalog {
    format_version: u16,
    texts: Vec<String>,
    packages: Vec<CatalogPackage>,
}

#[derive(Serialize)]
struct CatalogPackage {
    name: String,
    version: String,
    /// SPDX 식별자 — 크레이트가 밝히지 않았으면 빈 문자열이다
    license: String,
    repository: String,
    /// `texts`의 자리 번호
    texts: Vec<usize>,
}

fn main() {
    if let Err(message) = run() {
        eprintln!("실패: {message}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let root = repository_root()?;
    let metadata = load_metadata(&root)?;
    let shipped = shipped_packages(&metadata)?;

    let mut texts: Vec<String> = Vec::new();
    let mut index_of: BTreeMap<String, usize> = BTreeMap::new();
    let mut packages = Vec::with_capacity(shipped.len());
    let mut missing = Vec::new();

    for package in shipped {
        let found = license_texts(&package.manifest_path);
        if found.is_empty() {
            missing.push(format!("{} {}", package.name, package.version));
        }
        let mut indices = Vec::with_capacity(found.len());
        for text in found {
            let next = texts.len();
            let index = *index_of.entry(text.clone()).or_insert(next);
            if index == next {
                texts.push(text);
            }
            indices.push(index);
        }
        packages.push(CatalogPackage {
            name: package.name.clone(),
            version: package.version.clone(),
            license: package.license.clone().unwrap_or_default(),
            repository: package.repository.clone().unwrap_or_default(),
            texts: indices,
        });
    }

    let catalog = LicenseCatalog {
        format_version: 1,
        texts,
        packages,
    };
    let catalog_path = root.join("data/licenses.json");
    std::fs::write(
        &catalog_path,
        serde_json::to_string_pretty(&catalog).map_err(|error| format!("직렬화 실패: {error}"))?,
    )
    .map_err(|error| format!("{} 쓰기 실패: {error}", catalog_path.display()))?;

    let notice_path = root.join("data/notices/SOFTWARE-NOTICE.md");
    std::fs::write(&notice_path, notice(&catalog))
        .map_err(|error| format!("{} 쓰기 실패: {error}", notice_path.display()))?;

    println!(
        "라이브러리 {}개 / 라이선스 본문 {}개 → {} · {}",
        catalog.packages.len(),
        catalog.texts.len(),
        catalog_path.display(),
        notice_path.display()
    );
    // 본문을 못 찾은 크레이트는 SPDX 이름만 남는다 — 조용히 넘어가지 않고 알린다
    if !missing.is_empty() {
        println!("라이선스 본문 없음(식별자만 표기): {}", missing.join(", "));
    }
    Ok(())
}

fn repository_root() -> Result<PathBuf, String> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| "저장소 뿌리를 찾지 못했음".to_string())
}

fn load_metadata(root: &Path) -> Result<Metadata, String> {
    let output = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()))
        .current_dir(root)
        .args([
            "metadata",
            "--format-version",
            "1",
            "--filter-platform",
            TARGET,
        ])
        .output()
        .map_err(|error| format!("cargo metadata 실행 실패: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata 실패: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(|error| format!("metadata 해석 실패: {error}"))
}

/// 배포물에 링크되는 크레이트 — 뿌리에서 정상 의존성만 따라간 closure에서 우리 것을 뺀다.
fn shipped_packages(metadata: &Metadata) -> Result<Vec<&Package>, String> {
    let nodes: BTreeMap<&str, &Node> = metadata
        .resolve
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let root = metadata
        .packages
        .iter()
        .find(|package| package.name == SHIPPED_ROOT)
        .ok_or_else(|| format!("{SHIPPED_ROOT} 패키지를 찾지 못했음"))?;

    let by_id: BTreeMap<&str, &Package> = metadata
        .packages
        .iter()
        .map(|package| (package.id.as_str(), package))
        .collect();
    let mut reached: BTreeSet<&str> = BTreeSet::new();
    let mut pending = vec![root.id.as_str()];
    while let Some(id) = pending.pop() {
        if !reached.insert(id) {
            continue;
        }
        if by_id.get(id).is_some_and(|package| package.is_proc_macro()) {
            continue;
        }
        let (Some(node), Some(package)) = (nodes.get(id), by_id.get(id)) else {
            continue;
        };
        let enabled_optional = package.enabled_optional_dependencies(&node.features);
        for dep in &node.deps {
            // dev·build 의존성은 기기에 나가지 않는다
            if !dep.dep_kinds.iter().any(|kind| kind.kind.is_none()) {
                continue;
            }
            let name = by_id
                .get(dep.pkg.as_str())
                .map(|package| package.name.as_str())
                .unwrap_or_default();
            if package.ships(name, &enabled_optional) {
                pending.push(dep.pkg.as_str());
            }
        }
    }

    let ours: BTreeSet<&str> = metadata
        .workspace_members
        .iter()
        .map(String::as_str)
        .collect();
    let mut shipped: Vec<&Package> = metadata
        .packages
        .iter()
        .filter(|package| reached.contains(package.id.as_str()))
        .filter(|package| !ours.contains(package.id.as_str()))
        .collect();
    shipped.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.version.cmp(&right.version))
    });
    Ok(shipped)
}

/// 크레이트가 함께 배포한 라이선스 본문. 이름이 `LICENSE*`인 파일을 이름순으로 담는다 —
/// 이중 라이선스(MIT OR Apache-2.0) 크레이트는 파일이 둘이고 둘 다 실어야 한다.
fn license_texts(manifest_path: &Path) -> Vec<String> {
    let Some(directory) = manifest_path.parent() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_lowercase();
            LICENSE_FILE_STEMS.iter().any(|stem| name.starts_with(stem))
        })
        .collect();
    files.sort();
    files
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .collect()
}

fn notice(catalog: &LicenseCatalog) -> String {
    let mut document = String::from(
        "# 소프트웨어 라이선스\n\n\
         이 문서는 `taza-licenses`가 의존성 그래프에서 생성한다 — 손으로 고치지 않는다.\n\n\
         기기에 나가는 배포물(`taza-ffi` 정적 라이브러리)에 링크되는 크레이트만 싣는다.\n\
         팩 파이프라인·평가 하네스·바인딩 생성기처럼 데스크톱에서만 도는 것은 빠진다.\n\n\
         라이선스 본문은 앱의 `data/licenses.json`에 함께 실려 설정 화면에 나간다.\n\n",
    );
    for package in &catalog.packages {
        document.push_str(&format!("- **{}** {}", package.name, package.version));
        if !package.license.is_empty() {
            document.push_str(&format!(" — {}", package.license));
        }
        if !package.repository.is_empty() {
            document.push_str(&format!(" ({})", package.repository));
        }
        document.push('\n');
    }
    document
}
