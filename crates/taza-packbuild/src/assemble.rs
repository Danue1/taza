//! 정규화된 점수표 + 원천 기록을 언어팩 바이너리로 조립한다.

use crate::recipe::{LexiconEncoding, Recipe, Source};
use taza_corpus::parse::Annotation;
use taza_engine::contract::{CandidateGroup, EmojiCategory};
use taza_engine::pack::SectionKind;
use taza_engine::pack::metadata::keys;
use taza_engine::suggest::KeyEncoding;
use taza_pack::PackWriter;
use taza_pack::section::annotation::{AnnotationBuilder, AnnotationCatalogBuilder};
use taza_pack::section::lexicon::LexiconBuilder;
use taza_pack::section::metadata::MetadataBuilder;
use taza_pack::section::ngram::NgramModelBuilder;

pub struct AssembledPack {
    pub bytes: Vec<u8>,
    pub word_count: usize,
    pub lexicon_bytes: usize,
    pub bigram_count: usize,
    pub language_model_bytes: usize,
    pub annotation_key_count: usize,
    pub annotation_bytes: usize,
    pub catalog_item_count: usize,
}

/// 표제어를 팩의 저장 인코딩으로 옮긴다. 한글 자모 인코딩에서 분해할 수 없는 표제어는
/// 원천 잡음이므로 버린다 — 몇 개가 빠졌는지는 호출자가 보고한다.
fn encode(word: &str, encoding: LexiconEncoding) -> Option<String> {
    KeyEncoding::from(encoding).encode(word)
}

/// 팩 하나를 조립하는 데 필요한 재료 전부. 인자를 여덟 개 늘어놓는 대신 묶는다 —
/// 재료가 하나 더 생길 때 호출부가 전부 흔들리지 않게 하는 값이기도 하다.
pub struct PackInputs<'source> {
    pub recipe: &'source Recipe,
    pub sources: &'source [&'source Source],
    pub words: &'source [(String, u32)],
    pub bigrams: &'source [(String, String, u32)],
    pub annotations: &'source [Annotation],
    pub emoji_order: &'source [(EmojiCategory, String)],
    pub affixes: &'source [String],
}

