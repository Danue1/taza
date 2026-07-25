//! 언어별 팩 레시피 — 원천이 무엇이고 어떻게 팩으로 가공되는지의 단일 선언.
//! 언어 추가는 이 파일을 하나 더 쓰는 작업이 되도록 설계했다.

use serde::Deserialize;
use taza_engine::suggest::KeyEncoding;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    /// 팩 파일 이름과 카탈로그 식별자 (`english` → `english.tazapack`)
    pub name: String,
    /// 팩 헤더에 담기는 언어 태그 (`en`, `ko`)
    pub language: String,
    /// 언어가 자기를 부르는 이름 — 스페이스바와 언어 목록에 그대로 나간다
    pub display_name: String,
    /// 언어 키에 찍히는 짧은 표기
    pub keycap_label: String,
    /// 조합 골격 — 이 언어를 어느 합성기로 칠 것인가
    pub composer_skeleton: String,
    /// 이 팩이 싣는 배열의 이름 — 설정 화면에 그대로 나간다
    pub layout_name: String,
    /// 데이터 판 번호 — 같은 언어의 갱신 배포를 구분한다. 원천·규칙을 바꾸면 올린다.
    pub pack_version: u32,
    #[serde(default)]
    pub lexicon: LexiconRules,
    #[serde(default)]
    pub language_model: LanguageModelRules,
    #[serde(default)]
    pub script: ScriptTraits,
    /// 레이아웃 DSL 파일 경로 (레시피 파일 기준 상대 경로)
    pub layout: Option<PathBuf>,
    pub sources: Vec<Source>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LexiconRules {
    /// 표제어 저장 인코딩
    #[serde(default)]
    pub encoding: LexiconEncoding,
    /// 표제어에 허용되는 문자 집합 — 이 밖의 글자가 섞인 표제어는 버린다.
    #[serde(default)]
    pub character_set: CharacterSet,
    /// 팩에 담을 표제어 상한 (점수 내림차순으로 자른다) — 크기 예산의 손잡이
    pub max_words: usize,
    #[serde(default = "default_minimum_word_length")]
    pub minimum_word_length: usize,
}

impl Default for LexiconRules {
    fn default() -> Self {
        LexiconRules {
            encoding: LexiconEncoding::default(),
            character_set: CharacterSet::default(),
            max_words: 100_000,
            minimum_word_length: default_minimum_word_length(),
        }
    }
}

fn default_minimum_word_length() -> usize {
    2
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageModelRules {
    /// 팩에 담을 bigram 상한 (이득 내림차순으로 자른다) — 크기 예산의 손잡이
    pub max_bigrams: usize,
    /// 이 횟수 미만으로 관측된 짝은 잡음으로 본다. 문맥 신호는 낱말보다 희소하므로
    /// 표제어 임계와 따로 잡는다.
    #[serde(default = "default_minimum_bigram_count")]
    pub minimum_count: u64,
}

impl Default for LanguageModelRules {
    fn default() -> Self {
        LanguageModelRules {
            max_bigrams: 50_000,
            minimum_count: default_minimum_bigram_count(),
        }
    }
}

fn default_minimum_bigram_count() -> u64 {
    3
}

/// TOML 표면 — 값의 의미와 인코딩 규칙은 `taza_engine::suggest::KeyEncoding`이 원본이다.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LexiconEncoding {
    #[default]
    Utf8,
    HangulJamoDubeolsik,
}

