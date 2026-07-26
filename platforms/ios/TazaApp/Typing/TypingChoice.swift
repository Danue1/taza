import Foundation

extension SettingsModel {
    /// 언어가 항목마다 고르는 것 — 공통값을 따르거나, 이 언어에서만 켜거나 끈다
    enum TypingChoice: Hashable {
        case common
        case on
        case off
    }
}
