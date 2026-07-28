import UIKit

/// 배열 하나를 그림으로 보이기 위해 코어에서 받아 온 것.
///
/// 키 자리와 키에 찍히는 글자는 모두 코어가 정하고, 그리기는 키보드가 쓰는 판이 맡는다 —
/// 설정 화면이 배열 데이터를 따로 읽거나 자기 그리기를 두면 실제로 치게 될 키보드와
/// 그림이 갈라진다.
public struct TazaLayoutPreview: Identifiable {
    public let name: String
    /// 키보드가 그대로 받아 그리는 모델
    public let frame: KeyboardFrameModel
    /// 이 그림이 기준으로 삼은 키보드 폭(pt) — 미리보기는 자기 폭과 이 값의 비율을
    /// 판에 배율로 넘겨, 실제 키보드를 그대로 축소한 그림이 된다
    public let referenceWidth: CGFloat
    /// 키보드 판의 가로세로 비
    public let aspectRatio: CGFloat
    /// 키 테두리 설정 — 코어가 아니라 셸이 그리는 것이라 프레임에 실려 오지 않는다
    public let showsKeyBorders: Bool

    public var id: String { name }
}

/// 이 언어로 칠 수 있는 배열들의 문자면을 코어에서 받아 온다.
///
/// 배열 하나마다 세션을 세우지 않고 한 세션에서 배열만 갈아 가며 프레임을 받는다.
/// 배열은 코어에 있으므로 팩을 받지 않아도 다 나온다. 사용자 설정을 그대로 넣는 것은
/// 숫자 행처럼 **키 자리를 바꾸는 설정**이 있기 때문이다 — 넣지 않으면 그림에만
/// 숫자 줄이 없다.
public func tazaLayoutPreviews(
    for language: TazaLanguage,
    packURL: URL? = nil,
    preferences: FfiUserPreferences? = nil,
    referenceWidth: CGFloat = 390
) -> [TazaLayoutPreview] {
    guard let session = try? KeyboardSession(languageTag: language.tag) else {
        return []
    }
    if let packURL {
        try? session.loadPack(path: packURL.path)
    }
    if let preferences {
        session.setPreferences(preferences: preferences)
    }
    // 폰 세로를 기준으로 잡는다 — 미리보기는 이 그림을 자기 폭에 맞춰 줄인다
    session.setMetrics(
        formFactor: .phonePortrait,
        widthPoints: Float(referenceWidth),
        textScale: 1
    )
    let gridHeight = CGFloat(session.frameMetrics().gridHeight)
    guard gridHeight > 0 else { return [] }
    let displayName = session.language().displayName
    let showsKeyBorders = KeyboardPreferences().keyBorders

    let selected = session.selectedLayout()
    defer { _ = session.selectLayout(name: selected) }

    return session.availableLayouts().compactMap { name in
        guard session.selectLayout(name: name) else { return nil }
        return TazaLayoutPreview(
            name: name,
            frame: keyboardFrameModel(
                session.keyboardFrame(),
                languageDisplayName: displayName
            ),
            referenceWidth: referenceWidth,
            aspectRatio: referenceWidth / gridHeight,
            showsKeyBorders: showsKeyBorders
        )
    }
}