impl From<LexiconEncoding> for KeyEncoding {
    fn from(encoding: LexiconEncoding) -> Self {
        match encoding {
            LexiconEncoding::Utf8 => KeyEncoding::Utf8,
            LexiconEncoding::HangulJamoDubeolsik => KeyEncoding::HangulJamoDubeolsik,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CharacterSet {
    /// ASCII 소문자와 어퍼스트로피 — 라틴 v1의 타이핑 가능 범위
    #[default]
    LatinLowercase,
    /// 한글 음절만 (자모 낱글자·한자·라틴 혼입 배제)
    HangulSyllables,
}

impl CharacterSet {
    pub fn accepts(self, word: &str) -> bool {
        match self {
            CharacterSet::LatinLowercase => word
                .chars()
                .all(|character| character.is_ascii_lowercase() || character == '\''),
            CharacterSet::HangulSyllables => word
                .chars()
                .all(|character| ('가'..='힣').contains(&character)),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptTraits {
    /// 어절을 공백으로 나누는 스크립트인지 — 추후 언어가 코드 수정 없이 데이터로
    /// 선언되도록 팩 메타데이터에 실어 보낸다.
    #[serde(default = "default_true")]
    pub word_separated: bool,
    #[serde(default)]
    pub right_to_left: bool,
}

impl Default for ScriptTraits {
    fn default() -> Self {
        ScriptTraits {
            word_separated: true,
            right_to_left: false,
        }
    }
}

fn default_true() -> bool {
    true
}

// 추출기별 옵션을 원천 표에 그대로 펼쳐 담으므로(flatten) 미지 필드 거부는 쓸 수 없다.
#[derive(Debug, Deserialize)]
pub struct Source {
    pub name: String,
    pub version: String,
    /// SPDX 식별자 또는 라이선스 이름 — 고지 문서와 팩 메타데이터에 그대로 나간다.
    pub license: String,
    /// 저작자 표시 문구
    pub attribution: String,
    pub url: String,
    /// 내려받은 파일의 sha256 — 재현성과 무결성의 기준. 원천이 바뀌면 빌드가 멈춘다.
    pub sha256: String,
    /// 이 원천이 팩에 기여하는 방식
    pub role: Role,
    /// 점수 결합 가중치
    #[serde(default = "default_weight")]
    pub weight: f64,
    #[serde(flatten)]
    pub extraction: Extraction,
}

fn default_weight() -> f64 {
    1.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    /// 표제어 집합을 정한다 — 인벤토리 원천이 하나라도 있으면 팩의 표제어는 그 합집합이다.
    Inventory,
    /// 실사용 빈도만 보탠다 — 인벤토리에 없는 낱말은 버린다.
    Frequency,
}

/// 원천 파일에서 (표제어, 신호)를 뽑아내는 방법. 원천 형식마다 하나씩 늘어난다.
#[derive(Debug, Deserialize)]
#[serde(tag = "format", rename_all = "kebab-case")]
pub enum Extraction {
    /// SCOWL 배포본 — `final/<방언>-<범주>.<크기>` 파일들. 크기 등급이 곧 흔함의 등급이다.
    Scowl {
        dialects: Vec<String>,
        categories: Vec<String>,
        /// 포함할 최대 크기 등급 (SCOWL 관례: 10=최상위 1000단어 … 95=희귀)
        maximum_size: u32,
    },
    /// Tatoeba 문장 익스포트 (`식별자<TAB>언어<TAB>문장`, bzip2)
    Tatoeba {
        /// 이 횟수 미만으로 나타난 낱말은 잡음으로 본다.
        #[serde(default = "default_minimum_count")]
        minimum_count: u64,
    },
    /// mecab-ko-dic 형태소 사전 — CSV의 비용(cost)이 낮을수록 흔한 형태소다.
    MecabKoDic {
        /// 표제어를 그대로 취하는 CSV 파일들 (체언·부사·감탄사 등)
        files: Vec<String>,
        /// 어간에 종결어미 `다`를 붙여 기본형으로 만드는 CSV 파일들 (용언)
        #[serde(default)]
        verb_stem_files: Vec<String>,
        /// 조사를 붙여 어절을 만들 상위 체언 수 (0이면 결합형을 만들지 않는다).
        /// 교착어의 어절 전체를 담으면 폭발하므로 흔한 체언에만 붙인다.
        #[serde(default)]
        particle_expansion_nouns: usize,
    },
}

fn default_minimum_count() -> u64 {
    2
}

impl Recipe {
    pub fn load(path: &Path) -> Result<Recipe, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("{} 읽기 실패: {error}", path.display()))?;
        let mut recipe: Recipe = toml::from_str(&text)
            .map_err(|error| format!("{} 해석 실패: {error}", path.display()))?;
        if recipe.sources.is_empty() {
            return Err(format!("{}: 원천이 없음", path.display()));
        }
        // 레시피 안의 경로는 레시피 파일 기준으로 읽는다 — 어디서 실행해도 같게 돈다.
        if let Some(layout) = recipe.layout.take() {
            let directory = path.parent().unwrap_or(Path::new("."));
            recipe.layout = Some(directory.join(layout));
        }
        Ok(recipe)
    }
}
