//! 가나 표기 지식 — 조회 키가 서는 자리를 하나로 모은다.
//!
//! 일본어의 조회 키는 **히라가나 정규형**이다. 사람이 치는 길은 여럿이지만(로마자·12키
//! 가나·가타카나 붙여넣기) 사전에 물어보는 형태는 하나여야 하고, 표기(汉字·가타카나·
//! 전각 영수)는 조회 키에서 복원되지 않으므로 팩의 변환표가 따로 갖는다.
//!
//! 그래서 이 파일에는 "같은 소리의 다른 적기"를 하나로 모으는 규칙만 있다. 무엇이 무엇으로
//! 변환되는가는 여기 없다.

/// 가타카나를 히라가나로 옮길 때의 코드포인트 차. ァ(U+30A1)와 ぁ(U+3041)의 거리이며
/// ヶ(U+30F6)까지 같은 간격으로 이어진다.
const KATAKANA_OFFSET: u32 = 0x60;

/// 반각 가타카나 — 코드포인트 순서가 전각과 다르므로 표로 옮긴다. 탁점·반탁점은 뒤에
/// 따로 오므로(ｶ + ﾞ) 결합은 `normalize`가 한다.
const HALFWIDTH: [(char, char); 63] = [
    ('｡', '。'),
    ('｢', '「'),
    ('｣', '」'),
    ('､', '、'),
    ('･', '・'),
    ('ｦ', 'を'),
    ('ｧ', 'ぁ'),
    ('ｨ', 'ぃ'),
    ('ｩ', 'ぅ'),
    ('ｪ', 'ぇ'),
    ('ｫ', 'ぉ'),
    ('ｬ', 'ゃ'),
    ('ｭ', 'ゅ'),
    ('ｮ', 'ょ'),
    ('ｯ', 'っ'),
    ('ｰ', 'ー'),
    ('ｱ', 'あ'),
    ('ｲ', 'い'),
    ('ｳ', 'う'),
    ('ｴ', 'え'),
    ('ｵ', 'お'),
    ('ｶ', 'か'),
    ('ｷ', 'き'),
    ('ｸ', 'く'),
    ('ｹ', 'け'),
    ('ｺ', 'こ'),
    ('ｻ', 'さ'),
    ('ｼ', 'し'),
    ('ｽ', 'す'),
    ('ｾ', 'せ'),
    ('ｿ', 'そ'),
    ('ﾀ', 'た'),
    ('ﾁ', 'ち'),
    ('ﾂ', 'つ'),
    ('ﾃ', 'て'),
    ('ﾄ', 'と'),
    ('ﾅ', 'な'),
    ('ﾆ', 'に'),
    ('ﾇ', 'ぬ'),
    ('ﾈ', 'ね'),
    ('ﾉ', 'の'),
    ('ﾊ', 'は'),
    ('ﾋ', 'ひ'),
    ('ﾌ', 'ふ'),
    ('ﾍ', 'へ'),
    ('ﾎ', 'ほ'),
    ('ﾏ', 'ま'),
    ('ﾐ', 'み'),
    ('ﾑ', 'む'),
    ('ﾒ', 'め'),
    ('ﾓ', 'も'),
    ('ﾔ', 'や'),
    ('ﾕ', 'ゆ'),
    ('ﾖ', 'よ'),
    ('ﾗ', 'ら'),
    ('ﾘ', 'り'),
    ('ﾙ', 'る'),
    ('ﾚ', 'れ'),
    ('ﾛ', 'ろ'),
    ('ﾜ', 'わ'),
    ('ﾝ', 'ん'),
    ('ﾞ', '゛'),
    ('ﾟ', '゜'),
];

/// 탁점이 붙는 짝.
const VOICED: [(char, char); 22] = [
    ('か', 'が'),
    ('き', 'ぎ'),
    ('く', 'ぐ'),
    ('け', 'げ'),
    ('こ', 'ご'),
    ('さ', 'ざ'),
    ('し', 'じ'),
    ('す', 'ず'),
    ('せ', 'ぜ'),
    ('そ', 'ぞ'),
    ('た', 'だ'),
    ('ち', 'ぢ'),
    ('つ', 'づ'),
    ('て', 'で'),
    ('と', 'ど'),
    ('は', 'ば'),
    ('ひ', 'び'),
    ('ふ', 'ぶ'),
    ('へ', 'べ'),
    ('ほ', 'ぼ'),
    ('う', 'ゔ'),
    ('ゝ', 'ゞ'),
];

/// 반탁점이 붙는 짝 — は행에만 있다.
const SEMI_VOICED: [(char, char); 5] = [
    ('は', 'ぱ'),
    ('ひ', 'ぴ'),
    ('ふ', 'ぷ'),
    ('へ', 'ぺ'),
    ('ほ', 'ぽ'),
];

/// 「小゛゜」 키가 도는 주기. 한 키가 작은 글자·탁점·반탁점을 겸하는 것은 순정 12키의
/// 관례이고, 무엇이 무엇으로 갈리는지는 글자마다 다르므로 주기를 글자별로 적는다.
/// 마지막에서 한 번 더 누르면 첫 글자로 돌아온다.
const FORM_CYCLE: [&[char]; 22] = [
    &['あ', 'ぁ'],
    &['い', 'ぃ'],
    &['う', 'ぅ', 'ゔ'],
    &['え', 'ぇ'],
    &['お', 'ぉ'],
    &['か', 'が'],
    &['き', 'ぎ'],
    &['く', 'ぐ'],
    &['け', 'げ'],
    &['こ', 'ご'],
    &['さ', 'ざ'],
    &['し', 'じ'],
    &['す', 'ず'],
    &['せ', 'ぜ'],
    &['そ', 'ぞ'],
    &['た', 'だ'],
    &['ち', 'ぢ'],
    &['つ', 'っ', 'づ'],
    &['て', 'で'],
    &['と', 'ど'],
    &['や', 'ゃ'],
    &['わ', 'ゎ'],
];

