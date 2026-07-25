//! 원천 하나를 신호로 바꾸는 길 — 조달 → (캐시) → 컨테이너 해제 → 파싱.
//!
//! 원천마다 다른 것은 넷이고 서로 직교한다: 어디서 구하는가(`acquire`), 어떻게
//! 압축돼 있는가(`container`), 어떤 형식인가(`crate::parse`), 팩에 무엇으로
//! 기여하는가(`recipe::Role`). 이 파일은 앞의 셋을 잇기만 한다.
//!
//! 큰 원천은 훑기 전에 더 싸게 풀리는 사본으로 바꿔 둔다(`transcode`) — 원천의 배포
//! 형식은 우리가 고를 수 없지만, 다시 훑을 때 무엇을 읽을지는 고를 수 있다.

pub mod acquire;
pub mod cache;
pub mod container;
pub mod transcode;

use crate::parse::{self, Signal};
use crate::recipe::Source;
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
pub fn prepare(
    source: &Source,
    language: &str,
    cache_directory: &Path,
    use_cache: bool,
) -> Result<Prepared, String> {
    let Some(located) = acquire::locate(source, cache_directory)? else {
        return Ok(Prepared::Skipped);
    };
    let cache_path = cache::path(
        &cache_directory.join("signals"),
        &located.digest,
        &source.extraction,
        language,
    );
    if use_cache && let Some(signal) = cache::load(&cache_path) {
        return Ok(Prepared::Extracted {
            signal,
            from_cache: true,
        });
    }
    // 다시 훑어야 할 때는 원천을 더 싸게 풀리는 사본으로 바꿔 놓고 훑는다
    let path = transcode::faster_copy(&located.path, cache_directory);
    let signal = parse::parse(&source.extraction, &path, language)?;
    if use_cache {
        cache::store(&cache_path, &signal)?;
    }
    Ok(Prepared::Extracted {
        signal,
        from_cache: false,
    })
}
