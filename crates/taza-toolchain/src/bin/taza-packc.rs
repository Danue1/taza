//! 이미 만들어진 점수표를 팩으로 굽는 낮은 층 도구. 원천 조달부터 배포 산출물까지의
//! 전체 파이프라인은 `taza-packs`(레시피 기반)이며, 이 도구는 그 마지막 단계만 손으로
//! 돌려 보고 싶을 때 쓴다.
//!
//! 입력: `단어<TAB>점수` TSV, 선택적 bigram TSV(`앞단어<TAB>뒷단어<TAB>가중치`),
//! 선택적 레이아웃 텍스트 — 모두 빈 줄과 `#` 주석 허용 → 출력: 언어팩 바이너리.
//! 점수는 [1, `MAX_FREQUENCY`]로 정규화된 값이다 (원천 코퍼스의 절대 빈도가 아니다).
//!

use std::process::ExitCode;
use taza_engine::pack::SectionKind;
use taza_engine::pack::lexicon::MAX_FREQUENCY;
use taza_toolchain::lexicon::LexiconBuilder;
use taza_toolchain::ngram::NgramModelBuilder;
use taza_toolchain::{PackWriter, layout};

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
            return Err(format!(
                "{}행: `앞단어<TAB>뒷단어<TAB>가중치` 형식이 아님",
                line_number + 1
            ));
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

/// 한국어 단어를 자모 분해 후 두벌식 ASCII로 인코딩 — trie의 바이트 편집거리가
/// 자모 단위 편집거리가 되도록 하는 저장 형식 (`--hangul-jamo`)
fn encode_hangul_word(word: &str, line_number: usize) -> Result<String, String> {
    use taza_engine::lang::jamo::{decompose_word, encode_jamo_ascii};
    decompose_word(word)
        .and_then(|jamo| encode_jamo_ascii(&jamo))
        .ok_or_else(|| {
            format!(
                "{}행: 한글로 분해할 수 없는 단어: {word:?}",
                line_number + 1
            )
        })
}

fn compile(
    language: &str,
    tsv: &str,
    bigram_tsv: Option<&str>,
    layout_text: Option<&str>,
    hangul_jamo: bool,
) -> Result<Vec<u8>, String> {
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
            .map_err(|_| format!("{}행: 점수가 정수가 아님: {frequency:?}", line_number + 1))?;
        if !(1..=MAX_FREQUENCY).contains(&frequency) {
            return Err(format!(
                "{}행: 점수는 1 이상 {MAX_FREQUENCY} 이하의 정규화된 값이어야 함: {frequency}",
                line_number + 1
            ));
        }
        if hangul_jamo {
            lexicon.insert(&encode_hangul_word(word, line_number)?, frequency);
        } else {
            lexicon.insert(word, frequency);
        }
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
    if let Some(layout_text) = layout_text {
        writer.add_section(
            SectionKind::Layout,
            layout::serialize(&layout::parse(layout_text)?),
        );
    }
    Ok(writer.finish())
}