pub fn assemble(inputs: PackInputs<'_>) -> Result<AssembledPack, String> {
    let PackInputs {
        recipe,
        sources,
        words,
        bigrams,
        annotations,
        emoji_order,
        affixes,
    } = inputs;
    let mut lexicon = LexiconBuilder::new();
    for (word, score) in words {
        if let Some(encoded) = encode(word, recipe.build.lexicon.encoding) {
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
            encode(left, recipe.build.lexicon.encoding),
            encode(right, recipe.build.lexicon.encoding),
        ) else {
            continue;
        };
        language_model.insert_bigram(&left, &right, *weight);
        bigram_count += 1;
    }
    let language_model_section = (bigram_count > 0).then(|| language_model.build());
    let language_model_bytes = language_model_section.as_ref().map_or(0, Vec::len);

    // 곁들일 것의 키도 lexicon과 같은 조회 키 공간에 있어야 지금 치고 있는 어절로 물어볼
    // 수 있다. 표제어가 아닌 낱말에 달린 것은 내놓을 길이 없으므로 담지 않는다.
    let in_lexicon: std::collections::HashSet<&str> =
        words.iter().map(|(word, _)| word.as_str()).collect();
    let mut annotation_table = AnnotationBuilder::new();
    for annotation in annotations {
        if !in_lexicon.contains(annotation.word.as_str()) {
            continue;
        }
        if let Some(encoded) = encode(&annotation.word, recipe.build.lexicon.encoding) {
            annotation_table.insert(&encoded, annotation.group, &annotation.text);
        }
    }
    // 검색하지 않았을 때 보이는 목록은 표(조회 키 순서)에서 만들 수 없다 — 원천에 나온
    // 순서를 갈래별로 따로 싣는다. 표제어 여부는 묻지 않는다: 검색면은 낱말을 치는 자리가
    // 아니라 갈래를 훑는 자리이므로, 사전에 없는 낱말로만 불리는 것도 목록에는 서야 한다.
    // 이모지는 차례를 밝힌 원천(emoji-test)이 있으면 그 묶음·차례로 세운다 — 빌트인
    // 키보드와 같은 자리에 같은 순서로 서야 사람이 찾던 곳에서 찾는다. 그 원천이 없는
    // 팩은 주석 원천에 나온 순서를 쓴다.
    let mut catalog = AnnotationCatalogBuilder::new();
    for category in EmojiCategory::DISPLAY_ORDER {
        for (_, emoji) in emoji_order.iter().filter(|(kept, _)| *kept == category) {
            catalog.insert(CandidateGroup::Emoji, Some(category), emoji);
        }
    }
    for annotation in annotations {
        if annotation.group == CandidateGroup::Emoji && !emoji_order.is_empty() {
            continue;
        }
        catalog.insert(annotation.group, None, &annotation.text);
    }
    let catalog_item_count = catalog.item_count();
    let catalog_section = (catalog_item_count > 0).then(|| catalog.build());

    let annotation_key_count = annotation_table.key_count();
    let annotation_section = (annotation_key_count > 0).then(|| annotation_table.build());
    let annotation_bytes = annotation_section.as_ref().map_or(0, Vec::len);

    let mut metadata = MetadataBuilder::new();
    metadata.set(keys::PACK_VERSION, recipe.pack.pack_version.to_string());
    metadata.set(keys::RECIPE, &recipe.name);
    metadata.set(keys::WORD_COUNT, word_count.to_string());
    metadata.set(keys::BIGRAM_COUNT, bigram_count.to_string());
    metadata.set(
        keys::LEXICON_ENCODING,
        KeyEncoding::from(recipe.build.lexicon.encoding).tag(),
    );
    metadata.set(keys::DISPLAY_NAME, &recipe.pack.display_name);
    metadata.set(keys::KEYCAP_LABEL, &recipe.pack.keycap_label);
    metadata.set(keys::INPUT_METHOD, &recipe.pack.composer_skeleton);
    metadata.set(
        keys::WORD_SEPARATED,
        recipe.pack.script.word_separated.to_string(),
    );
    metadata.set(
        keys::RIGHT_TO_LEFT,
        recipe.pack.script.right_to_left.to_string(),
    );
    if !affixes.is_empty() {
        metadata.set(keys::AFFIXES, affixes.join("\n"));
    }
    metadata.set(keys::SOURCES, source_lines(sources));

    let mut writer = PackWriter::new(&recipe.pack.language);
    writer.add_section(SectionKind::Lexicon, lexicon_section);
    if let Some(section) = language_model_section {
        writer.add_section(SectionKind::NgramModel, section);
    }
    if let Some(section) = annotation_section {
        writer.add_section(SectionKind::Annotation, section);
    }
    if let Some(section) = catalog_section {
        writer.add_section(SectionKind::AnnotationCatalog, section);
    }
    writer.add_section(SectionKind::Metadata, metadata.build());
    Ok(AssembledPack {
        bytes: writer.finish(),
        word_count,
        lexicon_bytes,
        bigram_count,
        language_model_bytes,
        annotation_key_count,
        annotation_bytes,
        catalog_item_count,
    })
}

/// 고지는 실제로 팩에 들어간 원천만 적는다 — 선언해 두었지만 자리에 없어 건너뛴
/// 원천까지 적으면 쓰지도 않은 데이터를 출처로 밝히는 셈이 된다.
/// 원천 하나가 한 줄이고, 그 줄이 자기 이름·판·라이선스·저작자 표시를 모두 갖는다.
/// 와이어 형태는 `taza_engine::pack::metadata::keys::SOURCES` 참조.
pub fn source_lines(sources: &[&Source]) -> String {
    sources
        .iter()
        .map(|source| {
            [
                source.name.as_str(),
                source.version.as_str(),
                source.license.as_str(),
                source.attribution.as_str(),
            ]
            .join("\t")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
