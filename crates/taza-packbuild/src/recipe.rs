//! 언어별 팩 레시피 — 원천이 무엇이고 어떻게 팩으로 가공되는지의 단일 선언.
//! 언어 추가는 이 파일을 하나 더 쓰는 작업이 되도록 설계했다.
//!
//! 레시피는 종류가 다른 두 가지를 적는다. `[pack]`은 **기기에 나가는 것**이다 — 팩
//! 메타데이터에 실려 셸이 읽는다. `[build]`는 **여기서만 쓰는 손잡이**다 — 예산과 문턱이라
//! 값을 바꾸면 팩의 내용이 달라지지만, 그 값 자체는 기기에 나가지 않는다.
//!
//! 나누어 둔 값은 표시 이름을 다듬는 일이 어휘 예산과 무관함을 타입으로 말해 준다.
//!
//! 언어 하나가 디렉터리 하나다:
//! ```text
//! data/languages/<이름>/recipe.toml     이 파일
//! data/languages/<이름>/sources/*.toml  원천 조각, 이름순
//! ```
//! 원천을 조각으로만 받는 이유는 말뭉치가 계속 늘기 때문이다 — 새 말뭉치를 들이는 일이
//! 파일 하나를 떨구는 일이어야 하고, 그 길이 둘이면 어느 쪽에 적혔는지를 늘 확인해야 한다.

use serde::Deserialize;
use std::path::{Path, PathBuf};
use taza_corpus::SourceFile;
use taza_engine::suggest::KeyEncoding;

pub struct Recipe {
    /// 팩 파일 이름과 카탈로그 식별자 (`english` → `english.tazapack`). 언어 디렉터리의
    /// 이름이 그대로 쓰인다 — 두 자리에 적으면 둘이 어긋날 수 있다.
    pub name: String,
    pub pack: PackIdentity,
    pub build: BuildRules,
    pub sources: Vec<Source>,
}

/// `recipe.toml`이 담는 것. 원천과 이름은 파일이 놓인 자리가 말해 준다.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecipeFile {
    pack: PackIdentity,
    #[serde(default)]
    build: BuildRules,
}

/// 팩이 스스로 밝히는 것 — 전부 팩 메타데이터에 실려 기기로 나간다. 코어와 셸은
/// 언어별 표를 따로 두지 않고 이것만 읽는다.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackIdentity {
    /// 팩 헤더에 담기는 언어 태그 (`en`, `ko`)
    pub language: String,
    /// 언어가 자기를 부르는 이름 — 스페이스바와 언어 목록에 그대로 나간다
    pub display_name: String,
    /// 언어 키에 찍히는 짧은 표기
    pub keycap_label: String,
    /// 입력 방식 — 이 언어를 어느 방식으로 칠 것인가
    pub composer_skeleton: String,
    /// 데이터 판 번호 — 같은 언어의 갱신 배포를 구분한다. 원천·규칙을 바꾸면 올린다.
    pub pack_version: u32,
    #[serde(default)]
    pub script: ScriptTraits,
}

/// 원천에서 팩까지 오는 길의 손잡이 — 예산과 문턱. 값은 기기에 나가지 않고, 그 값이
/// 고른 결과만 나간다.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildRules {
    #[serde(default)]
    pub lexicon: LexiconRules,
    #[serde(default)]
    pub language_model: LanguageModelRules,
}

/// 원천 조각 파일의 내용 — `[[sources]]` 배열만 담는다.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceFragment {
    sources: Vec<Source>,
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
    /// 인벤토리에 없더라도 코퍼스에서 관측된 활용형을 표제어로 받아들일지.
    ///
    /// 교착어에서는 사전에 기본형("있다")만 있고 사람이 실제로 치는 것은 활용형
    /// ("있어", "있는", "있을")이다. 활용 규칙을 코드로 생성하려면 불규칙까지 다뤄야
    /// 하지만, 무엇이 실제로 쓰이는지는 코퍼스가 이미 알고 있다. 인벤토리가 준 어간으로
    /// 시작하는 것만 받아 고유명사·오타가 섞이는 길을 막는다.
    #[serde(default)]
    pub accept_inflections: bool,
    /// 인벤토리 밖 낱말을 코퍼스 증거만으로 표제어로 올리는 조건. 없으면 올리지 않는다.
    pub admission: Option<AdmissionRules>,
}