const USAGE: &str = "사용법: taza-packc <언어태그> <단어.tsv> <출력.tazapack> \
    [--bigrams <tsv>] [--layout <txt>] [--hangul-jamo]";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().collect();
    let [_, language, input_path, output_path, options @ ..] = arguments.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    let mut bigram_path: Option<&String> = None;
    let mut layout_path: Option<&String> = None;
    let mut hangul_jamo = false;
    let mut option_iterator = options.iter();
    while let Some(option) = option_iterator.next() {
        match option.as_str() {
            "--bigrams" => match option_iterator.next() {
                Some(path) => bigram_path = Some(path),
                None => {
                    eprintln!("{USAGE}");
                    return ExitCode::FAILURE;
                }
            },
            "--layout" => match option_iterator.next() {
                Some(path) => layout_path = Some(path),
                None => {
                    eprintln!("{USAGE}");
                    return ExitCode::FAILURE;
                }
            },
            "--hangul-jamo" => hangul_jamo = true,
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::FAILURE;
            }
        }
    }
    let read_file = |path: &String| -> Result<String, ExitCode> {
        std::fs::read_to_string(path).map_err(|error| {
            eprintln!("{path} 읽기 실패: {error}");
            ExitCode::FAILURE
        })
    };
    let tsv = match read_file(input_path) {
        Ok(content) => content,
        Err(code) => return code,
    };
    let bigram_tsv = match bigram_path.map(read_file).transpose() {
        Ok(content) => content,
        Err(code) => return code,
    };
    let layout_text = match layout_path.map(read_file).transpose() {
        Ok(content) => content,
        Err(code) => return code,
    };
    let pack = match compile(
        language,
        &tsv,
        bigram_tsv.as_deref(),
        layout_text.as_deref(),
        hangul_jamo,
    ) {
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
    use taza_engine::pack::Pack;
    use taza_engine::suggest::Dictionary;

    #[test]
    fn compiles_tsv_into_pack() {
        let tsv = "# 주석\nthe\t100\n\ntheme\t40\n";
        let bytes = compile("en", tsv, None, None, false).unwrap();
        let pack = Pack::open(&bytes).unwrap();
        assert_eq!(pack.language(), "en");
        let lexicon = pack.lexicon().unwrap();
        assert_eq!(lexicon.frequency("the"), Some(100));
        assert_eq!(lexicon.frequency("theme"), Some(40));
        assert!(pack.language_model().is_none());
        assert!(pack.layout().is_none());
    }

    #[test]
    fn compiles_bigrams_when_provided() {
        let bytes = compile(
            "en",
            "the\t100\nquick\t30\n",
            Some("the\tquick\t50\n"),
            None,
            false,
        )
        .unwrap();
        let pack = Pack::open(&bytes).unwrap();
        let language_model = pack.language_model().unwrap();
        let predictions = language_model.predict_next("the", 3);
        assert_eq!(predictions.len(), 1);
        assert_eq!(predictions[0].word, "quick");
    }

    #[test]
    fn compiles_layout_when_provided() {
        use taza_engine::pack::layout::KeyAction;
        let layout_text = "ㅂ:ㅃ ㅈ:ㅉ\nshift*0.15 ㅋ backspace*0.15\nlayer1*0.15 space*0.55 enter*0.3\n---\n1 2 3\nlayer0*0.15 space*0.55 enter*0.3\n";
        let bytes = compile("ko", "안녕\t10\n", None, Some(layout_text), false).unwrap();
        let layout_set = Pack::open(&bytes).unwrap().layout().unwrap();
        assert_eq!(layout_set.layers.len(), 2);

        let letters = &layout_set.layers[0];
        assert_eq!(letters.rows.len(), 3);
        assert_eq!(
            letters.rows[0].keys[0].action,
            KeyAction::Character {
                base: 'ㅂ',
                shifted: 'ㅃ'
            }
        );
        assert_eq!(letters.rows[1].keys[0].action, KeyAction::Shift);
        assert!((letters.rows[1].keys[0].width_ratio - 0.15).abs() < 1e-6);
        assert_eq!(
            letters.rows[2].keys[0].action,
            KeyAction::LayerSwitch { target: 1 }
        );

        let symbols = &layout_set.layers[1];
        assert_eq!(
            symbols.rows[1].keys[0].action,
            KeyAction::LayerSwitch { target: 0 }
        );
    }

    #[test]
    fn compiles_language_key_and_alternates() {
        use taza_engine::pack::layout::KeyAction;
        let layout_text = "a[àá] ([[{<] )[]}>]\nlayer1*0.125 language*0.125 space*0.45 enter*0.3\n";
        let bytes = compile("en", "the\t10\n", None, Some(layout_text), false).unwrap();
        let layout_set = Pack::open(&bytes).unwrap().layout().unwrap();
        let letters = &layout_set.layers[0];

        assert_eq!(
            letters.rows[0].keys[0].action,
            KeyAction::Character {
                base: 'a',
                shifted: 'a'
            }
        );
        assert_eq!(letters.rows[0].keys[0].alternates, vec!['à', 'á']);
        // 대괄호 자체가 키인 경우와 변형 표기가 섞이지 않는다
        assert_eq!(letters.rows[0].keys[1].alternates, vec!['[', '{', '<']);
        assert_eq!(letters.rows[0].keys[2].alternates, vec![']', '}', '>']);

        assert_eq!(letters.rows[1].keys[1].action, KeyAction::LanguageSwitch);
        assert!(letters.rows[1].keys[1].alternates.is_empty());
    }

    #[test]
    fn hangul_jamo_mode_encodes_words() {
        let bytes = compile("ko", "안녕\t90\n안내\t50\n", None, None, true).unwrap();
        let pack = Pack::open(&bytes).unwrap();
        let lexicon = pack.lexicon().unwrap();
        // 안녕 = ㅇㅏㄴㄴㅕㅇ = dkssud
        assert_eq!(lexicon.frequency("dkssud"), Some(90));
        assert_eq!(
            lexicon
                .search(
                    &taza_engine::suggest::Query {
                        key: "dkss",
                        max_cost: 0,
                        touches: &[],
                        extending: true,
                    },
                    10
                )
                .len(),
            2
        );
        assert!(compile("ko", "hello\t10\n", None, None, true).is_err());
    }

    #[test]
    fn rejects_malformed_rows() {
        assert!(compile("en", "the 100", None, None, false).is_err());
        assert!(compile("en", "the\tabc", None, None, false).is_err());
        assert!(compile("en", "the\t0", None, None, false).is_err());
        assert!(compile("en", "", None, None, false).is_err());
        assert!(compile("en", "the\t100", Some("the quick 50"), None, false).is_err());
        assert!(compile("en", "the\t100", Some(""), None, false).is_err());
        assert!(compile("en", "the\t100", None, Some(""), false).is_err());
        assert!(compile("en", "the\t100", None, Some("ab cd"), false).is_err());
        assert!(compile("en", "the\t100", None, Some("a*x"), false).is_err());
    }
}
