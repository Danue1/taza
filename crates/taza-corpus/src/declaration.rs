//! 원천 파일 하나를 레시피가 적는 표면 — **어디서 구하고 어떻게 읽는가**까지다.
//!
//! 그 원천이 팩에 무엇으로 기여하는지(`role`·`weight`)와 고지에 어떻게 오르는지
//! (이름·판·라이선스)는 여기 없다. 그것은 예산과 원천 목록을 아는 쪽의 몫이고,
//! 이 크레이트는 원천을 신호로 바꾸는 데 필요한 것만 안다.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use taza_engine::contract::CandidateGroup;

/// 원천 파일 하나. 레시피의 원천 표에 그대로 펼쳐 담기므로(flatten) 미지 필드 거부는
/// 쓸 수 없다.
#[derive(Debug, Deserialize)]
pub struct SourceFile {
    #[serde(flatten)]
    pub origin: Origin,
    /// 원천을 구할 수 없을 때 빌드를 멈추지 않고 건너뛴다. 기본값은 조달 방식이 정한다 —
    /// 손으로 갖다 놓는 원천은 아직 없는 것이 정상이고, 자동 조달 원천이 없는 것은 오류다.
    pub optional: Option<bool>,
    #[serde(flatten)]
    pub extraction: Extraction,
}

impl SourceFile {
    /// 원천이 자리에 없을 때 건너뛸 것인가
    pub fn is_optional(&self) -> bool {
        self.optional
            .unwrap_or(matches!(self.origin, Origin::Local { .. }))
    }

    /// 원천을 선언한 파일이 있는 자리를 기준으로 로컬 경로를 푼다
    pub fn resolve_paths(&mut self, directory: &Path) {
        if let Origin::Local { file, .. } = &mut self.origin {
            *file = directory.join(&*file);
        }
    }
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

/// 곁들일 것의 갈래 — 파일 형식이 같고 갈래만 다르므로 레시피가 밝힌다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
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
///
/// 되쓰기(`Serialize`)는 추출 결과 캐시가 이 선언의 지문을 뜨는 데 쓴다 —
/// `source::cache::path` 참조.
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "format", rename_all = "kebab-case")]
pub enum Extraction {
    /// mozc 배포본 — 읽기·표기·비용·연접·단漢字를 한 우산 아래 싣는다. 형태소 분석
    /// 사전과 달리 비용이 **변환 방향으로** 매겨져 있고 읽기가 이미 히라가나다.
    MozcDictionary {
        /// 주 어휘 파일들(`dictionary00.txt`…). 접미사·단漢字·연접·기호는 이름이 정해져
        /// 있으므로 여기 적지 않는다.
        dictionary_files: Vec<String>,
        /// 앞말에 붙는 말로 볼 품사 이름들(助詞·助動詞·接尾 등) — 문절을 가르는 값이다.
        #[serde(default)]
        dependent_tags: Vec<String>,
    },
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
    /// 유니코드 `emoji-test.txt` — 이모지의 묶음과 차례. 낱말은 늘리지 않는다.
    EmojiTest,
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