/// は행은 작은 글자가 없고 탁점·반탁점 둘을 갖는다 — 주기가 셋이라 따로 적는다.
const HA_CYCLE: [&[char]; 5] = [
    &['は', 'ば', 'ぱ'],
    &['ひ', 'び', 'ぴ'],
    &['ふ', 'ぶ', 'ぷ'],
    &['へ', 'べ', 'ぺ'],
    &['ほ', 'ぼ', 'ぽ'],
];

/// や행의 나머지 — ゆ·よ는 작은 글자만 갖는다.
const SMALL_ONLY: [(char, char); 2] = [('ゆ', 'ゅ'), ('よ', 'ょ')];

pub fn is_hiragana(character: char) -> bool {
    matches!(character, 'ぁ'..='ゖ' | 'ー' | 'ゝ' | 'ゞ')
}

/// 조회 키에 설 수 있는 글자인가. 장음 부호와 반복 부호는 읽기의 일부이므로 함께 받는다.
pub fn is_key_character(character: char) -> bool {
    is_hiragana(character)
}

/// 히라가나를 가타카나로 — 같은 읽기의 다른 표기를 후보로 낼 때 쓴다.
pub fn to_katakana(text: &str) -> String {
    text.chars()
        .map(|character| match character {
            'ぁ'..='ゖ' => {
                char::from_u32(character as u32 + KATAKANA_OFFSET).unwrap_or(character)
            }
            other => other,
        })
        .collect()
}

/// 어떤 적기로 왔든 히라가나 정규형으로. 가나로 옮길 수 없는 글자가 섞이면 None —
/// 팩 컴파일러는 그런 표제어를 원천 잡음으로 보고 버린다.
pub fn normalize(text: &str) -> Option<String> {
    let mut result = String::with_capacity(text.len());
    for character in text.chars() {
        let mapped = match character {
            // 반각은 전각으로 먼저 옮긴다 — 탁점 결합은 그 뒤에 한 규칙으로 끝난다
            '｡'..='ﾟ' => halfwidth(character)?,
            'ァ'..='ヶ' => char::from_u32(character as u32 - KATAKANA_OFFSET)?,
            other => other,
        };
        // 따로 온 탁점·반탁점은 앞 글자에 결합한다
        match mapped {
            '゛' | '\u{3099}' => match result.pop().and_then(voiced) {
                Some(combined) => result.push(combined),
                None => return None,
            },
            '゜' | '\u{309A}' => match result.pop().and_then(semi_voiced) {
                Some(combined) => result.push(combined),
                None => return None,
            },
            other if is_key_character(other) => result.push(other),
            _ => return None,
        }
    }
    Some(result)
}

fn halfwidth(character: char) -> Option<char> {
    HALFWIDTH
        .iter()
        .find(|(half, _)| *half == character)
        .map(|(_, full)| *full)
}

pub fn voiced(character: char) -> Option<char> {
    VOICED
        .iter()
        .find(|(plain, _)| *plain == character)
        .map(|(_, voiced)| *voiced)
}

pub fn semi_voiced(character: char) -> Option<char> {
    SEMI_VOICED
        .iter()
        .find(|(plain, _)| *plain == character)
        .map(|(_, semi)| *semi)
}

/// 「小゛゜」 키가 눌렸다 — 이 글자의 다음 꼴. 주기가 없는 글자면 None이고, 그때 키는
/// 아무 일도 하지 않는다.
pub fn next_form(character: char) -> Option<char> {
    let cycled = FORM_CYCLE.iter().chain(HA_CYCLE.iter()).find_map(|cycle| {
        cycle
            .iter()
            .position(|&form| form == character)
            .map(|index| cycle[(index + 1) % cycle.len()])
    });
    cycled.or_else(|| {
        SMALL_ONLY
            .iter()
            .find_map(|&(large, small)| match character {
                _ if character == large => Some(small),
                _ if character == small => Some(large),
                _ => None,
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 가타카나는_히라가나로_모인다() {
        assert_eq!(normalize("カタカナ").as_deref(), Some("かたかな"));
        assert_eq!(normalize("ヴァイオリン").as_deref(), Some("ゔぁいおりん"));
    }

    #[test]
    fn 반각과_따로_온_탁점을_모은다() {
        assert_eq!(normalize("ｶﾞｯｷ").as_deref(), Some("がっき"));
        assert_eq!(normalize("は゛").as_deref(), Some("ば"));
        assert_eq!(normalize("は゜").as_deref(), Some("ぱ"));
    }

    #[test]
    fn 가나가_아닌_글자가_섞이면_키가_되지_않는다() {
        assert_eq!(normalize("漢字"), None);
        assert_eq!(normalize("abc"), None);
    }

    #[test]
    fn 소문자_탁점_주기가_한_바퀴_돈다() {
        assert_eq!(next_form('つ'), Some('っ'));
        assert_eq!(next_form('っ'), Some('づ'));
        assert_eq!(next_form('づ'), Some('つ'));
        assert_eq!(next_form('は'), Some('ば'));
        assert_eq!(next_form('ぱ'), Some('は'));
        assert_eq!(next_form('ん'), None);
    }

    #[test]
    fn 가타카나_표기를_낸다() {
        assert_eq!(to_katakana("ばいおりん"), "バイオリン");
    }
}
