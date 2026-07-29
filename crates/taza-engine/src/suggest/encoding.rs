//! 입력 시퀀스 ↔ 사전 조회 키. 팩의 lexicon 인코딩과 짝을 이루며, 랭킹은 전부 키
//! 공간에서 이뤄지고 표시 형태로의 복원은 후보를 낼 때 한 번만 한다.

/// 팩이 표제어를 어떤 형태로 담고 있는지. 태그 문자열이 팩 메타데이터에 실린다.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum KeyEncoding {
    /// 표제어를 그대로 UTF-8로 — 라틴 전반
    #[default]
    Utf8,
    /// 한글을 자모 분해 후 두벌식 ASCII로. trie의 바이트 편집거리가 그대로 자모
    /// 편집거리가 되므로 교정을 자모 수준에서 다룰 수 있다.
    HangulJamoDubeolsik,
    /// 일본어를 히라가나 정규형으로. **이 인코딩만 되돌릴 수 없다** — 읽기 하나에 표기가
    /// 여럿이므로 표시 형태는 키에서 복원되지 않고 팩의 변환표가 따로 갖는다. `decode`가
    /// 읽기를 그대로 내는 것은 표가 없을 때의 바닥값이다(가나로만 적는 말은 그것이 곧 표기다).
    Kana,
}

impl KeyEncoding {
    pub fn tag(self) -> &'static str {
        match self {
            KeyEncoding::Utf8 => "utf8",
            KeyEncoding::HangulJamoDubeolsik => "hangul-jamo-dubeolsik",
            KeyEncoding::Kana => "kana",
        }
    }

    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "utf8" => Some(KeyEncoding::Utf8),
            "hangul-jamo-dubeolsik" => Some(KeyEncoding::HangulJamoDubeolsik),
            "kana" => Some(KeyEncoding::Kana),
            _ => None,
        }
    }

    /// 표시 형태 → 조회 키. 이 인코딩으로 담을 수 없는 글자가 섞이면 None이며,
    /// 팩 컴파일러는 그런 표제어를 원천 잡음으로 보고 버린다.
    pub fn encode(self, display: &str) -> Option<String> {
        match self {
            KeyEncoding::Utf8 => Some(display.to_string()),
            KeyEncoding::HangulJamoDubeolsik => encode_hangul(display),
            KeyEncoding::Kana => encode_kana(display),
        }
    }

    /// 조회 키 → 표시 형태.
    pub fn decode(self, key: &str) -> Option<String> {
        match self {
            KeyEncoding::Utf8 => Some(key.to_string()),
            KeyEncoding::HangulJamoDubeolsik => decode_hangul(key),
            KeyEncoding::Kana => Some(key.to_string()),
        }
    }

    /// 친 어절을 조회 키로 접는다 — 접기가 성립하는 키 공간에서만. 접을 것이 없거나
    /// 접기가 안전하지 않으면 None이며, 그때는 어절을 그대로 조회한다.
    ///
    /// 접기는 인코딩의 성질이지 보편 규칙이 아니다. 두벌식 ASCII는 **대문자 자리에
    /// 된소리·이중모음을 싣는다** — 'R'은 ㄲ이고 'r'은 ㄱ이다. 그런 키를 접으면 글자가
    /// 바뀌어 "까치"를 치는 사람이 "가치"를 받는다.
    pub(crate) fn fold(self, key: &str) -> Option<(String, super::lookup::Restore)> {
        match self {
            KeyEncoding::Utf8 => super::lookup::fold(key),
            KeyEncoding::HangulJamoDubeolsik | KeyEncoding::Kana => None,
        }
    }

    /// 이 키 공간에서 표시 글자 하나에 대응하는 키 바이트. 한 바이트가 되지 않는 글자는
    /// None이다 — 공간 모델이 자리마다 바이트 하나를 견주므로 그런 글자는 셈할 수 없다.
    ///
    /// 대문자는 접어서 본다. 조회 키가 접힌 공간에 있으면 터치도 같은 공간에서 견줘야
    /// 하고(문장 첫 글자가 매번 대문자로 들어온다), 접기가 없는 공간에서는 대소문자가
    /// 서로 다른 글자라 애초에 접을 것이 없다.
    pub(crate) fn key_byte(self, character: char) -> Option<u8> {
        let folded = match self.fold(&character.to_string()) {
            Some((folded, _)) => folded,
            None => character.to_string(),
        };
        match self.encode(&folded)?.as_bytes() {
            [byte] => Some(*byte),
            _ => None,
        }
    }
}

#[cfg(feature = "lang-hangul")]
fn encode_hangul(display: &str) -> Option<String> {
    use crate::lang::jamo::{decompose_word, encode_jamo_ascii};
    decompose_word(display)
        .as_deref()
        .and_then(encode_jamo_ascii)
}

#[cfg(feature = "lang-hangul")]
fn decode_hangul(key: &str) -> Option<String> {
    use crate::lang::jamo::{compose_word, decode_jamo_ascii};
    decode_jamo_ascii(key).map(|jamo| compose_word(&jamo))
}

// 한글이 빠진 빌드에서는 이 인코딩의 팩을 읽어도 조회가 성립하지 않는다 —
// 셸이 해당 언어를 비활성 처리하므로 여기서는 빈 결과로 물러난다.
#[cfg(not(feature = "lang-hangul"))]
fn encode_hangul(_display: &str) -> Option<String> {
    None
}

#[cfg(not(feature = "lang-hangul"))]
fn decode_hangul(_key: &str) -> Option<String> {
    None
}

#[cfg(feature = "lang-japanese")]
fn encode_kana(display: &str) -> Option<String> {
    crate::lang::kana::normalize(display)
}

#[cfg(not(feature = "lang-japanese"))]
fn encode_kana(_display: &str) -> Option<String> {
    None
}
