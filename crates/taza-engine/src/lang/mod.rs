//! 언어 선언과 조합 골격.
//!
//! 코드가 열거하는 것은 **골격**이지 언어가 아니다. 골격은 새로 만들 때마다 코드가
//! 늘지만(자모 오토마타, 변환기 등) 언어는 그 골격에 데이터를 꽂는 일이므로, 언어가
//! 늘어도 코어와 FFI는 그대로여야 한다. 그래서 표시 이름·키캡 표기·조회 키 인코딩은
//! 팩 메타데이터가 선언하고, 코드에는 팩을 아직 못 받았을 때 쓰는 내장 선언만 남는다.

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
use crate::keyboard::{KeyboardLayoutSet, layouts};
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

/// 조합 골격 — 어떤 방식으로 입력을 글자로 만드는가. 언어와 달리 이것은 코드다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposerSkeleton {
    /// composing 없이 즉시 확정 — 제안·교정도 붙지 않는다
    Direct,
    /// composing 없이 즉시 확정하되 어절을 추적해 제안·자동교정을 붙인다
    Latin,
    /// 자모 오토마타 + marked text
    Hangul,
    /// 자모 오토마타 + 천지인 모음 조합(하늘·땅·사람) + 자음 멀티탭
    HangulCheonjiin,
}

/// 후보 개수 상한 — 후보 바에 실제로 들어가는 수라 골격과 무관하다.
const SUGGESTION_LIMIT: usize = 3;

/// 어절 하나에 곁들일 항목 수(갈래마다). 후보 바는 낱말을 고르는 자리이고 이모지·기호·
/// 얼굴 문자는 그 곁에 붙는 것이므로, 갈래마다 한 자리씩만 준다.
const ANNOTATION_SUGGESTION_LIMIT: usize = 1;

impl ComposerSkeleton {
    pub fn tag(self) -> &'static str {
        match self {
            ComposerSkeleton::Direct => "direct",
            ComposerSkeleton::Latin => "latin",
            ComposerSkeleton::Hangul => "hangul",
            ComposerSkeleton::HangulCheonjiin => "hangul-cheonjiin",
        }
    }

    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "direct" => Some(ComposerSkeleton::Direct),
            "latin" => Some(ComposerSkeleton::Latin),
            "hangul" => Some(ComposerSkeleton::Hangul),
            "hangul-cheonjiin" => Some(ComposerSkeleton::HangulCheonjiin),
            _ => None,
        }
    }

    /// 이 빌드에 골격이 포함되지 않았으면 None — 셸은 해당 언어를 비활성 처리한다.
    pub fn composer(self) -> Option<Box<dyn Composer>> {
        match self {
            ComposerSkeleton::Direct | ComposerSkeleton::Latin => {
                #[cfg(feature = "lang-latin")]
                {
                    if self == ComposerSkeleton::Direct {
                        Some(Box::new(direct::DirectComposer::new()))
                    } else {
                        Some(Box::new(latin::LatinComposer::new()))
                    }
                }
                #[cfg(not(feature = "lang-latin"))]
                {
                    None
                }
            }
            ComposerSkeleton::Hangul => {
                #[cfg(feature = "lang-hangul")]
                {
                    Some(Box::new(hangul::HangulComposer::new()))
                }
                #[cfg(not(feature = "lang-hangul"))]
                {
                    None
                }
            }
            ComposerSkeleton::HangulCheonjiin => {
                #[cfg(feature = "lang-hangul")]
                {
                    Some(Box::new(cheonjiin::CheonjiinComposer::new()))
                }
                #[cfg(not(feature = "lang-hangul"))]
                {
                    None
                }
            }
        }
    }

    /// 단어 경계에서 자동교정을 시도하는가. 원문(as-typed) 후보를 함께 노출할지도
    /// 이 값에 딸린다 — 교정을 피해 원문을 고르는 것이 곧 학습 경로이기 때문이다.
    /// 한글처럼 조합 자체가 표시 단위인 스크립트는 경계 교정 대신 후보 선택으로 고친다.
    fn autocorrects(self) -> bool {
        self == ComposerSkeleton::Latin
    }

    /// 팩에 레이아웃 섹션이 없을 때 쓰는 내장 배열.
    fn builtin_layout(self) -> KeyboardLayoutSet {
        match self {
            ComposerSkeleton::Hangul | ComposerSkeleton::HangulCheonjiin => layouts::dubeolsik(),
            _ => layouts::qwerty(),
        }
    }
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
    /// 이 언어가 쓰는 배열의 이름 — 설정 화면의 설명
    pub layout_name: String,
    pub skeleton: ComposerSkeleton,
    pub encoding: KeyEncoding,
}

impl LanguageDescriptor {
    /// 팩이 스스로 밝힌 선언. 필수 키가 빠졌거나 모르는 골격이면 None.
    pub fn from_pack(pack: &Pack<'_>) -> Option<Self> {
        let metadata = pack.metadata()?;
        let skeleton = ComposerSkeleton::from_tag(metadata.get(keys::COMPOSER_SKELETON)?)?;
        let encoding = KeyEncoding::from_tag(metadata.get(keys::LEXICON_ENCODING)?)?;
        Some(LanguageDescriptor {
            tag: pack.language().to_string(),
            display_name: metadata.get(keys::DISPLAY_NAME)?.to_string(),
            keycap_label: metadata.get(keys::KEYCAP_LABEL)?.to_string(),
            layout_name: metadata.get(keys::LAYOUT_NAME)?.to_string(),
            skeleton,
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
                layout_name: "QWERTY".to_string(),
                skeleton: ComposerSkeleton::Latin,
                encoding: KeyEncoding::Utf8,
            }),
            "ko" => Some(LanguageDescriptor {
                tag: "ko".to_string(),
                display_name: "한국어".to_string(),
                keycap_label: "한".to_string(),
                layout_name: "두벌식".to_string(),
                skeleton: ComposerSkeleton::Hangul,
                encoding: KeyEncoding::HangulJamoDubeolsik,
            }),
            _ => None,
        }
    }

    pub fn suggestion_policy(&self) -> SuggestionPolicy {
        SuggestionPolicy {
            encoding: self.encoding,
            autocorrect: self.skeleton.autocorrects(),
            limit: SUGGESTION_LIMIT,
            annotation_limit: ANNOTATION_SUGGESTION_LIMIT,
        }
    }

    pub(crate) fn builtin_layout(&self) -> KeyboardLayoutSet {
        self.skeleton.builtin_layout()
    }
}
