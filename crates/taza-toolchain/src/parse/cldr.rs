//! CLDR 이모지 주석 — 이모지마다 그것을 부르는 낱말들이 달려 있다.
//!
//! ```xml
//! <annotation cp="😀">미소 | 스마일 | 얼굴 | 웃음 | 활짝 웃는 얼굴</annotation>
//! <annotation cp="😀" type="tts">활짝 웃는 얼굴</annotation>
//! ```
//!
//! 어절 하나로 치는 낱말만 받는다. "활짝 웃는 얼굴"처럼 띄어 쓴 이름은 후보 바가
//! 상대하는 단위가 아니다 — 사람은 어절을 치고, 그 어절에 이모지가 달려 있어야 한다.

use super::Signal;
use crate::source::container;
use std::io::BufRead;
use std::path::Path;

pub fn parse(path: &Path) -> Result<Signal, String> {
    let mut annotations = Vec::new();
    container::for_each_member(path, |name, reader| {
        for line in reader.lines() {
            let line = line.map_err(|error| format!("{name} 읽기 실패: {error}"))?;
            let Some((emoji, keywords)) = annotation(line.trim()) else {
                continue;
            };
            for keyword in keywords {
                annotations.push((keyword, emoji.to_string()));
            }
        }
        Ok(())
    })?;
    if annotations.is_empty() {
        return Err(format!("{}: 이모지 주석을 읽지 못했음", path.display()));
    }
    Ok(Signal {
        emoji: annotations,
        ..Signal::default()
    })
}

/// 한 줄에서 (이모지, 낱말들)을 뽑는다. `type="tts"`는 대표 이름 한 개뿐이라 낱말 목록과
/// 같은 자리에 넣어도 뜻이 어긋나지 않는다 — 어느 쪽이든 "이렇게 부르는 이모지"다.
fn annotation(line: &str) -> Option<(&str, Vec<String>)> {
    let rest = line.strip_prefix("<annotation cp=\"")?;
    let (emoji, rest) = rest.split_once('"')?;
    let body = rest.split_once('>')?.1.strip_suffix("</annotation>")?;
    let keywords = body
        .split('|')
        .map(str::trim)
        // 어절 하나로 치는 것만. 빈 것과 띄어 쓴 이름은 후보 바의 단위가 아니다.
        .filter(|keyword| !keyword.is_empty() && !keyword.contains(char::is_whitespace))
        .map(unescape)
        .collect::<Vec<_>>();
    (!keywords.is_empty()).then_some((emoji, keywords))
}

/// CLDR 주석에 나타나는 XML 엔티티. 낱말 안에 쓰이는 것은 이 셋뿐이다.
fn unescape(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_keywords_and_the_display_name() {
        let (emoji, keywords) = annotation(
            r#"<annotation cp="😀" draft="contributed">미소 | 스마일 | 웃음</annotation>"#,
        )
        .unwrap();
        assert_eq!(emoji, "😀");
        assert_eq!(keywords, vec!["미소", "스마일", "웃음"]);
    }

    /// 띄어 쓴 이름은 어절이 아니다 — 사람은 "활짝 웃는 얼굴"을 한 번에 치지 않는다.
    #[test]
    fn multi_word_names_are_skipped() {
        assert!(
            annotation(r#"<annotation cp="😀" type="tts">활짝 웃는 얼굴</annotation>"#).is_none()
        );
        // 어절 하나짜리 대표 이름은 받는다
        let (emoji, keywords) =
            annotation(r#"<annotation cp="🐱" type="tts">고양이</annotation>"#).unwrap();
        assert_eq!((emoji, keywords), ("🐱", vec!["고양이".to_string()]));
    }

    #[test]
    fn other_lines_are_ignored() {
        assert!(annotation("<ldml>").is_none());
        assert!(annotation(r#"<version number="$Revision$"/>"#).is_none());
    }
}
