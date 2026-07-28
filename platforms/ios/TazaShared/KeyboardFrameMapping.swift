import UIKit

/// 코어가 내려준 키 프레임을 디자인 시스템의 그리기 모델로 옮긴다 — 상태 없는 번역이다.
///
/// 키보드 익스텐션과 설정 앱의 배열 미리보기가 **같은 함수**를 부른다. 미리보기가 자기
/// 변환을 따로 두면 키캡 글자·색·간격이 실제 키보드와 조금씩 어긋나고, 그 어긋남은
/// 아무도 알아채지 못한 채 남는다.
public func keyboardFrameModel(
    _ frame: FfiKeyboardFrame,
    languageDisplayName: String? = nil
) -> KeyboardFrameModel {
    KeyboardFrameModel(
        rows: frame.rows.map { row in
            row.enumerated().map { index, key in
                KeyModel(
                    label: legend(for: key),
                    appearance: appearance(for: key),
                    fontSize: CGFloat(key.fontSize),
                    bounds: CGRect(
                        x: CGFloat(key.bounds.x),
                        y: CGFloat(key.bounds.y),
                        width: CGFloat(key.bounds.width),
                        height: CGFloat(key.bounds.height)
                    ),
                    accessibilityLabel: accessibilityLabel(for: key),
                    accessibilityValue: key.role == .languageSwitch ? languageDisplayName : nil,
                    accessibilityHint: hint(for: key),
                    alternates: key.alternates,
                    isActive: key.shiftActive,
                    leadingExtraGap: key.role == .backspace
                        && splitsFromNeighbour(key, index > 0 ? row[index - 1] : nil)
                        ? TazaTheme.Key.edgeExtraGap : 0,
                    trailingExtraGap: key.role == .shift
                        && splitsFromNeighbour(key, index + 1 < row.count ? row[index + 1] : nil)
                        ? TazaTheme.Key.edgeExtraGap : 0
                )
            }
        }
    )
}

/// 가장자리 여백을 더할 자리인지 — 이웃한 글자 키보다 **넓을 때만**이다. 두벌식 아랫줄의
/// shift·backspace는 좁은 글자 열보다 넓어 갈라져 보여야 하지만, 천지인처럼 모든 키가
/// 한 칸씩인 판에서 여백을 더하면 그 키만 칸 밖으로 밀려 윗줄과 어긋난다.
///
/// 좁은 쪽에는 더하지 않는다: 천지인+의 backspace는 한 칸(0.125)이고 왼쪽 이웃 ㅡ는 세
/// 칸이라 폭이 다르지만, 여백을 주면 같은 한 칸인 아래 엔터보다 좁아진다.
private func splitsFromNeighbour(_ key: FfiFrameKey, _ neighbour: FfiFrameKey?) -> Bool {
    guard let neighbour else { return false }
    return key.bounds.width - neighbour.bounds.width > 0.001
}

/// 키에 적히는 글자. 낱말로 적히는 키(리턴키)는 코어가 갈래만 알려 주므로 화면
/// 언어로 옮기고, 나머지는 코어가 준 글자를 그대로 쓴다.
private func legend(for key: FfiFrameKey) -> String {
    switch key.legend {
    case .go: NSLocalizedString("이동", comment: "리턴키 — go")
    case .search: NSLocalizedString("검색", comment: "리턴키 — search")
    case .send: NSLocalizedString("전송", comment: "리턴키 — send")
    case .next: NSLocalizedString("다음", comment: "리턴키 — next")
    case .done: NSLocalizedString("완료", comment: "리턴키 — done")
    case .join: NSLocalizedString("연결", comment: "리턴키 — join")
    case .route: NSLocalizedString("경로", comment: "리턴키 — route")
    case .continue: NSLocalizedString("계속", comment: "리턴키 — continue")
    case .return, nil: key.label
    }
}

/// VoiceOver가 읽는 이름. 코어는 역할만 알려 주고 문구는 화면 언어를 탄다 —
/// 접근성은 계약의 일부이되, 그 계약이 나르는 것은 신원이지 한국어 문장이 아니다.
private func accessibilityLabel(for key: FfiFrameKey) -> String {
    switch key.role {
    case .character: key.label
    case .shift: NSLocalizedString("shift", comment: "shift 키")
    case .backspace: NSLocalizedString("삭제", comment: "backspace 키")
    case .space: NSLocalizedString("스페이스", comment: "스페이스 키")
    case .enter: legend(for: key)
    case .layerSwitch: NSLocalizedString("자판 전환", comment: "레이어 전환 키")
    case .languageSwitch: NSLocalizedString("언어", comment: "언어 전환 키")
    case .languageSelect: key.label
    case .cursorRight: NSLocalizedString("커서 오른쪽", comment: "커서 오른쪽 이동 키")
    case .blank: ""
    }
}

private func appearance(for key: FfiFrameKey) -> KeyCapView.Appearance {
    if key.emphasized {
        return .emphasized
    }
    switch key.role {
    case .character: return .letter
    case .languageSwitch, .languageSelect: return .language
    case .space: return .space
    case .blank: return .blank
    default: return .control
    }
}

private func hint(for key: FfiFrameKey) -> String? {
    switch key.role {
    case .languageSwitch, .languageSelect:
        NSLocalizedString("길게 눌러 언어와 설정 선택", comment: "언어 키 길게 누르기")
    case .space:
        NSLocalizedString("길게 눌러 커서 이동", comment: "스페이스 키 길게 누르기")
    case .character:
        key.alternates.isEmpty
            ? nil
            : NSLocalizedString("길게 눌러 변형 문자 선택", comment: "변형 문자 팝업")
    default: nil
    }
}
