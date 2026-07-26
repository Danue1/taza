#!/bin/bash
# taza-ffi를 기기·시뮬레이터용 정적 라이브러리로 빌드하고 Swift 바인딩과 함께
# XCFramework로 묶는다. FFI 계약(crates/taza-ffi/src/lib.rs)이 바뀌면 다시 돌린다.
set -euo pipefail

script_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repository_root="$(cd "${script_directory}/../.." && pwd)"
cd "${repository_root}"

profile=release
library_name=libtaza_ffi.a

# 의존성이 바뀌면 고지도 함께 바뀌어야 한다 — 목록을 손으로 맞추지 않는다
cargo run --release -q -p taza-toolchain --bin taza-licenses

cargo build --release -p taza-ffi --target aarch64-apple-ios
cargo build --release -p taza-ffi --target aarch64-apple-ios-sim

# Swift 바인딩 생성 (uniffi proc-macro 모드 — 라이브러리에서 메타데이터를 읽는다)
generated="${script_directory}/Generated"
rm -rf "${generated}"
mkdir -p "${generated}"
cargo run --release -p taza-ffi --features cli --bin uniffi-bindgen -- generate \
    --library "target/aarch64-apple-ios-sim/${profile}/libtaza_ffi.dylib" \
    --language swift \
    --out-dir "${generated}"

# 모듈맵 이름은 XCFramework 헤더 디렉터리 규칙에 맞춘다
mv "${generated}/taza_ffiFFI.modulemap" "${generated}/module.modulemap" 2>/dev/null || true

framework="${script_directory}/TazaFfi.xcframework"
rm -rf "${framework}"
for slice in aarch64-apple-ios aarch64-apple-ios-sim; do
    headers="${script_directory}/.headers-${slice}"
    rm -rf "${headers}"
    mkdir -p "${headers}"
    cp "${generated}/taza_ffiFFI.h" "${headers}/"
    cp "${generated}/module.modulemap" "${headers}/"
done

xcodebuild -create-xcframework \
    -library "target/aarch64-apple-ios/${profile}/${library_name}" \
    -headers "${script_directory}/.headers-aarch64-apple-ios" \
    -library "target/aarch64-apple-ios-sim/${profile}/${library_name}" \
    -headers "${script_directory}/.headers-aarch64-apple-ios-sim" \
    -output "${framework}"

rm -rf "${script_directory}/.headers-aarch64-apple-ios" \
       "${script_directory}/.headers-aarch64-apple-ios-sim"
# 바인딩 디렉터리에는 Swift 소스만 남긴다 (헤더·모듈맵은 XCFramework 안에 있다)
rm -f "${generated}/taza_ffiFFI.h" "${generated}/module.modulemap"

echo "XCFramework 갱신 완료: ${framework}"
