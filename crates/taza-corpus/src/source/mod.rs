//! 원천 하나를 신호로 바꾸는 길 — 조달 → (캐시) → 컨테이너 해제 → 파싱.
//!
//! 원천마다 다른 것은 셋이고 서로 직교한다: 어디서 구하는가(`acquire`), 어떻게
//! 압축돼 있는가(`container`), 어떤 형식인가(`crate::parse`). 이 파일은 그 셋을 잇는다.
//! 팩에 무엇으로 기여하는지는 여기서 묻지 않는다 — 그것은 원천이 아니라 예산의 문제다.
//!
//! 큰 원천은 훑기 전에 더 싸게 풀리는 사본으로 바꿔 둔다(`transcode`) — 원천의 배포
//! 형식은 우리가 고를 수 없지만, 다시 훑을 때 무엇을 읽을지는 고를 수 있다.

pub mod acquire;
pub mod cache;
pub mod container;
pub mod transcode;

use crate::declaration::SourceFile;
use crate::parse::{self, Signal};
use std::path::Path;

/// 원천 하나를 처리한 결과.
pub enum Prepared {
    Extracted {
        signal: Signal,
        from_cache: bool,
    },
    /// 자리에 없는 선택 원천 — 손으로 받아야 하는 말뭉치가 아직 없어도 팩은 나와야 한다.
    Skipped,
}

/// 원천을 신호로 만든다. 캐시가 살아 있으면 원천을 다시 훑지 않는다.
///
/// 캐시는 두 갈래다: 내려받은 원천 그대로(`downloads`, 기가바이트 단위)와 그것을 훑어
/// 얻은 신호(`signals`). 신호가 낡는 조건이 원천이 낡는 조건보다 훨씬 자주 걸리므로
/// (파서를 고칠 때마다) 한 자리에 섞어 두면 지울 때 늘 함께 지우게 된다.
pub fn prepare(
    declared: &SourceFile,
    name: &str,
    language: &str,
    cache_directory: &Path,
    use_cache: bool,
) -> Result<Prepared, String> {
    let downloads = cache_directory.join("downloads");
    let Some(located) = acquire::locate(declared, name, &downloads)? else {
        return Ok(Prepared::Skipped);
    };
    let cache_path = cache::path(
        &cache_directory.join("signals"),
        &located.digest,
        &declared.extraction,
        language,
    )?;
    if use_cache && let Some(signal) = cache::load(&cache_path) {
        return Ok(Prepared::Extracted {
            signal,
            from_cache: true,
        });
    }
    // 다시 훑어야 할 때는 원천을 더 싸게 풀리는 사본으로 바꿔 놓고 훑는다
    let path = transcode::faster_copy(&located.path, &downloads);
    let signal = parse::parse(&declared.extraction, &path, language)?;
    if use_cache {
        cache::store(&cache_path, &signal)?;
    }
    Ok(Prepared::Extracted {
        signal,
        from_cache: false,
    })
}