/// 사전에 없는 낱말을 코퍼스가 표제어로 밀어 넣는 문턱.
///
/// 외래어와 신어에서는 사전이 늘 뒤늦다 — 형태소 사전은 2018년판이고 사람이 오늘 치는
/// 말("유튜브", "밈", "구독자")은 거기에 없다. 무엇이 실제로 쓰이는지는 코퍼스가 이미
/// 알고 있으므로, 증거가 충분한 낱말은 인벤토리 없이도 받는다. 아래 조건들은 그 증거의
/// 문턱이며, 함께 들어오려는 잡음(오타·인명·지명·외국어 음차)을 막는 자리다.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdmissionRules {
    /// 승격에 필요한 최소 관측 횟수 — discovery 원천들의 합(가중치 적용 전)
    pub minimum_count: u64,
    /// 서로 다른 discovery 원천 몇 곳에서 보여야 하는가. 한 곳만 보고 올리면 그 원천의
    /// 편향이 그대로 사전이 된다 — 위키백과만 쓰면 인명·지명이, 메신저 말뭉치만 쓰면
    /// 오타가 표제어가 된다.
    #[serde(default = "default_minimum_sources")]
    pub minimum_sources: usize,
    /// 승격 표제어 수 상한. 예산은 사전 표제어와 공유하므로 이 값이 곧 "새 말에
    /// 얼마나 자리를 내줄 것인가"의 손잡이다.
    pub maximum: usize,
}

fn default_minimum_sources() -> usize {
    1
}

