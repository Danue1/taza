//! 언어별 팩 레시피 — 원천이 무엇이고 어떻게 팩으로 가공되는지의 단일 선언.
//! 언어 추가는 이 파일을 하나 더 쓰는 작업이 되도록 설계했다.

use serde::Deserialize;
use std::path::{Path, PathBuf};
use taza_engine::contract::CandidateGroup;
use taza_engine::suggest::KeyEncoding;

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
    /// 원천 조각 디렉터리 (레시피 파일 기준 상대 경로). 이 디렉터리의 `*.toml`을
    /// 이름순으로 읽어 원천 목록 뒤에 잇는다 — 말뭉치가 늘어날 때 레시피 본문을
    /// 건드리지 않고 파일 하나를 떨구면 되도록.
    pub source_directory: Option<PathBuf>,
    #[serde(default)]
    pub sources: Vec<Source>,
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
    #[serde(flatten)]
    pub origin: Origin,
    /// 이 원천이 팩에 기여하는 방식
    pub role: Role,
    /// 점수 결합 가중치
    #[serde(default = "default_weight")]
    pub weight: f64,
    /// 원천을 구할 수 없을 때 빌드를 멈추지 않고 건너뛴다. 기본값은 조달 방식이 정한다 —
    /// 손으로 갖다 놓는 원천은 아직 없는 것이 정상이고, 자동 조달 원천이 없는 것은 오류다.
    pub optional: Option<bool>,
    #[serde(flatten)]
    pub extraction: Extraction,
}

/// 원천 파일을 어디서 구하는가.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Origin {
    /// 자동 조달 — 내려받아 sha256으로 검증하고 캐시에 남긴다.
    Remote {
        url: String,
        /// 내려받은 파일의 sha256 — 재현성과 무결성의 기준. 원천이 바뀌면 빌드가 멈춘다.
        sha256: String,
    },
    /// 손으로 갖다 놓는 원천 — 로그인·이용 신청을 거쳐야 해서 URL로 받을 수 없는 말뭉치
    /// (모두의 말뭉치, 우리말샘)가 여기 들어온다. 경로는 이 원천을 선언한 파일 기준이다.
    Local {
        file: PathBuf,
        /// 있으면 검증한다. 손으로 받은 판은 사람마다 다를 수 있어 필수로 두지 않는다.
        sha256: Option<String>,
    },
}

impl Source {
    /// 원천이 자리에 없을 때 건너뛸 것인가
    pub fn is_optional(&self) -> bool {
        self.optional
            .unwrap_or(matches!(self.origin, Origin::Local { .. }))
    }

    /// 원천을 선언한 파일이 있는 자리를 기준으로 로컬 경로를 푼다
    fn resolve_paths(&mut self, directory: &Path) {
        if let Origin::Local { file, .. } = &mut self.origin {
            *file = directory.join(&*file);
        }
    }
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

/// 곁들일 것의 갈래 — 파일 형식이 같고 갈래만 다르므로 레시피가 밝힌다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AnnotationGroupName {
    Emoji,
    Symbol,
    Emoticon,
}

impl From<AnnotationGroupName> for CandidateGroup {
    fn from(name: AnnotationGroupName) -> Self {
        match name {
            AnnotationGroupName::Emoji => CandidateGroup::Emoji,
            AnnotationGroupName::Symbol => CandidateGroup::Symbol,
            AnnotationGroupName::Emoticon => CandidateGroup::Emoticon,
        }
    }
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
    /// MediaWiki XML 덤프 (`<page>` … `<text>`, bzip2). 문어체로 기울어 있으므로
    /// 구어체 원천과 함께 쓰고 weight로 균형을 잡는다.
    Wikipedia {
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
        /// 활용형 CSV — 표층형이 어간+어미로 분석돼 있는 항목들. 규칙 활용은 코퍼스가
        /// 보여 주지만 축약·불규칙("재밌어", "몰라", "예뻐")은 그 형태가 코퍼스에
        /// 나타나지 않으면 어디서도 알 수 없다. 사전이 이미 적어 둔 것을 받는다.
        ///
        /// 이 파일에는 조사 결합 분석도 섞여 있으므로(`가` = JKS+NP) 용언 활용만 받는다.
        #[serde(default)]
        inflection_files: Vec<String>,
        /// 조사를 붙여 어절을 만들 상위 체언 수 (0이면 결합형을 만들지 않는다).
        /// 교착어의 어절 전체를 담으면 폭발하므로 흔한 체언에만 붙인다.
        #[serde(default)]
        particle_expansion_nouns: usize,
    },
    /// 국립국어원 모두의 말뭉치 — 말뭉치 종류마다 JSON 스키마가 조금씩 다르지만
    /// 문장이 `form` 필드에 담긴다는 점은 같다. 신문·구어·메신저·웹이 같은 추출기로
    /// 들어오므로, 말뭉치가 늘어날 때 조각 파일만 더하면 된다.
    NiklCorpus {
        #[serde(default = "default_minimum_count")]
        minimum_count: u64,
    },
    /// CLDR 주석 — 이모지·기호마다 그것을 부르는 낱말이 달려 있다. 갈래(이모지/기호)는
    /// 코드포인트가 이모지 표현인지로 갈리므로 레시피가 밝히지 않는다.
    CldrAnnotations,
    /// 곁들일 것 목록 (`곁들일것<TAB>낱말,낱말,…`). 공개 원천이 없는 갈래(얼굴 문자)를
    /// 손으로 갖춰 싣는 통로다. `#`로 시작하는 줄은 주석이다.
    AnnotationList {
        /// 이 파일이 담은 갈래
        group: AnnotationGroupName,
    },
    /// 우리말샘 사전 XML. 110만 표제어 가운데 방언·북한어·옛말이 상당수라 그대로 받으면
    /// 표준 어휘를 예산에서 밀어낸다 — 사전이 스스로 구분해 둔 갈래로 고른다.
    Urimalsam {
        /// 받을 뜻풀이 갈래 (`senseInfo/type`) — 보통 `일반어`만
        sense_types: Vec<String>,
        /// 받을 표제어 단위 (`wordInfo/word_unit`) — 속담·관용구는 문장이라 어절 사전에
        /// 들어갈 자리가 없다
        word_units: Vec<String>,
        /// 표제어로 두지 않을 품사. 조사·어미·접사는 홀로 쓰이는 어절이 아니다.
        #[serde(default = "default_excluded_parts_of_speech")]
        excluded_parts_of_speech: Vec<String>,
        /// 사전에는 빈도가 없다. 모든 표제어가 같은 등급을 받고, 예산 안에 들어갈 순위는
        /// 코퍼스가 정한다.
        #[serde(default = "default_word_list_rank")]
        rank: f64,
    },
    /// 줄 단위 낱말 목록 (`낱말` 또는 `낱말<TAB>빈도`). 형식이 제각각인 사전·용어집·
    /// 신어 자료를 사람이 한 번 이 꼴로 뽑아 두면 그대로 흡수된다 — 원천마다 파서를
    /// 늘리지 않고 새 어휘를 들이는 통로다. `.zip`·`.gz`·평문 모두 읽는다.
    WordList {
        /// 빈도 열이 없을 때 낱말에 매기는 흔함 등급 (인벤토리 역할에서 쓰인다)
        #[serde(default = "default_word_list_rank")]
        rank: f64,
        /// 빈도 열이 있을 때, 이 횟수 미만은 잡음으로 본다
        #[serde(default = "default_minimum_count")]
        minimum_count: u64,
    },
}

