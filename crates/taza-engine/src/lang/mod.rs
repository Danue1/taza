//! 언어 선언과 입력 방식.
//!
//! 코드가 등록하는 것은 **입력 방식**이지 언어가 아니다. 방식은 새로 만들 때마다 코드가
//! 늘지만(자모 오토마타, 변환기, 그 방식으로 치는 자판) 언어는 그 방식에 데이터를 꽂는
//! 일이므로, 언어가 늘어도 코어와 FFI는 그대로여야 한다. 그래서 표시 이름·키캡 표기·조회
//! 키 인코딩은 팩 메타데이터가 선언하고, 코드에는 팩을 아직 못 받았을 때 쓰는 내장 선언만
//! 남는다.
//!
//! 방식 하나가 **자기 배열과 자기 합성기를 함께** 갖는다. 조합 규칙과 그 규칙으로 치는
//! 자판은 짝이라, 둘을 따로 등록하면 어느 한쪽만 있는 상태가 생긴다.

#[cfg(feature = "lang-hangul")]
pub mod cheonjiin;
#[cfg(feature = "lang-latin")]
pub mod direct;
#[cfg(feature = "lang-hangul")]
pub mod hangul;
#[cfg(feature = "lang-hangul")]
pub mod jamo;
#[cfg(feature = "lang-latin")]
pub mod latin;

use crate::contract::{Composer, Pack};
use crate::keyboard::{KeyboardLayoutSet, NamedLayoutSet};
use crate::pack::metadata::keys;
use crate::suggest::{KeyEncoding, SuggestionPolicy};

/// 키캡에 찍을 글자. 자리를 밝힌 자모(세벌식 배열의 초성·중성·종성)는 홀로 놓이면
/// 글꼴이 점선 동그라미를 달거나 좁게 그리므로, 사람에게 보일 때만 호환 자모로 옮긴다.
/// 합성기가 받는 값은 자리를 잃으면 안 되므로 그대로 흘러간다.
pub(crate) fn keycap_form(character: char) -> char {
    #[cfg(feature = "lang-hangul")]
    {
        jamo::keycap_form(character)
    }
    #[cfg(not(feature = "lang-hangul"))]
    {
        character
    }
}

/// 후보 개수 상한 — 후보 바에 실제로 들어가는 수라 방식과 무관하다.
const SUGGESTION_LIMIT: usize = 3;

/// 어절 하나에 곁들일 항목 수(갈래마다). 후보 바는 낱말을 고르는 자리이고 이모지·기호·
/// 얼굴 문자는 그 곁에 붙는 것이므로, 갈래마다 한 자리씩만 준다.
const ANNOTATION_SUGGESTION_LIMIT: usize = 1;

/// 입력 방식 하나 — 어떤 방식으로 입력을 글자로 만드는가. 언어와 달리 이것은 코드다.
///
/// 방식을 늘리는 일은 파일 하나에 구현 하나를 더하고 `REGISTRY`에 한 줄을 잇는 일이다.
/// 키가 무엇을 내는지는 여전히 배열이 **데이터로** 적는다 — 이웃 확률(`keyboard::hit`)이
/// 판 전체의 키를 훑어야 하므로, 방식이 키 해석을 통째로 가로채면 그 정보가 사라진다.
pub trait InputMethod: Send + Sync {
    /// 팩·설정이 이 방식을 가리키는 유일한 이름
    fn tag(&self) -> &'static str;

    /// 이 방식으로 칠 수 있는 배열들. 첫 항목이 기본 배열이고, 어느 것으로 칠지는 설정이
    /// 정한다. 같은 스크립트를 치는 방식들은 같은 목록을 내야 한다 — 목록이 방식마다
    /// 다르면 한 배열을 고른 순간 다른 배열로 갈아탈 길이 막힌다.
    fn layouts(&self) -> Vec<NamedLayoutSet>;

    fn composer(&self) -> Box<dyn Composer>;

    /// 단어 경계에서 자동교정을 시도하는가. 원문(as-typed) 후보를 함께 노출할지도
    /// 이 값에 딸린다 — 교정을 피해 원문을 고르는 것이 곧 학습 경로이기 때문이다.
    /// 한글처럼 조합 자체가 표시 단위인 스크립트는 경계 교정 대신 후보 선택으로 고친다.
    fn autocorrects(&self) -> bool {
        false
    }

