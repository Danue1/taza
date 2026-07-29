//! 로마자 → 가나 오토마톤.
//!
//! 상태는 **아직 가나가 되지 못한 로마자**뿐이다. 그것이 조합 창에 그대로 보이는 것이
//! 순정 관례이고(「ky」를 친 자리에 ky가 보인다), 가나가 완성되는 순간 그 자리를 가나가
//! 대신한다.
//!
//! 표를 규칙으로 줄이지 않고 그대로 편 까닭은 예외가 규칙만큼 많기 때문이다 — し·ち·つ·
//! ふ는 자리만 보면 si·ti·tu·hu이고, 사람이 실제로 치는 것은 둘 다이며, ファ·ティ·ウィ
//! 같은 외래어 표기는 어느 규칙에도 서지 않는다. 표가 길어지는 값을 치르고 규칙과 예외가
//! 한자리에서 읽히는 것을 얻는다.
//!
//! 표에 없는 두 가지만 규칙으로 남는다: 같은 자음을 잇달아 치면 촉음(っ)이 되고, ん은
//! 뒤에 무엇이 오는지를 보고서야 정해진다.

/// 로마자 한 토막과 그것이 내는 가나. 긴 것부터 맞추므로 순서는 뜻이 없다.
const TABLE: &[(&str, &str)] = &[
    // あ행
    ("a", "あ"),
    ("i", "い"),
    ("u", "う"),
    ("e", "え"),
    ("o", "お"),
    ("yi", "い"),
    ("ye", "いぇ"),
    ("wu", "う"),
    // か행
    ("ka", "か"),
    ("ki", "き"),
    ("ku", "く"),
    ("ke", "け"),
    ("ko", "こ"),
    ("ca", "か"),
    ("cu", "く"),
    ("co", "こ"),
    ("qa", "くぁ"),
    ("qi", "くぃ"),
    ("qu", "く"),
    ("qe", "くぇ"),
    ("qo", "くぉ"),
    ("kya", "きゃ"),
    ("kyi", "きぃ"),
    ("kyu", "きゅ"),
    ("kye", "きぇ"),
    ("kyo", "きょ"),
    ("kwa", "くぁ"),
    // が행
    ("ga", "が"),
    ("gi", "ぎ"),
    ("gu", "ぐ"),
    ("ge", "げ"),
    ("go", "ご"),
    ("gya", "ぎゃ"),
    ("gyi", "ぎぃ"),
    ("gyu", "ぎゅ"),
    ("gye", "ぎぇ"),
    ("gyo", "ぎょ"),
    ("gwa", "ぐぁ"),
    // さ행
    ("sa", "さ"),
    ("si", "し"),
    ("su", "す"),
    ("se", "せ"),
    ("so", "そ"),
    ("shi", "し"),
    ("sha", "しゃ"),
    ("shu", "しゅ"),
    ("she", "しぇ"),
    ("sho", "しょ"),
    ("sya", "しゃ"),
    ("syi", "しぃ"),
    ("syu", "しゅ"),
    ("sye", "しぇ"),
    ("syo", "しょ"),
    ("ce", "せ"),
    ("ci", "し"),
    ("swa", "すぁ"),
    // ざ행
    ("za", "ざ"),
    ("zi", "じ"),
    ("zu", "ず"),
    ("ze", "ぜ"),
    ("zo", "ぞ"),
    ("ji", "じ"),
    ("ja", "じゃ"),
    ("ju", "じゅ"),
    ("je", "じぇ"),
    ("jo", "じょ"),
    ("jya", "じゃ"),
    ("jyi", "じぃ"),
    ("jyu", "じゅ"),
    ("jye", "じぇ"),
    ("jyo", "じょ"),
    ("zya", "じゃ"),
    ("zyi", "じぃ"),
    ("zyu", "じゅ"),
    ("zye", "じぇ"),
    ("zyo", "じょ"),
    // た행
    ("ta", "た"),
    ("ti", "ち"),
    ("tu", "つ"),
    ("te", "て"),
    ("to", "と"),
    ("chi", "ち"),
    ("tsu", "つ"),
    ("cha", "ちゃ"),
    ("chu", "ちゅ"),
    ("che", "ちぇ"),
    ("cho", "ちょ"),
    ("cya", "ちゃ"),
    ("cyi", "ちぃ"),
    ("cyu", "ちゅ"),
    ("cye", "ちぇ"),
    ("cyo", "ちょ"),
    ("tya", "ちゃ"),
    ("tyi", "ちぃ"),
    ("tyu", "ちゅ"),
    ("tye", "ちぇ"),
    ("tyo", "ちょ"),
    ("tsa", "つぁ"),
    ("tsi", "つぃ"),
    ("tse", "つぇ"),
    ("tso", "つぉ"),
    ("tha", "てゃ"),
    ("thi", "てぃ"),
    ("thu", "てゅ"),
    ("the", "てぇ"),
    ("tho", "てょ"),
    ("twa", "とぁ"),
    ("twi", "とぃ"),
    ("twu", "とぅ"),
    ("twe", "とぇ"),
    ("two", "とぉ"),
    // だ행
    ("da", "だ"),
    ("di", "ぢ"),
    ("du", "づ"),
    ("de", "で"),
    ("do", "ど"),
    ("dya", "ぢゃ"),
    ("dyi", "ぢぃ"),
    ("dyu", "ぢゅ"),
    ("dye", "ぢぇ"),
    ("dyo", "ぢょ"),
    ("dha", "でゃ"),
    ("dhi", "でぃ"),
    ("dhu", "でゅ"),
    ("dhe", "でぇ"),
    ("dho", "でょ"),
    ("dwa", "どぁ"),
    ("dwi", "どぃ"),
    ("dwu", "どぅ"),
    ("dwe", "どぇ"),
    ("dwo", "どぉ"),
    // な행
    ("na", "な"),
    ("ni", "に"),
    ("nu", "ぬ"),
    ("ne", "ね"),
    ("no", "の"),
    ("nya", "にゃ"),
    ("nyi", "にぃ"),
    ("nyu", "にゅ"),
    ("nye", "にぇ"),
    ("nyo", "にょ"),
    ("nn", "ん"),
    ("n'", "ん"),
    ("xn", "ん"),
    // は행
    ("ha", "は"),
    ("hi", "ひ"),
    ("hu", "ふ"),
    ("he", "へ"),
    ("ho", "ほ"),
    ("fu", "ふ"),
    ("fa", "ふぁ"),
    ("fi", "ふぃ"),
    ("fe", "ふぇ"),
    ("fo", "ふぉ"),
    ("hya", "ひゃ"),
    ("hyi", "ひぃ"),
    ("hyu", "ひゅ"),
    ("hye", "ひぇ"),
    ("hyo", "ひょ"),
    ("fya", "ふゃ"),
    ("fyu", "ふゅ"),
    ("fyo", "ふょ"),
    // ば행
    ("ba", "ば"),
    ("bi", "び"),
    ("bu", "ぶ"),
    ("be", "べ"),
    ("bo", "ぼ"),
    ("bya", "びゃ"),
    ("byi", "びぃ"),
    ("byu", "びゅ"),
    ("bye", "びぇ"),
    ("byo", "びょ"),
    // ぱ행
    ("pa", "ぱ"),
    ("pi", "ぴ"),
    ("pu", "ぷ"),
    ("pe", "ぺ"),
    ("po", "ぽ"),
    ("pya", "ぴゃ"),
    ("pyi", "ぴぃ"),
    ("pyu", "ぴゅ"),
    ("pye", "ぴぇ"),
    ("pyo", "ぴょ"),
    // ま행
    ("ma", "ま"),
    ("mi", "み"),
    ("mu", "む"),
    ("me", "め"),
    ("mo", "も"),
    ("mya", "みゃ"),
    ("myi", "みぃ"),
    ("myu", "みゅ"),
    ("mye", "みぇ"),
    ("myo", "みょ"),
    // や행
    ("ya", "や"),
    ("yu", "ゆ"),
    ("yo", "よ"),
    // ら행
    ("ra", "ら"),
    ("ri", "り"),
    ("ru", "る"),
    ("re", "れ"),
    ("ro", "ろ"),
    ("rya", "りゃ"),
    ("ryi", "りぃ"),
    ("ryu", "りゅ"),
    ("rye", "りぇ"),
    ("ryo", "りょ"),
    // わ행
    ("wa", "わ"),
    ("wi", "うぃ"),
    ("we", "うぇ"),
    ("wo", "を"),
    ("wha", "うぁ"),
    ("whi", "うぃ"),
    ("whu", "う"),
    ("whe", "うぇ"),
    ("who", "うぉ"),
    // ゔ행 — 표기는 ヴ이지만 조회 키는 히라가나이므로 ゔ로 둔다
    ("vu", "ゔ"),
    ("va", "ゔぁ"),
    ("vi", "ゔぃ"),
    ("ve", "ゔぇ"),
    ("vo", "ゔぉ"),
    ("vya", "ゔゃ"),
    ("vyu", "ゔゅ"),
    ("vyo", "ゔょ"),
    // 작은 글자를 곧장 — x와 l 둘 다 통한다
    ("xa", "ぁ"),
    ("xi", "ぃ"),
    ("xu", "ぅ"),
    ("xe", "ぇ"),
    ("xo", "ぉ"),
    ("la", "ぁ"),
    ("li", "ぃ"),
    ("lu", "ぅ"),
    ("le", "ぇ"),
    ("lo", "ぉ"),
    ("xya", "ゃ"),
    ("xyu", "ゅ"),
    ("xyo", "ょ"),
    ("lya", "ゃ"),
    ("lyu", "ゅ"),
    ("lyo", "ょ"),
    ("xtu", "っ"),
    ("xtsu", "っ"),
    ("ltu", "っ"),
    ("ltsu", "っ"),
    ("xwa", "ゎ"),
    ("lwa", "ゎ"),
    // 부호 — 일본어 자판이 그 자리에 내는 것들
    ("-", "ー"),
    (".", "。"),
    (",", "、"),
    ("/", "・"),
    ("[", "「"),
    ("]", "」"),
];

