//! 오프라인 사전 컴파일 파이프라인의 입구.
//! 입력: `단어<TAB>빈도` TSV (빈 줄과 `#` 주석 허용) → 출력: 언어팩 바이너리.

use std::process::ExitCode;
use taza_pack::lexicon::LexiconBuilder;
use taza_pack::{PackWriter, SectionKind};

fn compile(language: &str, tsv: &str) -> Result<Vec<u8>, String> {
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
    Ok(writer.finish())
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().collect();
    let [_, language, input_path, output_path] = arguments.as_slice() else {
        eprintln!("사용법: taza-lexicon-compiler <언어태그> <입력.tsv> <출력.tazapack>");
        return ExitCode::FAILURE;
    };
    let tsv = match std::fs::read_to_string(input_path) {
        Ok(content) => content,
        Err(error) => {
            eprintln!("{input_path} 읽기 실패: {error}");
            return ExitCode::FAILURE;
        }
    };
    let pack = match compile(language, &tsv) {
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
        let bytes = compile("en", tsv).unwrap();
        let pack = Pack::open(&bytes).unwrap();
        assert_eq!(pack.language(), "en");
        let lexicon = pack.lexicon().unwrap();
        assert_eq!(lexicon.frequency("the"), Some(100));
        assert_eq!(lexicon.frequency("theme"), Some(40));
    }

    #[test]
    fn rejects_malformed_rows() {
        assert!(compile("en", "the 100").is_err());
        assert!(compile("en", "the\tabc").is_err());
        assert!(compile("en", "the\t0").is_err());
        assert!(compile("en", "").is_err());
    }
}
