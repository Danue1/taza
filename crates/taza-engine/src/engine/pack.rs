//! 언어팩과 배열 교체. 팩이 나르는 것은 사전·언어모델·곁들일 것과 언어가 자기를 밝히는
//! 선언이고, 배열은 코드에 있다 — 배열을 고르는 일이 팩을 받았는지와 무관한 것은 그래서다.

use std::sync::Arc;

use crate::contract::Pack;
use crate::keyboard::{Keyboard, layouts};
use crate::lang::LanguageDescriptor;
use crate::pack::PackError;

use super::{Engine, PackBytes};

impl Engine {
    /// 언어팩을 갈아 끼운다. 팩이 스스로 밝힌 선언(표시 이름·골격·조회 키 인코딩)이
    /// 내장 선언을 대신하고, 골격이 바뀌면 그 골격의 배열들이 함께 온다 — 언어를
    /// 늘리는 일이 팩 배포로 끝나되, 그 언어가 쓰는 골격은 이 빌드에 이미 있어야 한다.
    pub fn load_pack(&mut self, pack: Arc<dyn PackBytes>) -> Result<(), PackError> {
        let opened = Pack::open(pack.bytes())?;
        if let Some(declared) = LanguageDescriptor::from_pack(&opened) {
            // 합성기 교체는 배열을 고른 뒤에 한다(`apply_layout_skeleton`) — 골격을
            // 밝히는 쪽이 언어와 배열 둘이라 한자리에서 정해야 어긋나지 않는다
            self.language = declared;
        }
        self.refresh_suggester();
        // 고르고 있던 배열은 이름으로 이어 간다 — 팩을 갈았다고 배열이 되돌아가면
        // 사용자가 고른 것이 배포 때마다 풀린다
        let chosen = self.layout_name().to_string();
        self.layouts = layouts::for_skeleton(self.language.skeleton);
        self.selected_layout = self
            .layouts
            .iter()
            .position(|entry| entry.name == chosen)
            .unwrap_or(0);
        self.rebuild_keyboard();
        self.pack = Some(pack);
        Ok(())
    }

    /// 이 언어로 칠 수 있는 배열의 이름들 — 설정 화면의 선택지가 된다.
    pub fn available_layouts(&self) -> Vec<String> {
        self.layouts
            .iter()
            .map(|entry| entry.name.to_string())
            .collect()
    }

    /// 지금 치고 있는 배열의 이름.
    pub fn layout_name(&self) -> &str {
        self.layouts
            .get(self.selected_layout)
            .map(|entry| entry.name)
            .unwrap_or_default()
    }

    /// 배열을 바꾼다. 그런 이름의 배열이 없으면 아무것도 하지 않고 false —
    /// 설정에 남아 있는 이름이 판올림으로 사라졌을 때 조용히 기본값에 머문다.
    pub fn select_layout(&mut self, name: &str) -> bool {
        let Some(index) = self.layouts.iter().position(|entry| entry.name == name) else {
            return false;
        };
        if index == self.selected_layout {
            return true;
        }
        self.selected_layout = index;
        self.rebuild_keyboard();
        true
    }

    /// 배열이 자기 골격을 밝혔으면 그 합성기로 갈아 끼운다 — 같은 언어 안에서도 조합
    /// 규칙이 다른 배열(천지인)이 있기 때문이다. 밝히지 않은 배열은 언어의 골격을 쓴다.
    /// 이 빌드에 그 골격이 없으면 쓰던 합성기에 머문다.
    ///
    /// 진행 중인 조합은 여기서 확정되지 않는다 — Effect를 낼 통로가 없기 때문이다.
    /// 배열·팩을 갈아 끼우기 전에 `CursorMoved`(또는 `FocusLost`)로 조합을 먼저 끝내는
    /// 것은 부르는 쪽의 몫이다.
    fn apply_layout_skeleton(&mut self) {
        let declared = self
            .layouts
            .get(self.selected_layout)
            .and_then(|entry| entry.skeleton);
        let skeleton = declared.unwrap_or(self.language.skeleton);
        if skeleton == self.active_skeleton {
            return;
        }
        let Some(composer) = skeleton.composer() else {
            return;
        };
        self.composer = composer;
        self.active_skeleton = skeleton;
    }

    /// 배열이 바뀌어도 셸이 알려 준 표시 환경·설정·필드 성격은 이어진다.
    pub(super) fn rebuild_keyboard(&mut self) {
        self.apply_layout_skeleton();
        let Some(layers) = self
            .layouts
            .get(self.selected_layout)
            .map(|entry| entry.layouts.clone())
        else {
            return;
        };
        let traits = self.keyboard.traits();
        self.keyboard = Keyboard::new(layers, self.language.clone());
        self.keyboard.set_metrics(self.metrics);
        self.keyboard.set_preferences(self.preferences);
        self.keyboard.set_field(traits);
    }
}