/// 촉음이 되는 자음. 모음과 ん은 겹쳐도 촉음이 아니며(ん은 「nn」이 이미 표에 있다),
/// 「tch」는 헵번식 표기라 t 다음의 c도 촉음으로 받는다.
fn doubles_into_small_tsu(first: char, second: char) -> bool {
    let consonant =
        first.is_ascii_alphabetic() && !matches!(first, 'a' | 'i' | 'u' | 'e' | 'o' | 'n');
    consonant && (first == second || (first == 't' && second == 'c'))
}

/// ん이 확정되는 자리 — n 다음에 모음도 y도 n도 아닌 글자가 오면 그 n은 홀로 ん이다.
fn resolves_to_n(rest: char) -> bool {
    !matches!(rest, 'a' | 'i' | 'u' | 'e' | 'o' | 'y' | 'n' | '\'')
}

fn exact(roman: &str) -> Option<&'static str> {
    TABLE
        .iter()
        .find(|(key, _)| *key == roman)
        .map(|(_, kana)| *kana)
}

fn extendable(roman: &str) -> bool {
    TABLE.iter().any(|(key, _)| key.starts_with(roman))
}

/// 타건 하나가 만든 것.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RomajiOutput {
    /// 이번 타건으로 가나가 된 부분
    pub kana: String,
    /// 아직 가나가 되지 못하고 남은 로마자 — 조합 창에 그대로 보인다
    pub pending: String,
}

