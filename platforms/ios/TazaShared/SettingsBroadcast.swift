import Foundation

/// 설정이 바뀌었다는 것을 다른 프로세스에 알리는 길.
///
/// 앱과 익스텐션은 App Group 저장소를 함께 보지만, 한쪽이 쓴 것을 다른 쪽이 **언제**
/// 알게 되는지는 저장소가 알려 주지 않는다(`UserDefaults`의 변경 통지는 프로세스를
/// 넘지 않는다). 그래서 값은 저장소로, 신호는 Darwin 알림으로 따로 보낸다 —
/// 알림에는 내용이 실리지 않으므로 받는 쪽은 늘 저장소를 다시 읽는다.
///
/// 이 길이 없으면 설정은 키보드가 다음에 뜰 때에야 반영된다. 설정 화면 안의 테스트 칸은
/// 키보드를 띄운 채로 값을 바꾸는 자리라, 그 기다림이 그대로 드러난다.
public enum SettingsBroadcast {
    /// 무엇이 바뀌었는지. 갈래를 나누는 이유는 **학습 스냅샷에는 쓰는 쪽이 둘**이기
    /// 때문이다: 익스텐션은 치는 동안 배우고, 앱은 지운다. 설정 하나를 바꿀 때마다
    /// 저장된 스냅샷을 세션에 되씌우면 아직 저장되지 않은 학습이 되감긴다. 그래서
    /// 스냅샷은 앱이 실제로 손댔을 때만 다시 읽는다.
    public enum Kind: CaseIterable, Sendable {
        /// 입력 보조·배열·표시·입력감 — 값의 주인이 앱 하나뿐인 것들
        case settings
        /// 학습 스냅샷을 앱이 고쳤다(재설정·최근 사용 지우기)
        case learning

        var notificationName: CFString {
            switch self {
            case .settings: return "io.danuel.taza.settings-changed" as CFString
            case .learning: return "io.danuel.taza.learning-changed" as CFString
            }
        }
    }

    /// 값을 저장한 쪽이 부른다.
    public static func post(_ kind: Kind) {
        CFNotificationCenterPostNotification(
            CFNotificationCenterGetDarwinNotifyCenter(),
            CFNotificationName(kind.notificationName),
            nil,
            nil,
            true
        )
    }

    /// 받는 쪽이 살아 있는 동안 유지한다. 알림은 프로세스 밖에서 오므로 콜백에는
    /// 컨텍스트를 실을 수 없다 — 옵저버는 자기 자신을 키로 쓰고 핸들러만 붙든다.
    public final class Observer {
        private let handler: (Kind) -> Void

        public init(handler: @escaping (Kind) -> Void) {
            self.handler = handler
            let observer = Unmanaged.passUnretained(self).toOpaque()
            for kind in Kind.allCases {
                CFNotificationCenterAddObserver(
                    CFNotificationCenterGetDarwinNotifyCenter(),
                    observer,
                    { _, observer, name, _, _ in
                        guard let observer, let name else { return }
                        let instance = Unmanaged<Observer>.fromOpaque(observer)
                            .takeUnretainedValue()
                        let received = Kind.allCases.first {
                            CFStringCompare($0.notificationName, name.rawValue, []) == .compareEqualTo
                        }
                        guard let received else { return }
                        DispatchQueue.main.async { instance.handler(received) }
                    },
                    kind.notificationName,
                    nil,
                    .deliverImmediately
                )
            }
        }

        deinit {
            CFNotificationCenterRemoveEveryObserver(
                CFNotificationCenterGetDarwinNotifyCenter(),
                Unmanaged.passUnretained(self).toOpaque()
            )
        }
    }
}
