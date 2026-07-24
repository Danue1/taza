//! 오프라인 사전 컴파일 파이프라인의 입구.
//! 입력: `단어<TAB>빈도` TSV, 선택적 bigram TSV(`앞단어<TAB>뒷단어<TAB>가중치`),
//! 선택적 레이아웃 텍스트 — 모두 빈 줄과 `#` 주석 허용 → 출력: 언어팩 바이너리.
//!
//! 레이아웃 문법: 한 줄 = 한 행, 공백 구분 토큰 `표기[:시프트표기][*폭비율]`.
//! 제어 키는 이름으로: `shift`, `backspace`, `space`, `enter`. 기본 폭 0.1.
//! ```text
//! ㅂ:ㅃ ㅈ:ㅉ ㄷ:ㄸ ㄱ:ㄲ ㅅ:ㅆ ㅛ ㅕ ㅑ ㅐ:ㅒ ㅔ:ㅖ
//! shift*0.15 ㅋ ㅌ ㅊ ㅍ ㅠ ㅜ ㅡ backspace*0.15
//! space*0.7 enter*0.3
//! ```

use std::process::ExitCode;
use taza_pack::layout::{KeyAction, KeyboardLayout, LayoutKey, LayoutRow};
use taza_pack::lexicon::LexiconBuilder;
use taza_pack::ngram::NgramModelBuilder;
use taza_pack::{PackWriter, SectionKind};

const DEFAULT_KEY_WIDTH: f32 = 0.1;

fn parse_layout(text: &str) -> Result<KeyboardLayout, String> {
    let mut rows = Vec::new();
    for (line_number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut keys = Vec::new();
        for token in line.split_whitespace() {
            let (specification, width_ratio) = match token.split_once('*') {
                Some((specification, width)) => {
                    let width: f32 = width.parse().map_err(|_| {
                        format!("{}행: 폭 비율이 숫자가 아님: {width:?}", line_number + 1)
                    })?;
                    (specification, width)
                }
                None => (token, DEFAULT_KEY_WIDTH),
            };
            let action = match specification {
                "shift" => KeyAction::Shift,
                "backspace" => KeyAction::Backspace,
                "space" => KeyAction::Space,
                "enter" => KeyAction::Enter,
                characters => {
                    let (base, shifted) = match characters.split_once(':') {
                        Some((base, shifted)) => (base, shifted),
                        None => (characters, characters),
                    };
                    let single = |part: &str| -> Result<char, String> {
                        let mut iterator = part.chars();
                        match (iterator.next(), iterator.next()) {
                            (Some(character), None) => Ok(character),
                            _ => Err(format!(
                                "{}행: 키 표기는 1글자여야 함: {part:?}",
                                line_number + 1
                            )),
                        }
                    };
                    KeyAction::Character {
                        base: single(base)?,
                        shifted: single(shifted)?,
                    }
                }
            };
            keys.push(LayoutKey {
                action,
                width_ratio,
            });
        }
        rows.push(LayoutRow { keys });
    }
    if rows.is_empty() {
        return Err("레이아웃에 행이 없음".to_string());
    }
    Ok(KeyboardLayout { rows })
}

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

fn compile(
    language: &str,
    tsv: &str,
    bigram_tsv: Option<&str>,
    layout_text: Option<&str>,
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
    if let Some(layout_text) = layout_text {
        writer.add_section(
            SectionKind::Layout,
            taza_pack::layout::serialize(&parse_layout(layout_text)?),
        );
    }
    Ok(writer.finish())
}

const USAGE: &str =
    "사용법: taza-lexicon-compiler <언어태그> <단어.tsv> <출력.tazapack> [--bigrams <tsv>] [--layout <txt>]";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().collect();
    let [_, language, input_path, output_path, options @ ..] = arguments.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    let mut bigram_path: Option<&String> = None;
    let mut layout_path: Option<&String> = None;
    let mut option_iterator = options.iter();
    while let Some(option) = option_iterator.next() {
        let value = option_iterator.next();
        match (option.as_str(), value) {
            ("--bigrams", Some(path)) => bigram_path = Some(path),
            ("--layout", Some(path)) => layout_path = Some(path),
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
    let pack = match compile(language, &tsv, bigram_tsv.as_deref(), layout_text.as_deref()) {
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
        let bytes = compile("en", tsv, None, None).unwrap();
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
        let bytes =
            compile("en", "the\t100\nquick\t30\n", Some("the\tquick\t50\n"), None).unwrap();
        let pack = Pack::open(&bytes).unwrap();
        let language_model = pack.language_model().unwrap();
        let predictions = language_model.predict_next("the", 3);
        assert_eq!(predictions.len(), 1);
        assert_eq!(predictions[0].word, "quick");
    }

    #[test]
    fn compiles_layout_when_provided() {
        use taza_pack::layout::KeyAction;
        let layout_text = "ㅂ:ㅃ ㅈ:ㅉ\nshift*0.15 ㅋ backspace*0.15\nspace*0.7 enter*0.3\n";
        let bytes = compile("ko", "안녕\t10\n", None, Some(layout_text)).unwrap();
        let layout = Pack::open(&bytes).unwrap().layout().unwrap();
        assert_eq!(layout.rows.len(), 3);
        assert_eq!(
            layout.rows[0].keys[0].action,
            KeyAction::Character {
                base: 'ㅂ',
                shifted: 'ㅃ'
            }
        );
        assert_eq!(layout.rows[1].keys[0].action, KeyAction::Shift);
        assert!((layout.rows[1].keys[0].width_ratio - 0.15).abs() < 1e-6);
        assert_eq!(
            layout.rows[1].keys[1].action,
            KeyAction::Character {
                base: 'ㅋ',
                shifted: 'ㅋ'
            }
        );
        assert!((layout.rows[2].keys[0].width_ratio - 0.7).abs() < 1e-6);
    }

    #[test]
    fn rejects_malformed_rows() {
        assert!(compile("en", "the 100", None, None).is_err());
        assert!(compile("en", "the\tabc", None, None).is_err());
        assert!(compile("en", "the\t0", None, None).is_err());
        assert!(compile("en", "", None, None).is_err());
        assert!(compile("en", "the\t100", Some("the quick 50"), None).is_err());
        assert!(compile("en", "the\t100", Some(""), None).is_err());
        assert!(compile("en", "the\t100", None, Some("")).is_err());
        assert!(compile("en", "the\t100", None, Some("ab cd")).is_err());
        assert!(compile("en", "the\t100", None, Some("a*x")).is_err());
    }
}