/// 로마자 타건을 가나로 옮기는 오토마톤.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Romaji {
    pending: String,
}

impl Romaji {
    pub fn new() -> Self {
        Romaji::default()
    }

    pub fn pending(&self) -> &str {
        &self.pending
    }

    pub fn clear(&mut self) {
        self.pending.clear();
    }

    /// 타건 하나. 소문자로 접어 받으므로 shift가 올라가 있어도 같은 가나가 나온다 —
    /// 대소문자는 가나에 뜻이 없다.
    pub fn push(&mut self, letter: char) -> RomajiOutput {
        let mut buffer = std::mem::take(&mut self.pending);
        buffer.extend(letter.to_lowercase());
        let mut kana = String::new();
        loop {
            if let Some(mapped) = exact(&buffer) {
                kana.push_str(mapped);
                buffer.clear();
                break;
            }
            if extendable(&buffer) {
                break;
            }
            // 표에 없다 — 앞에서부터 한 글자씩 떼어 내며 규칙을 본다
            let mut characters = buffer.chars();
            let Some(first) = characters.next() else {
                break;
            };
            let Some(second) = characters.clone().next() else {
                // 홀로 남은 글자가 어떤 토막의 시작도 아니면 그대로 글로 남는다
                kana.push(first);
                buffer.clear();
                break;
            };
            if doubles_into_small_tsu(first, second) {
                kana.push('っ');
            } else if first == 'n' && resolves_to_n(second) {
                kana.push('ん');
            } else {
                kana.push(first);
            }
            buffer = characters.collect();
        }
        self.pending = buffer;
        RomajiOutput {
            kana,
            pending: self.pending.clone(),
        }
    }

