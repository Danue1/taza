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
}

impl KeyEncoding {
    pub fn tag(self) -> &'static str {
        match self {
            KeyEncoding::Utf8 => "utf8",
            KeyEncoding::HangulJamoDubeolsik => "hangul-jamo-dubeolsik",
        }
    }

    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "utf8" => Some(KeyEncoding::Utf8),
            "hangul-jamo-dubeolsik" => Some(KeyEncoding::HangulJamoDubeolsik),
            _ => None,
        }
    }

    /// 표시 형태 → 조회 키. 이 인코딩으로 담을 수 없는 글자가 섞이면 None이며,
    /// 팩 컴파일러는 그런 표제어를 원천 잡음으로 보고 버린다.
    pub fn encode(self, display: &str) -> Option<String> {
        match self {
            KeyEncoding::Utf8 => Some(display.to_string()),
            KeyEncoding::HangulJamoDubeolsik => encode_hangul(display),
        }
    }

    /// 조회 키 → 표시 형태.
    pub fn decode(self, key: &str) -> Option<String> {
        match self {
            KeyEncoding::Utf8 => Some(key.to_string()),
            KeyEncoding::HangulJamoDubeolsik => decode_hangul(key),
        }
    }
}

#[cfg(feature = "lang-hangul")]
fn encode_hangul(display: &str) -> Option<String> {
    use crate::lang::jamo::{decompose_word, encode_jamo_ascii};
    decompose_word(display).as_deref().and_then(encode_jamo_ascii)
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