    /// 기본 배열 한 벌 — 배열을 고르는 자리가 아닌 곳(오타 합성 같은 오프라인 계산)이
    /// 키 기하만 필요할 때 쓴다.
    fn default_layouts(&self) -> KeyboardLayoutSet {
        let mut layouts = self.layouts();
        assert!(!layouts.is_empty(), "입력 방식마다 배열이 최소 한 벌");
        layouts.swap_remove(0).layouts
    }
}

/// 방식을 가리키는 이름이 곧 그 방식이다 — 태그가 같으면 같은 방식이고, 레지스트리에
/// 같은 태그가 둘 있을 수 없으므로 이 비교는 실체 비교와 같다.
impl PartialEq for dyn InputMethod {
    fn eq(&self, other: &Self) -> bool {
        self.tag() == other.tag()
    }
}

impl Eq for dyn InputMethod {}

impl std::fmt::Debug for dyn InputMethod {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.tag())
    }
}

/// 이 빌드가 싣고 있는 입력 방식 전부. 언어 feature를 끄면 그 방식이 목록에서 빠지고,
/// 그 방식을 가리키는 팩·설정은 조용히 쓰이지 않는다.
const REGISTRY: &[&dyn InputMethod] = &[
    #[cfg(feature = "lang-latin")]
    &direct::DIRECT,
    #[cfg(feature = "lang-latin")]
    &latin::LATIN,
    #[cfg(feature = "lang-hangul")]
    &hangul::HANGUL,
    #[cfg(feature = "lang-hangul")]
    &cheonjiin::CHEONJIIN,
];

/// 태그가 가리키는 입력 방식. 이 빌드에 없으면 None — 셸은 해당 언어를 비활성 처리한다.
pub fn input_method(tag: &str) -> Option<&'static dyn InputMethod> {
    REGISTRY.iter().copied().find(|method| method.tag() == tag)
}

/// 언어 하나의 선언. 원본은 팩 메타데이터이고, 코드에 남는 것은 팩을 아직 못 받았을 때
/// 쓰는 내장 선언뿐이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageDescriptor {
    /// 언어 태그 — 팩·설정·셸이 언어를 가리키는 유일한 이름
    pub tag: String,
    /// 스페이스바와 언어 목록에 쓰는 이름. 언어는 자기 이름으로 표기한다(순정 관례).
    pub display_name: String,
    /// 언어 키에 찍히는 짧은 표기
    pub keycap_label: String,
    pub method: &'static dyn InputMethod,
    pub encoding: KeyEncoding,
}

impl LanguageDescriptor {
    /// 팩이 스스로 밝힌 선언. 필수 키가 빠졌거나 이 빌드에 없는 방식이면 None.
    pub fn from_pack(pack: &Pack<'_>) -> Option<Self> {
        let metadata = pack.metadata()?;
        let method = input_method(metadata.get(keys::INPUT_METHOD)?)?;
        let encoding = KeyEncoding::from_tag(metadata.get(keys::LEXICON_ENCODING)?)?;
        Some(LanguageDescriptor {
            tag: pack.language().to_string(),
            display_name: metadata.get(keys::DISPLAY_NAME)?.to_string(),
            keycap_label: metadata.get(keys::KEYCAP_LABEL)?.to_string(),
            method,
            encoding,
        })
    }

    /// 팩 없이 쓸 수 있는 내장 선언. 앱에 내장된 기본 언어와, 팩을 아직 못 받은 언어의
    /// 폴백이다 — 여기 없는 태그도 팩만 받으면 동작한다.
    pub fn builtin(tag: &str) -> Option<Self> {
        match tag {
            "en" => Some(LanguageDescriptor {
                tag: "en".to_string(),
                display_name: "English".to_string(),
                keycap_label: "A".to_string(),
                method: input_method("latin")?,
                encoding: KeyEncoding::Utf8,
            }),
            "ko" => Some(LanguageDescriptor {
                tag: "ko".to_string(),
                display_name: "한국어".to_string(),
                keycap_label: "한".to_string(),
                method: input_method("hangul")?,
                encoding: KeyEncoding::HangulJamoDubeolsik,
            }),
            _ => None,
        }
    }

    pub fn suggestion_policy(&self) -> SuggestionPolicy {
        SuggestionPolicy {
            encoding: self.encoding,
            autocorrect: self.method.autocorrects(),
            limit: SUGGESTION_LIMIT,
            annotation_limit: ANNOTATION_SUGGESTION_LIMIT,
        }
    }
}
