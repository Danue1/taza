//! 언어팩을 만드는 명령. 서브커맨드 하나가 파이프라인의 어느 지점에서 시작하는지를 뜻한다.
//!
//! ```text
//! taza build   [언어…]           레시피에서 조달·정규화·조립·배포까지
//! taza compile <언어태그> …      이미 만들어진 점수표를 팩으로만
//! ```
//!
//! 기기에 나가는 라이브러리의 라이선스 고지는 이 명령에 없다 — 그것은 팩이 아니라
//! 의존성 그래프를 읽는 일이라 파이프라인을 의존하지 않는 `taza-licenses`가 맡는다.

mod build;
mod compile;

use std::process::ExitCode;

const USAGE: &str = "사용법: taza <build|compile> …";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [command, rest @ ..] = arguments.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    match command.as_str() {
        "build" => build::main(rest),
        "compile" => compile::main(rest),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}