fn default_minimum_count() -> u64 {
    2
}

fn default_word_list_rank() -> f64 {
    0.5
}

fn default_excluded_parts_of_speech() -> Vec<String> {
    ["조사", "어미", "접사"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

impl Recipe {
    pub fn load(path: &Path) -> Result<Recipe, String> {
        let directory = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("{} 읽기 실패: {error}", path.display()))?;
        let mut recipe: Recipe = toml::from_str(&text)
            .map_err(|error| format!("{} 해석 실패: {error}", path.display()))?;
        // 레시피 안의 경로는 그것을 적은 파일 기준으로 읽는다 — 어디서 실행해도 같게 돈다.
        for source in &mut recipe.sources {
            source.resolve_paths(&directory);
        }
        if let Some(layout) = recipe.layout.take() {
            recipe.layout = Some(directory.join(layout));
        }
        if let Some(fragments) = recipe.source_directory.take() {
            let fragments = directory.join(fragments);
            recipe.sources.extend(load_fragments(&fragments)?);
            recipe.source_directory = Some(fragments);
        }
        if recipe.sources.is_empty() {
            return Err(format!("{}: 원천이 없음", path.display()));
        }
        Ok(recipe)
    }
}

/// 조각 디렉터리의 `*.toml`을 이름순으로 읽어 원천 목록을 잇는다. 디렉터리가 아직
/// 없으면 원천이 없는 것으로 본다 — 조각을 하나도 두지 않은 레시피도 그대로 빌드된다.
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
            source.resolve_paths(directory);
            sources.push(source);
        }
    }
    Ok(sources)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 조각 디렉터리가 원천 목록을 잇고, 조각 안의 로컬 경로는 조각 파일 기준으로 풀린다.
    #[test]
    fn source_fragments_extend_the_recipe() {
        let directory = std::env::temp_dir().join("taza-recipe-fragments");
        let fragments = directory.join("sample.sources.d");
        std::fs::create_dir_all(&fragments).unwrap();
        std::fs::write(
            directory.join("sample.toml"),
            r#"
name = "sample"
language = "ko"
display_name = "표본"
keycap_label = "표"
composer_skeleton = "hangul"
layout_name = "두벌식"
pack_version = 1
source_directory = "sample.sources.d"

[lexicon]
max_words = 10

[lexicon.admission]
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

        let recipe = Recipe::load(&directory.join("sample.toml")).unwrap();
        let [source] = recipe.sources.as_slice() else {
            panic!("원천이 하나여야 함: {}", recipe.sources.len());
        };
        assert_eq!(source.role, Role::Discovery);
        // 손으로 받는 원천은 아직 없는 것이 정상이므로 기본이 선택이다
        assert!(source.is_optional());
        match &source.origin {
            Origin::Local { file, .. } => assert_eq!(file, &fragments.join("corpus.zip")),
            other => panic!("로컬 원천이어야 함: {other:?}"),
        }
        let admission = recipe.lexicon.admission.unwrap();
        assert_eq!(admission.minimum_count, 50);
        assert_eq!(admission.minimum_sources, 1);

        std::fs::remove_dir_all(&directory).unwrap();
    }
}
