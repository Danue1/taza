//! 원천 하나를 신호로 바꾸는 길 — 조달 → (캐시) → 컨테이너 해제 → 파싱.
//!
//! 원천마다 다른 것은 넷이고 서로 직교한다: 어디서 구하는가(`acquire`), 어떻게
//! 압축돼 있는가(`container`), 어떤 형식인가(`crate::parse`), 팩에 무엇으로
//! 기여하는가(`recipe::Role`). 이 파일은 앞의 셋을 잇기만 한다.

pub mod acquire;
pub mod cache;
pub mod container;

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
    let signal = parse::parse(&source.extraction, &located.path, language)?;
    if use_cache {
        cache::store(&cache_path, &signal)?;
    }
    Ok(Prepared::Extracted {
        signal,
        from_cache: false,
    })
}
