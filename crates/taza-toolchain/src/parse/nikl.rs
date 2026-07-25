//! 국립국어원 모두의 말뭉치(JSON).

use super::Signal;
use super::corpus::CorpusCounts;
use crate::source::container;
use std::path::Path;

/// 모두의 말뭉치에서 문장이 들어 있는 배열의 이름. 말뭉치 종류마다 스키마가 조금씩
/// 다르지만(신문은 `paragraph`, 구어는 `utterance`, 분석 말뭉치는 `sentence`) 문장이
/// `form`에 담긴다는 점은 같다. 이 목록에 없는 배열 안의 `form`은 문장이 아니라 형태소·
/// 어절 조각이므로 세지 않는다 — 세면 낱말이 두 번 계산되고 이웃 짝이 무너진다.
const NIKL_SENTENCE_CONTAINERS: [&str; 3] = ["paragraph", "sentence", "utterance"];

/// 국립국어원 모두의 말뭉치(JSON). 이용 신청을 거쳐 손으로 내려받는 원천이라 로컬
/// 조달을 전제하며, 말뭉치 종류가 늘어도 레시피 조각만 더하면 같은 추출기로 들어온다.
pub fn parse(path: &Path, minimum_count: u64) -> Result<Signal, String> {
    let mut corpus = CorpusCounts::default();
    container::for_each_member(path, |name, reader| {
        if !name.ends_with(".json") {
            return Ok(());
        }
        let document: serde_json::Value = serde_json::from_reader(reader)
            .map_err(|error| format!("{name} 해석 실패: {error}"))?;
        read_nikl_value(&document, false, &mut corpus);
        corpus.prune_if_large();
        Ok(())
    })?;
    if corpus.counts.is_empty() {
        return Err(format!("{}: 문장을 읽지 못했음", path.display()));
    }
    Ok(corpus.finish(minimum_count))
}

fn read_nikl_value(value: &serde_json::Value, in_sentences: bool, corpus: &mut CorpusCounts) {
    match value {
        serde_json::Value::Object(fields) => {
            if in_sentences && let Some(serde_json::Value::String(form)) = fields.get("form") {
                // 한 문장 안에서도 문장 부호에서 끊는다 — 통째로 세면 마침표를 건너뛴
                // 짝이 문맥으로 둔갑한다
                for sentence in form.split(['.', '!', '?', '\n']) {
                    corpus.read_sentence(sentence, false);
                }
            }
            for (key, child) in fields {
                read_nikl_value(
                    child,
                    NIKL_SENTENCE_CONTAINERS.contains(&key.as_str()),
                    corpus,
                );
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                read_nikl_value(item, in_sentences, corpus);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 말뭉치 종류마다 문장이 담기는 배열 이름이 다르지만 같은 추출기로 들어와야 한다.
    /// 형태소·어절 조각도 `form`을 쓰므로, 그것까지 세면 낱말이 두 번 계산된다.
    #[test]
    fn nikl_counts_sentences_but_not_morpheme_fragments() {
        let document = serde_json::json!({
            "document": [{
                "paragraph": [{ "form": "밈이 유행한다" }],
                "sentence": [{
                    "form": "유행한다",
                    "morpheme": [{ "form": "유행" }, { "form": "하" }]
                }]
            }],
            "utterance": [{ "form": "밈 좋아" }]
        });
        let mut corpus = CorpusCounts::default();
        read_nikl_value(&document, false, &mut corpus);
        assert_eq!(corpus.counts.get("밈"), Some(&1));
        assert_eq!(corpus.counts.get("밈이"), Some(&1));
        assert_eq!(corpus.counts.get("유행한다"), Some(&2));
        // 형태소 조각은 문장이 아니다
        assert_eq!(corpus.counts.get("유행"), None);
    }

    /// 문서 하나를 통째로 넣으면 마침표를 건너뛴 짝이 문맥으로 둔갑한다.
    #[test]
    fn nikl_breaks_the_neighbour_chain_at_sentence_ends() {
        let document = serde_json::json!({ "paragraph": [{ "form": "앞말이다. 뒷말이다" }] });
        let mut corpus = CorpusCounts::default();
        read_nikl_value(&document, false, &mut corpus);
        assert!(corpus.pairs.is_empty());
    }
}
