//! 오프라인 사전 컴파일 파이프라인의 입구.
//! 입력: `단어<TAB>빈도` TSV + 선택적 `앞단어<TAB>뒷단어<TAB>가중치` bigram TSV
//! (빈 줄과 `#` 주석 허용) → 출력: 언어팩 바이너리.

use std::process::ExitCode;
use taza_pack::lexicon::LexiconBuilder;
use taza_pack::ngram::NgramModelBuilder;
use taza_pack::{PackWriter, SectionKind};

fn compile_bigrams(tsv: &str) -> Result<Vec<u8>, String> {
    let mut ngram = NgramModelBuilder::new();
    let mut bigram_count = 0usize;
    for (line_number, line) in tsv.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        let [left, right, weight] = fields.as_slice() else {
            return Err(format!("{}행: `앞단어<TAB>뒷단어<TAB>가중치` 형식이 아님", line_number + 1));
        };
        let weight: u32 = weight
            .trim()
            .parse()
            .map_err(|_| format!("{}행: 가중치가 정수가 아님: {weight:?}", line_number + 1))?;
        ngram.insert_bigram(left, right, weight);
        bigram_count += 1;
    }
    if bigram_count == 0 {
        return Err("bigram 입력에 항목이 없음".to_string());
    }
    Ok(ngram.build())
}

fn compile(language: &str, tsv: &str, bigram_tsv: Option<&str>) -> Result<Vec<u8>, String> {
    let mut lexicon = LexiconBuilder::new();
    let mut word_count = 0usize;
    for (line_number, line) in tsv.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (word, frequency) = line
            .split_once('\t')
            .ok_or_else(|| format!("{}행: 탭 구분자가 없음", line_number + 1))?;
        let frequency: u32 = frequency
            .trim()
            .parse()
            .map_err(|_| format!("{}행: 빈도가 정수가 아님: {frequency:?}", line_number + 1))?;
        if frequency == 0 {
            return Err(format!("{}행: 빈도는 1 이상이어야 함", line_number + 1));
        }
        lexicon.insert(word, frequency);
        word_count += 1;
    }
    if word_count == 0 {
        return Err("입력에 단어가 없음".to_string());
    }
    let mut writer = PackWriter::new(language);
    writer.add_section(SectionKind::Lexicon, lexicon.build());
    if let Some(bigram_tsv) = bigram_tsv {
        writer.add_section(SectionKind::NgramModel, compile_bigrams(bigram_tsv)?);
    }
    Ok(writer.finish())
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().collect();
    let (language, input_path, output_path, bigram_path) = match arguments.as_slice() {
        [_, language, input_path, output_path] => (language, input_path, output_path, None),
        [_, language, input_path, output_path, bigram_path] => {
            (language, input_path, output_path, Some(bigram_path))
        }
        _ => {
            eprintln!(
                "사용법: taza-lexicon-compiler <언어태그> <단어.tsv> <출력.tazapack> [bigram.tsv]"
            );
            return ExitCode::FAILURE;
        }
    };
    let tsv = match std::fs::read_to_string(input_path) {
        Ok(content) => content,
        Err(error) => {
            eprintln!("{input_path} 읽기 실패: {error}");
            return ExitCode::FAILURE;
        }
    };
    let bigram_tsv = match bigram_path {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(content) => Some(content),
            Err(error) => {
                eprintln!("{path} 읽기 실패: {error}");
                return ExitCode::FAILURE;
            }
        },
        None => None,
    };
    let pack = match compile(language, &tsv, bigram_tsv.as_deref()) {
        Ok(pack) => pack,
        Err(message) => {
            eprintln!("컴파일 실패: {message}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = std::fs::write(output_path, &pack) {
        eprintln!("{output_path} 쓰기 실패: {error}");
        return ExitCode::FAILURE;
    }
    println!("{output_path} 작성 완료 ({} bytes)", pack.len());
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::compile;
    use taza_pack::Pack;

    #[test]
    fn compiles_tsv_into_pack() {
        let tsv = "# 주석\nthe\t100\n\ntheme\t40\n";
        let bytes = compile("en", tsv, None).unwrap();
        let pack = Pack::open(&bytes).unwrap();
        assert_eq!(pack.language(), "en");
        let lexicon = pack.lexicon().unwrap();
        assert_eq!(lexicon.frequency("the"), Some(100));
        assert_eq!(lexicon.frequency("theme"), Some(40));
        assert!(pack.language_model().is_none());
    }

    #[test]
    fn compiles_bigrams_when_provided() {
        let bytes = compile("en", "the\t100\nquick\t30\n", Some("the\tquick\t50\n")).unwrap();
        let pack = Pack::open(&bytes).unwrap();
        let language_model = pack.language_model().unwrap();
        let predictions = language_model.predict_next("the", 3);
        assert_eq!(predictions.len(), 1);
        assert_eq!(predictions[0].word, "quick");
    }

    #[test]
    fn rejects_malformed_rows() {
        assert!(compile("en", "the 100", None).is_err());
        assert!(compile("en", "the\tabc", None).is_err());
        assert!(compile("en", "the\t0", None).is_err());
        assert!(compile("en", "", None).is_err());
        assert!(compile("en", "the\t100", Some("the quick 50")).is_err());
        assert!(compile("en", "the\t100", Some("")).is_err());
    }
}
