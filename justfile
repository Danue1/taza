# 저장소 뿌리에서 도는 명령들. 손으로 외워 치던 순서를 여기에 적어 둔다 —
# 코어(Rust)가 시뮬레이터의 키보드까지 닿으려면 거쳐야 하는 단계가 정해져 있고,
# 그중 하나를 빠뜨리면 고친 것이 반영되지 않은 채 옛 동작을 보게 되기 때문이다.

# 띄울 시뮬레이터. 다른 기종을 볼 때는 `just device="iPad Pro 13-inch (M4)" simulator`.
device := "iPhone 17 Pro"

# Xcode 산출물 자리. 저장소 안에 두어 파생 데이터를 지울 때 이 한 칸만 지우면 된다.
derived := "platforms/ios/.build"

application := "io.danuel.taza.TazaApp"

default:
    @just --list

# 고친 코어를 시뮬레이터의 키보드에 반영한다.
#
# 익스텐션은 앱 번들 안에 실려 나가므로 코어만 다시 지어서는 바뀌지 않는다 —
# XCFramework를 새로 묶고, 앱을 다시 지어 설치해야 익스텐션이 새 코어를 연다.
simulator: xcframework
    xcodebuild -project platforms/ios/Taza.xcodeproj \
        -scheme TazaApp \
        -configuration Debug \
        -destination 'platform=iOS Simulator,name={{ device }}' \
        -derivedDataPath {{ derived }} \
        build
    # 이미 켜져 있으면 그 상태 그대로 쓴다
    xcrun simctl boot "{{ device }}" || true
    open -a Simulator
    xcrun simctl install "{{ device }}" \
        "{{ derived }}/Build/Products/Debug-iphonesimulator/TazaApp.app"
    xcrun simctl launch "{{ device }}" {{ application }}

# 코어를 기기·시뮬레이터용으로 지어 Swift 바인딩과 함께 묶는다.
xcframework:
    ./platforms/ios/build-xcframework.sh

# project.yml을 고쳤을 때 Xcode 프로젝트를 다시 만든다. `simulator`가 이것을 딸리지
# 않는 까닭은 프로젝트 파일이 저장소에 있어서다 — 부를 때마다 돌리면 고치지도 않은
# 파일이 매번 바뀐 것으로 잡힌다.
project:
    cd platforms/ios && xcodegen generate

# CI가 보는 것과 같은 것을 손에서도 본다.
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- --deny warnings
    cargo test --workspace
