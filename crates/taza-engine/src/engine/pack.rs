//! 언어팩과 배열 교체. 언어를 늘리는 일이 팩 배포로 끝나는 것은 이 파일이 팩이 밝힌
//! 선언(표시 이름·골격·배열)을 내장 선언 위에 덮어쓰기 때문이다.

use std::sync::Arc;

use crate::contract::Pack;
use crate::keyboard::Keyboard;
use crate::lang::{ComposerSkeleton, LanguageDescriptor};
use crate::pack::PackError;
use crate::pack::layout::NamedLayoutSet;

use super::{Engine, PackBytes};

impl Engine {
    /// 언어팩을 갈아 끼운다. 팩이 스스로 밝힌 선언(표시 이름·골격·조회 키 인코딩)이
    /// 내장 선언을 대신하고, 레이아웃 섹션이 있으면 배열도 함께 바뀐다 — 언어를
    /// 늘리는 일이 팩 배포로 끝나는 것은 이 갱신 덕분이다.
    pub fn load_pack(&mut self, pack: Arc<dyn PackBytes>) -> Result<(), PackError> {
        let opened = Pack::open(pack.bytes())?;
        let declared = LanguageDescriptor::from_pack(&opened);
        let packed_layouts = opened.layouts();
        if let Some(declared) = declared {
            // 합성기 교체는 배열을 고른 뒤에 한다(`apply_layout_skeleton`) — 골격을
            // 밝히는 쪽이 언어와 배열 둘이라 한자리에서 정해야 어긋나지 않는다
            self.language = declared;
        }
        self.refresh_suggester();
        // 고르고 있던 배열은 이름으로 이어 간다 — 팩을 갱신했다고 배열이 되돌아가면
        // 사용자가 고른 것이 배포 때마다 풀린다
        let chosen = self.layout_name().to_string();
        self.layouts = packed_layouts.unwrap_or_else(|| {
            vec![NamedLayoutSet {
                name: String::new(),
                skeleton: None,
                layouts: self.language.builtin_layout(),
            }]
        });
        // 이름 없이 실려 온 배열(배열이 하나뿐이던 시절의 팩)은 팩 메타데이터가 이름을 댄다
        for entry in &mut self.layouts {
            if entry.name.is_empty() {
                entry.name = self.language.layout_name.clone();
            }
        }
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
            .map(|entry| entry.name.clone())
            .collect()
    }

    /// 지금 치고 있는 배열의 이름.
    pub fn layout_name(&self) -> &str {
        self.layouts
            .get(self.selected_layout)
            .map(|entry| entry.name.as_str())
            .unwrap_or(&self.language.layout_name)
    }

    /// 배열을 바꾼다. 그런 이름의 배열이 없으면 아무것도 하지 않고 false —
    /// 설정에 남아 있는 이름이 팩 갱신으로 사라졌을 때 조용히 기본값에 머문다.
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
    fn apply_layout_skeleton(&mut self) {
        let declared = self
            .layouts
            .get(self.selected_layout)
            .and_then(|entry| entry.skeleton.as_deref())
            .and_then(ComposerSkeleton::from_tag);
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
        let layers = self
            .layouts
            .get(self.selected_layout)
            .map(|entry| entry.layouts.clone())
            .unwrap_or_else(|| self.language.builtin_layout());
        let traits = self.keyboard.traits();
        self.keyboard = Keyboard::new(layers, self.language.clone());
        self.keyboard.set_metrics(self.metrics);
        self.keyboard.set_preferences(self.preferences);
        self.keyboard.set_field(traits);
    }
}