impl Default for LexiconRules {
    fn default() -> Self {
        LexiconRules {
            encoding: LexiconEncoding::default(),
            character_set: CharacterSet::default(),
            max_words: 100_000,
            minimum_word_length: default_minimum_word_length(),
            accept_inflections: false,
            admission: None,
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
    /// 히라가나 정규형 — 되돌릴 수 없는 유일한 인코딩이라 표기는 변환표가 따로 갖는다
    Kana,
}

impl From<LexiconEncoding> for KeyEncoding {
    fn from(encoding: LexiconEncoding) -> Self {
        match encoding {
            LexiconEncoding::Utf8 => KeyEncoding::Utf8,
            LexiconEncoding::HangulJamoDubeolsik => KeyEncoding::HangulJamoDubeolsik,
            LexiconEncoding::Kana => KeyEncoding::Kana,
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
    /// 가나만 — 조회 키가 서는 자리다. 표기(한자·가타카나)는 변환표가 따로 갖는다.
    Kana,
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
            CharacterSet::Kana => {
                !word.is_empty()
                    && taza_engine::suggest::KeyEncoding::Kana
                        .encode(word)
                        .is_some()
            }
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

/// 원천 하나가 팩에 오르는 조건 전부 — 파일을 어디서 어떻게 읽는지는 이 표의 나머지에
/// 그대로 펼쳐 담긴다(flatten). 그래서 미지 필드 거부는 쓸 수 없다.
#[derive(Debug, Deserialize)]
pub struct Source {
    pub name: String,
    pub version: String,
    /// SPDX 식별자 또는 라이선스 이름 — 고지 문서와 팩 메타데이터에 그대로 나간다.
    pub license: String,
    /// 저작자 표시 문구
    pub attribution: String,
    /// 이 원천이 팩에 기여하는 방식
    pub role: Role,
    /// 점수 결합 가중치
    #[serde(default = "default_weight")]
    pub weight: f64,
    /// 파일을 어디서 구하고 어떻게 읽는가 — `taza-corpus`의 몫이다
    #[serde(flatten)]
    pub file: SourceFile,
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
    /// 빈도를 보태면서, 증거가 충분하면 인벤토리에 없는 낱말도 표제어로 올린다.
    /// 사전이 아직 싣지 않은 외래어·신어가 팩에 들어오는 유일한 통로다
    /// (`[lexicon.admission]`이 그 문턱을 정한다).
    Discovery,
}

impl Recipe {
    /// 언어 디렉터리 하나를 읽는다 — `recipe.toml`과 그 옆의 `sources/`.
    pub fn load(directory: &Path) -> Result<Recipe, String> {
        let name = directory
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("{}: 언어 이름을 읽을 수 없음", directory.display()))?
            .to_string();
        let path = directory.join("recipe.toml");
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("{} 읽기 실패: {error}", path.display()))?;
        let file: RecipeFile = toml::from_str(&text)
            .map_err(|error| format!("{} 해석 실패: {error}", path.display()))?;
        let sources = load_fragments(&directory.join("sources"))?;
        if sources.is_empty() {
            return Err(format!("{}: 원천이 없음", directory.display()));
        }
        Ok(Recipe {
            name,
            pack: file.pack,
            build: file.build,
            sources,
        })
    }
}

/// 조각 디렉터리의 `*.toml`을 이름순으로 읽어 원천 목록을 만든다. 이름순인 이유는
/// 원천의 차례가 결과에 남기 때문이다(이모지 목록의 순서 등) — 파일 이름의 숫자 앞머리가
/// 그 차례를 사람이 정하는 자리다.
fn load_fragments(directory: &Path) -> Result<Vec<Source>, String> {
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut paths: Vec<PathBuf> = std::fs::read_dir(directory)
        .map_err(|error| format!("{} 읽기 실패: {error}", directory.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "toml")
        })
        .collect();
    paths.sort();

    let mut sources: Vec<Source> = Vec::new();
    for path in paths {
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("{} 읽기 실패: {error}", path.display()))?;
        let fragment: SourceFragment = toml::from_str(&text)
            .map_err(|error| format!("{} 해석 실패: {error}", path.display()))?;
        for mut source in fragment.sources {
            source.file.resolve_paths(directory);
            sources.push(source);
        }
    }
    Ok(sources)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 언어 디렉터리 하나가 레시피와 원천 조각으로 읽히고, 조각 안의 로컬 경로는 조각
    /// 파일 기준으로 풀린다. 팩 이름은 디렉터리가 준다.
    #[test]
    fn a_language_directory_becomes_a_recipe() {
        let directory = std::env::temp_dir().join("taza-recipe-fragments/sample");
        let fragments = directory.join("sources");
        std::fs::create_dir_all(&fragments).unwrap();
        std::fs::write(
            directory.join("recipe.toml"),
            r#"
[pack]
language = "ko"
display_name = "표본"
keycap_label = "표"
composer_skeleton = "hangul"
pack_version = 1

[build.lexicon]
max_words = 10

[build.lexicon.admission]
minimum_count = 50
maximum = 100
"#,
        )
        .unwrap();
        std::fs::write(
            fragments.join("10-local.toml"),
            r#"
[[sources]]
name = "손으로 받는 말뭉치"
version = "2026"
license = "KOGL Type 1"
attribution = "출처 표시"
file = "corpus.zip"
role = "discovery"
format = "nikl-corpus"
"#,
        )
        .unwrap();

        let recipe = Recipe::load(&directory).unwrap();
        assert_eq!(recipe.name, "sample");
        let [source] = recipe.sources.as_slice() else {
            panic!("원천이 하나여야 함: {}", recipe.sources.len());
        };
        assert_eq!(source.role, Role::Discovery);
        // 손으로 받는 원천은 아직 없는 것이 정상이므로 기본이 선택이다
        assert!(source.file.is_optional());
        match &source.file.origin {
            taza_corpus::Origin::Local { file, .. } => {
                assert_eq!(file, &fragments.join("corpus.zip"))
            }
            other => panic!("로컬 원천이어야 함: {other:?}"),
        }
        let admission = recipe.build.lexicon.admission.unwrap();
        assert_eq!(admission.minimum_count, 50);
        assert_eq!(admission.minimum_sources, 1);

        std::fs::remove_dir_all(&directory).unwrap();
    }
}
