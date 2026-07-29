//! 실팩으로 변환을 재 보는 자리 — `cargo run -p taza-engine --example convert_probe -- <팩> [읽기…]`
//!
//! 팩을 **mmap으로 연다** — 기기에서도 그렇게 열기 때문이다. 통째로 읽어 들이면 파일
//! 크기가 그대로 상주 메모리가 되어, 이 도구로 재는 값이 실제와 달라진다.
use std::time::Instant;
use taza_engine::convert::Conversion;
use taza_engine::pack::Pack;

fn main() {
    let path = std::env::args().nth(1).expect("팩 경로");
    let file = std::fs::File::open(&path).expect("팩 열기");
    let mapped = unsafe { memmap2::Mmap::map(&file) }.expect("mmap");
    let pack = Pack::open(&mapped).expect("팩 읽기");
    let table = pack.conversion().expect("변환표");
    let matrix = pack.connection();
    println!(
        "연접 표: {:?}",
        matrix.map(|m| (m.row_count(), m.column_count()))
    );
    let conversion = Conversion::new(table, matrix, None);

    for reading in [
        "きしゃのきしゃがきしゃできしゃした",
        "にわにはにわにわとりがいる",
        "きょうはいいてんきですね",
        "にほんごにゅうりょくをためす",
        "とうきょうとにすんでいます",
        "このほんをよんでください",
    ] {
        let start = Instant::now();
        let segments = conversion.convert(reading);
        let elapsed = start.elapsed();
        let surface: String = segments.iter().map(|s| s.surface.as_str()).collect();
        let split: Vec<&str> = segments.iter().map(|s| s.surface.as_str()).collect();
        println!("{reading}\n  → {surface}   문절 {split:?}  ({:?})", elapsed);
    }
    // 평가 셋만큼 돌려 실제로 만져지는 쪽수를 늘려 본다 — 상주 메모리는 이 뒤에 잰다
    if let Ok(text) = std::fs::read_to_string("data/languages/japanese/evaluation.tsv") {
        let start = Instant::now();
        let mut count = 0usize;
        for line in text.lines().filter(|line| !line.starts_with('#')) {
            if let Some(reading) = line.split('\t').nth(1) {
                conversion.convert(reading);
                count += 1;
            }
        }
        println!("\n평가 셋 {count}문장 변환: {:?}", start.elapsed());
    }
    for reading in ["きしゃ", "こうしょう", "あい"] {
        let candidates = conversion.candidates(reading);
        println!("{reading}: {:?}", &candidates[..candidates.len().min(8)]);
    }
}
