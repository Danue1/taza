//! 배포 산출물 — 전송용 압축 아카이브와 카탈로그.
//!
//! 팩 자체는 기기에서 mmap 조회로 읽히므로 압축된 상태로 둘 수 없다. 그래서 압축은
//! 전송 구간에만 쓴다: 컨테이너 앱이 `.tazapack.zst`를 받아 해시를 확인하고 풀어
//! App Group에 놓으면, 익스텐션은 평소처럼 mmap한다.

use crate::fetch::hex_digest;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 전송 크기가 우선인 오프라인 압축이라 최고 단계를 쓴다 — 해제 비용은 단계와 무관하다.
const COMPRESSION_LEVEL: i32 = 19;

#[derive(Debug, Serialize, Deserialize)]
pub struct Catalog {
    /// 이 카탈로그의 팩들이 요구하는 팩 포맷 버전 — 구버전 앱은 받지 않고 넘긴다.
    pub format_version: u16,
    pub packs: Vec<CatalogEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub name: String,
    pub language: String,
    pub pack_version: u32,
    pub word_count: usize,
    /// 카탈로그 URL 기준 상대 경로
    pub archive_file: String,
    pub archive_size: u64,
    pub archive_sha256: String,
    /// 압축을 푼 팩의 크기·해시 — 설치 직전에 한 번 더 검증한다.
    pub pack_size: u64,
    pub pack_sha256: String,
    pub sources: String,
    pub attribution: String,
}

pub struct Archive {
    pub bytes: Vec<u8>,
    pub sha256: String,
}

pub fn compress(pack: &[u8]) -> Result<Archive, String> {
    let bytes =
        zstd::encode_all(pack, COMPRESSION_LEVEL).map_err(|error| format!("압축 실패: {error}"))?;
    let sha256 = hex_digest(&bytes);
    Ok(Archive { bytes, sha256 })
}

/// 카탈로그는 언어별 빌드가 각자 자기 항목만 갈아 끼운다 — 한 언어를 다시 만들었다고
/// 다른 언어의 배포 정보가 사라지면 안 된다.
pub fn update_catalog(path: &Path, entry: CatalogEntry) -> Result<Catalog, String> {
    let mut catalog = match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str::<Catalog>(&text)
            .map_err(|error| format!("{} 해석 실패: {error}", path.display()))?,
        Err(_) => Catalog {
            format_version: taza_engine::pack::FORMAT_VERSION,
            packs: Vec::new(),
        },
    };
    catalog.format_version = taza_engine::pack::FORMAT_VERSION;
    catalog.packs.retain(|existing| existing.name != entry.name);
    catalog.packs.push(entry);
    catalog
        .packs
        .sort_by(|left, right| left.name.cmp(&right.name));
    let text = serde_json::to_string_pretty(&catalog)
        .map_err(|error| format!("카탈로그 직렬화 실패: {error}"))?;
    std::fs::write(path, format!("{text}\n"))
        .map_err(|error| format!("{} 쓰기 실패: {error}", path.display()))?;
    Ok(catalog)
}

/// 고지 문서는 손으로 쓰지 않는다 — 레시피의 원천 기록에서 만들어야 원천을 바꿀 때
/// 고지가 따라 바뀐다.
pub fn write_notice(path: &Path, catalog: &Catalog) -> Result<(), String> {
    let mut document = String::from(
        "# 데이터 출처·라이선스\n\n\
         이 문서는 `taza-packs`가 `data/recipes/*.toml`에서 생성한다 — 손으로 고치지 않는다.\n",
    );
    for pack in &catalog.packs {
        document.push_str(&format!(
            "\n## {} ({}) — 판 {}\n\n표제어 {}개\n\n### 원천\n\n",
            pack.name, pack.language, pack.pack_version, pack.word_count
        ));
        for line in pack.sources.lines() {
            document.push_str(&format!("- {line}\n"));
        }
        document.push_str("\n### 저작자 표시\n\n");
        for line in pack.attribution.lines() {
            document.push_str(&format!("> {line}\n"));
        }
    }
    document.push_str(
        "\n## 재현\n\n```\ncargo run --release -p taza-toolchain --bin taza-packs\n```\n",
    );
    std::fs::write(path, document).map_err(|error| format!("{} 쓰기 실패: {error}", path.display()))
}
