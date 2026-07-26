import UIKit

/// 글자 키를 길게 눌렀을 때 위로 서는 변형 문자 팝업.
extension KeyboardViewController {
    func showAlternatesPopup(options: [String], near point: CGPoint) {
        guard let keyFrame = gridView.keyFrame(at: point) else { return }
        dismissAlternatesPopup()
        let popup = AlternatesPopupView(options: options, metrics: metrics)
        let size = popup.preferredSize(keySize: keyFrame.size)
        // 첫 칸(누르고 있는 글자)이 눌린 키 위에 오도록 왼쪽을 맞춘다 — 손을 그대로 떼면
        // 치던 글자가 그대로 들어가고, 옆으로 끌면 변형에 닿는다
        let originX = min(
            max(keyFrame.minX - 4, 4),
            max(view.bounds.width - size.width - 4, 4)
        )
        // 키 위에 세우되 판 밖으로는 나가지 않는다 — 익스텐션은 자기 뷰 밖에 그릴 수
        // 없어, 첫 행처럼 위가 모자라면 그대로 두면 잘린다
        let keyTop = gridView.frame.minY + keyFrame.minY
        popup.frame = CGRect(
            x: originX,
            y: max(keyTop - size.height - 4, 0),
            width: size.width,
            height: size.height
        )
        popup.layoutIfNeeded()
        popup.updateHighlight(atX: keyFrame.midX - originX)
        view.addSubview(popup)
        alternatesPopup = popup
    }

    func dismissAlternatesPopup() {
        alternatesPopup?.removeFromSuperview()
        alternatesPopup = nil
    }

    /// 팝업에서 고른 변형 문자 — 키는 손을 뗄 때 확정되므로 아직 들어간 글자가 없다.
    /// 고른 것 하나만 일반 키 입력과 같은 경로로 흘려보낸다.
    func commitAlternate(_ alternate: String) {
        guard let session = activeSession else { return }
        apply(effects: session.selectAlternate(alternate: alternate, context: currentContext()))
    }
}
