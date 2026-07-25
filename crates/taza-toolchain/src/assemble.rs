//! 정규화된 점수표 + 레이아웃 + 원천 기록을 언어팩 바이너리로 조립한다.

use crate::lexicon::LexiconBuilder;
use crate::metadata::MetadataBuilder;
use crate::ngram::NgramModelBuilder;
use crate::recipe::{LexiconEncoding, Recipe, Source};
use crate::{PackWriter, layout};
use taza_engine::pack::SectionKind;
use taza_engine::pack::metadata::keys;
use taza_engine::suggest::KeyEncoding;

pub struct AssembledPack {
    pub bytes: Vec<u8>,
    pub word_count: usize,
    pub lexicon_bytes: usize,
    pub bigram_count: usize,
    pub language_model_bytes: usize,
}

/// 표제어를 팩의 저장 인코딩으로 옮긴다. 한글 자모 인코딩에서 분해할 수 없는 표제어는
/// 원천 잡음이므로 버린다 — 몇 개가 빠졌는지는 호출자가 보고한다.
fn encode(word: &str, encoding: LexiconEncoding) -> Option<String> {
    KeyEncoding::from(encoding).encode(word)
}

pub fn assemble(
    recipe: &Recipe,
    sources: &[&Source],
    words: &[(String, u32)],
    bigrams: &[(String, String, u32)],
    affixes: &[String],
    layout_text: Option<&str>,
) -> Result<AssembledPack, String> {
    let mut lexicon = LexiconBuilder::new();
    for (word, score) in words {
        if let Some(encoded) = encode(word, recipe.lexicon.encoding) {
            lexicon.insert(&encoded, *score);
        }
    }
    let word_count = lexicon.word_count();
    if word_count == 0 {
        return Err("표제어가 하나도 남지 않았음".to_string());
    }
    let lexicon_section = lexicon.build();
    let lexicon_bytes = lexicon_section.len();

    // 언어모델 토큰도 lexicon과 같은 조회 키 공간에 있어야 한다
    let mut language_model = NgramModelBuilder::new();
    let mut bigram_count = 0usize;
    for (left, right, weight) in bigrams {
        let (Some(left), Some(right)) = (
            encode(left, recipe.lexicon.encoding),
            encode(right, recipe.lexicon.encoding),
        ) else {
            continue;
        };
        language_model.insert_bigram(&left, &right, *weight);
        bigram_count += 1;
    }
    let language_model_section = (bigram_count > 0).then(|| language_model.build());
    let language_model_bytes = language_model_section.as_ref().map_or(0, Vec::len);

    let mut metadata = MetadataBuilder::new();
    metadata.set(keys::PACK_VERSION, recipe.pack_version.to_string());
    metadata.set(keys::RECIPE, &recipe.name);
    metadata.set(keys::WORD_COUNT, word_count.to_string());
    metadata.set(keys::BIGRAM_COUNT, bigram_count.to_string());
    metadata.set(
        keys::LEXICON_ENCODING,
        KeyEncoding::from(recipe.lexicon.encoding).tag(),
    );
    metadata.set(keys::DISPLAY_NAME, &recipe.display_name);
    metadata.set(keys::KEYCAP_LABEL, &recipe.keycap_label);
    metadata.set(keys::COMPOSER_SKELETON, &recipe.composer_skeleton);
    metadata.set(keys::LAYOUT_NAME, &recipe.layout_name);
    metadata.set(
        keys::WORD_SEPARATED,
        recipe.script.word_separated.to_string(),
    );
    metadata.set(keys::RIGHT_TO_LEFT, recipe.script.right_to_left.to_string());
    if !affixes.is_empty() {
        metadata.set(keys::AFFIXES, affixes.join("\n"));
    }
    metadata.set(keys::SOURCES, source_lines(sources));
    metadata.set(keys::ATTRIBUTION, attribution(sources));

    let mut writer = PackWriter::new(&recipe.language);
    writer.add_section(SectionKind::Lexicon, lexicon_section);
    if let Some(section) = language_model_section {
        writer.add_section(SectionKind::NgramModel, section);
    }
    if let Some(layout_text) = layout_text {
        writer.add_section(
            SectionKind::Layout,
            layout::serialize(&layout::parse(layout_text)?),
        );
    }
    writer.add_section(SectionKind::Metadata, metadata.build());
    Ok(AssembledPack {
        bytes: writer.finish(),
        word_count,
        lexicon_bytes,
        bigram_count,
        language_model_bytes,
    })
}

/// 고지는 실제로 팩에 들어간 원천만 적는다 — 선언해 두었지만 자리에 없어 건너뛴
/// 원천까지 적으면 쓰지도 않은 데이터를 출처로 밝히는 셈이 된다.
pub fn source_lines(sources: &[&Source]) -> String {
    sources
        .iter()
        .map(|source| format!("{} {} ({})", source.name, source.version, source.license))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn attribution(sources: &[&Source]) -> String {
    sources
        .iter()
        .map(|source| source.attribution.clone())
        .collect::<Vec<_>>()
        .join("\n")
}
