//! 유니코드 `emoji-test.txt` — 이모지가 어느 묶음에 속하고 어떤 차례로 서는지를 밝힌 표.
//!
//! ```text
//! # group: Smileys & Emotion
//! # subgroup: face-smiling
//! 1F600 ; fully-qualified # 😀 E1.0 grinning face
//! ```
//!
//! 이 원천은 낱말을 하나도 늘리지 않는다 — 검색면이 검색어 없이 보여 줄 차례만 준다.
//! 완전한 형태(fully-qualified)만 받는다: 나머지는 같은 이모지의 이형이라 키보드에 두 번
//! 설 까닭이 없다. 피부색 같은 부품(Component) 묶음도 홀로 쓰는 것이 아니라 뺀다.

use super::Signal;
use crate::source::container;
use std::io::BufRead;
use std::path::Path;
use taza_engine::contract::EmojiCategory;

pub fn parse(path: &Path) -> Result<Signal, String> {
    let mut emoji_order = Vec::new();
    let mut category = None;
    container::for_each_member(path, |name, reader| {
        for line in reader.lines() {
            let line = line.map_err(|error| format!("{name} 읽기 실패: {error}"))?;
            let line = line.trim();
            if let Some(group) = line.strip_prefix("# group:") {
                category = category_of(group.trim());
                continue;
            }
            let (Some(category), Some(emoji)) = (category, fully_qualified_emoji(line)) else {
                continue;
            };
            emoji_order.push((category, emoji));
        }
        Ok(())
    })?;
    if emoji_order.is_empty() {
        return Err(format!("{}: 이모지 차례를 읽지 못했음", path.display()));
    }
    Ok(Signal {
        emoji_order,
        ..Signal::default()
    })
}

/// 유니코드 묶음 이름 → 검색면 묶음. 사람은 스마일리와 한자리에 서고(빌트인 관례),
/// 부품은 홀로 쓸 것이 아니라 자리를 갖지 않는다.
fn category_of(group: &str) -> Option<EmojiCategory> {
    match group {
        "Smileys & Emotion" | "People & Body" => Some(EmojiCategory::SmileysAndPeople),
        "Animals & Nature" => Some(EmojiCategory::AnimalsAndNature),
        "Food & Drink" => Some(EmojiCategory::FoodAndDrink),
        "Activities" => Some(EmojiCategory::Activities),
        "Travel & Places" => Some(EmojiCategory::TravelAndPlaces),
        "Objects" => Some(EmojiCategory::Objects),
        "Symbols" => Some(EmojiCategory::Symbols),
        "Flags" => Some(EmojiCategory::Flags),
        _ => None,
    }
}

/// 한 줄에서 완전한 형태의 이모지를 뽑는다. 주석 자리에 그림이 그대로 실려 있으므로
/// 코드포인트를 조립하지 않고 그것을 쓴다.
fn fully_qualified_emoji(line: &str) -> Option<String> {
    let (status, comment) = line.split_once('#')?;
    if !status.contains("; fully-qualified") {
        return None;
    }
    comment.split_whitespace().next().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 묶음 줄이 그 뒤에 오는 이모지의 자리를 정하고, 완전한 형태만 받는다.
    #[test]
    fn groups_split_the_order_and_only_full_forms_are_kept() {
        let mut category = None;
        let mut order = Vec::new();
        for line in [
            "# group: Smileys & Emotion",
            "1F600 ; fully-qualified # 😀 E1.0 grinning face",
            "# group: Component",
            "1F3FB ; component # 🏻 E1.0 light skin tone",
            "# group: Animals & Nature",
            "1F415 200D 1F9BA ; fully-qualified # 🐕‍🦺 E12.0 service dog",
            "1F415 1F9BA ; minimally-qualified # 🐕🦺 E12.0 service dog",
        ] {
            if let Some(group) = line.strip_prefix("# group:") {
                category = category_of(group.trim());
                continue;
            }
            if let (Some(category), Some(emoji)) = (category, fully_qualified_emoji(line)) {
                order.push((category, emoji));
            }
        }
        assert_eq!(
            order,
            vec![
                (EmojiCategory::SmileysAndPeople, "😀".to_string()),
                (EmojiCategory::AnimalsAndNature, "🐕‍🦺".to_string()),
            ]
        );
    }
}