    /// 남은 로마자를 한 글자 무른다. 무를 것이 없으면 false — 그때 삭제는 이미 가나가 된
    /// 쪽의 일이다.
    pub fn backspace(&mut self) -> bool {
        self.pending.pop().is_some()
    }

    /// 조합이 끝났다 — 아직 가나가 되지 못한 로마자는 친 그대로 남는다. 순정도 미완성
    /// 로마자를 버리지 않고 글자로 확정한다.
    pub fn flush(&mut self) -> String {
        let pending = std::mem::take(&mut self.pending);
        // 홀로 남은 n은 ん이다 — 그것만이 뒤를 보지 않고도 정해지는 로마자다
        match pending.as_str() {
            "n" => "ん".to_string(),
            other => other.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn type_all(letters: &str) -> (String, String) {
        let mut romaji = Romaji::new();
        let mut kana = String::new();
        for letter in letters.chars() {
            kana.push_str(&romaji.push(letter).kana);
        }
        (kana, romaji.pending().to_string())
    }

    #[test]
    fn 직음을_옮긴다() {
        assert_eq!(type_all("sakura"), ("さくら".to_string(), String::new()));
        assert_eq!(type_all("tokyo"), ("ときょ".to_string(), String::new()));
    }

    #[test]
    fn 요음은_표가_가진다() {
        assert_eq!(type_all("kyou"), ("きょう".to_string(), String::new()));
        assert_eq!(
            type_all("shashinn"),
            ("しゃしん".to_string(), String::new())
        );
        assert_eq!(
            type_all("syashinn"),
            ("しゃしん".to_string(), String::new())
        );
    }

    #[test]
    fn 겹자음은_촉음이_된다() {
        assert_eq!(type_all("gakkou"), ("がっこう".to_string(), String::new()));
        assert_eq!(type_all("kitte"), ("きって".to_string(), String::new()));
        assert_eq!(type_all("matcha"), ("まっちゃ".to_string(), String::new()));
    }

    /// ん은 로마자에서 유일하게 **뒤를 봐야 정해지는** 글자다. n 다음에 자음이 오면 그
    /// 자리에서 ん이 되고, 모음이나 y가 오면 な행이 된다. 둘 다 아닌 자리(어절 끝, 또는
    /// 모음 앞의 ん)에서는 사람이 n을 한 번 더 쳐서 못을 박는다 — 「かんい」가 "kanni"인
    /// 것이 그래서이고, 그 규칙 때문에 「こんにちは」는 n이 셋이다.
    #[test]
    fn ん은_뒤를_보고_정해진다() {
        assert_eq!(type_all("hon"), ("ほ".to_string(), "n".to_string()));
        assert_eq!(type_all("hondana"), ("ほんだな".to_string(), String::new()));
        assert_eq!(type_all("honya"), ("ほにゃ".to_string(), String::new()));
        assert_eq!(type_all("honnya"), ("ほんや".to_string(), String::new()));
        assert_eq!(type_all("kanni"), ("かんい".to_string(), String::new()));
        assert_eq!(
            type_all("konnnichiha"),
            ("こんにちは".to_string(), String::new())
        );
    }

    #[test]
    fn 홀로_남은_n은_확정에서_ん이_된다() {
        let mut romaji = Romaji::new();
        for letter in "hon".chars() {
            romaji.push(letter);
        }
        assert_eq!(romaji.flush(), "ん");
    }

    #[test]
    fn 미완성_로마자는_조합_창에_남는다() {
        let (kana, pending) = type_all("ky");
        assert!(kana.is_empty());
        assert_eq!(pending, "ky");
    }

    #[test]
    fn 표에_없는_글자는_글로_남는다() {
        assert_eq!(type_all("q1"), ("q1".to_string(), String::new()));
    }

    #[test]
    fn 무르기는_남은_로마자부터() {
        let mut romaji = Romaji::new();
        romaji.push('k');
        assert!(romaji.backspace());
        assert!(!romaji.backspace());
    }

    #[test]
    fn 부호도_일본어_자리로_간다() {
        assert_eq!(type_all("a-."), ("あー。".to_string(), String::new()));
    }
}
