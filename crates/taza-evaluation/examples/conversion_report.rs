//! 실팩의 변환 정확도 —
//! `cargo run --release -p taza-evaluation --example conversion_report -- <팩> <mozc 아카이브>`
//!
//! 평가 셋은 사전과 같은 배포본에서 온다(`dictionary_oss/evaluation.tsv`). 회귀 사례
//! 모음이라 어려운 쪽으로 치우쳐 있으므로, 절대값이 아니라 **같은 셋 위의 A/B**로 읽는다.
use std::io::Read;
use taza_engine::convert::Conversion;
use taza_engine::pack::Pack;
use taza_evaluation::conversion::{measure, parse_mozc_evaluation};

fn evaluation_text(archive_path: &str) -> String {
    let file = std::fs::File::open(archive_path).expect("아카이브");
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    for entry in archive.entries().expect("목록") {
        let mut entry = entry.expect("항목");
        let path = entry.path().expect("경로").to_path_buf();
        if path.ends_with("dictionary_oss/evaluation.tsv") {
            let mut text = String::new();
            entry.read_to_string(&mut text).expect("읽기");
            return text;
        }
    }
    panic!("evaluation.tsv를 찾지 못함");
}

fn main() {
    let mut arguments = std::env::args().skip(1);
    let pack_path = arguments.next().expect("팩 경로");
    let archive_path = arguments.next().expect("mozc 아카이브 경로");

    let cases = parse_mozc_evaluation(&evaluation_text(&archive_path));
    let bytes = std::fs::read(&pack_path).expect("팩 읽기");
    let pack = Pack::open(&bytes).expect("팩 열기");
    let conversion = Conversion::new(pack.conversion().expect("변환표"), pack.connection(), None);

    let metrics = measure(&conversion, &cases);
    println!(
        "사례 {} · 문장 {:.3} · 글자 {:.3} · 닿음 {:.3}",
        metrics.cases, metrics.sentence, metrics.character, metrics.reachable
    );

    // 어디서 어긋나는지 눈으로 보는 자리 — 수치만으로는 고칠 곳을 알 수 없다
    let mut wrong = Vec::new();
    for case in &cases {
        let converted: String = conversion
            .convert(&case.reading)
            .iter()
            .map(|segment| segment.surface.as_str())
            .collect();
        if converted != case.expected {
            wrong.push((case.reading.clone(), case.expected.clone(), converted));
        }
    }
    println!("\n틀린 것 {}개 중 앞 12개:", wrong.len());
    for (reading, expected, converted) in wrong.iter().take(12) {
        println!("  {reading}\n    정답 {expected}\n    변환 {converted}");
    }
}
