//! 코어의 최상위 조립 지점. 키보드 상태·합성기·개인화·언어팩을 한 객체가 소유하므로
//! 셸(taza-ffi)은 타입 번역만 하고 조립하지 않는다.
//!
//! sans-io는 유지된다 — 파일을 여는 일은 셸의 몫이고, 코어는 이미 열린 바이트만 받는다.
//!
//! 이 파일에는 `Engine`이 무엇을 쥐고 있는지와 셸이 주입하는 것들만 둔다. 하는 일은
//! 갈래별로 나뉘어 있다: 팩·배열 교체는 `pack`, 입력 진입점은 `input`, 합성 뒤의
//! 랭킹·교정·학습은 `compose`, 통합 검색면은 `annotation`.

mod annotation;
mod compose;
mod input;
mod pack;

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::contract::{Composer, EditorContext, Effect, FieldKind, FieldTraits, UserPreferences};
use crate::keyboard::{
    FrameKey, FrameMetrics, KeySignal, Keyboard, KeyboardFrame, KeyboardMetrics, ShellRequest,
};
use crate::lang::{ComposerSkeleton, LanguageDescriptor};
use crate::pack::layout::NamedLayoutSet;
use crate::personalization::{PersonalizationState, PersonalizationStore};
use crate::suggest::{Suggester, Suggestion};

use compose::Correction;

/// 언어팩 바이트의 소유자. 온디바이스에서는 mmap, 테스트·평가에서는 `Vec<u8>`이며
/// 둘 다 `AsRef<[u8]>`이므로 별도 구현이 필요 없다.
pub trait PackBytes: Send + Sync {
    fn bytes(&self) -> &[u8];
}

impl<Source: AsRef<[u8]> + Send + Sync> PackBytes for Source {
    fn bytes(&self) -> &[u8] {
        self.as_ref()
    }
}

/// 터치 한 번의 결과 — 입력이 만든 Effect와, 코어가 판정할 수 없어 셸에 넘기는 요청.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PressResult {
    pub effects: Vec<Effect>,
    /// shift·레이어 전환 등으로 프레임을 다시 그려야 하는지
    pub layout_changed: bool,
    pub request: Option<ShellRequest>,
}

/// 키보드 익스텐션 프로세스당 하나. 이벤트를 받아 Effect 목록을 낸다.
pub struct Engine {
    language: LanguageDescriptor,
    composer: Box<dyn Composer>,
    suggester: Suggester,
    keyboard: Keyboard,
    /// 이 언어로 칠 수 있는 배열들 — 팩이 실은 목록이고, 팩이 없으면 골격의 내장 배열
    /// 하나다. 어느 것으로 칠지는 설정이 정한다.
    layouts: Vec<NamedLayoutSet>,
    selected_layout: usize,
    /// 지금 꽂혀 있는 합성기의 골격 — 배열이 자기 골격을 밝히면 갈아 끼우므로,
    /// 무엇이 꽂혀 있는지를 알아야 헛되이 다시 만들지 않는다
    active_skeleton: ComposerSkeleton,
    personalization: PersonalizationStore,
    /// 팩 교체로 키보드를 다시 만들어도 셸이 주입한 표시 환경은 이어져야 한다
    metrics: KeyboardMetrics,
    preferences: UserPreferences,
    pack: Option<Arc<dyn PackBytes>>,
    /// 후보 목록은 Engine이 소유한다 — 셸은 인덱스로 고르고, 학습·문맥 추적에 쓰는
    /// 조회 키는 표시 텍스트와 함께 여기에만 남는다
    suggestions: Vec<Suggestion>,
    /// 직전에 확정된 어휘의 조회 키 — 언어모델 문맥
    previous_word: Option<String>,
    /// 지금 어절에 대해 눌린 키 신호들. 조회 키의 **끝에서부터** 맞춰 쓴다 — 커서 이동
    /// 뒤 문맥에서 되가져온 앞부분에는 신호가 없으므로 조회 키보다 짧을 수 있다.
    touches: Vec<KeySignal>,
    /// 사용자가 정한 대치 표(순정의 "텍스트 대치") — 친 말을 확정하는 순간 갈아치운다.
    /// 값의 주인은 설정 앱이므로 코어는 주입받아 조회만 한다.
    shortcuts: BTreeMap<String, String>,
    /// 방금 자동교정으로 갈아치운 것 — 바로 뒤에 오는 Backspace는 이것을 되돌린다.
    /// 다음 입력이 하나라도 지나가면 사라진다(순정 키보드 관습).
    reverted_correction: Option<Correction>,
}

impl Engine {
    /// 이 빌드에 골격이 포함되지 않았으면 None — 셸은 해당 언어를 비활성 처리한다.
    pub fn new(language: LanguageDescriptor) -> Option<Self> {
        let composer = language.skeleton.composer()?;
        Some(Engine::with_composer(language, composer))
    }

    /// 언어의 기본 합성기 대신 다른 합성기를 꽂는다. 한 언어에 복수 배열·합성기를
    /// 두는 경우(인도계 음역↔네이티브 등)와 테스트가 쓰는 통로다.
    pub fn with_composer(language: LanguageDescriptor, composer: Box<dyn Composer>) -> Self {
        let builtin = NamedLayoutSet {
            name: language.layout_name.clone(),
            skeleton: None,
            layouts: language.builtin_layout(),
        };
        Engine {
            suggester: Suggester::new(language.suggestion_policy()),
            composer,
            keyboard: Keyboard::new(builtin.layouts.clone(), language.clone()),
            layouts: vec![builtin],
            selected_layout: 0,
            active_skeleton: language.skeleton,
            language,
            personalization: PersonalizationStore::new(),
            metrics: KeyboardMetrics::default(),
            preferences: UserPreferences::default(),
            pack: None,
            suggestions: Vec::new(),
            previous_word: None,
            touches: Vec::new(),
            shortcuts: BTreeMap::new(),
            reverted_correction: None,
        }
    }

    pub fn language(&self) -> &LanguageDescriptor {
        &self.language
    }

    /// 사용자 설정 주입 — 셸이 설정 저장소에서 읽어 넣는다. 팩과 무관한 값이라
    /// 팩을 갈아 끼워도 유지되고, 설정 화면에서 바뀐 값은 다음 키보드 표시 때
    /// 이 호출로 반영된다.
    pub fn set_preferences(&mut self, preferences: UserPreferences) {
        self.preferences = preferences;
        self.keyboard.set_preferences(preferences);
        self.refresh_suggester();
    }

    /// 후보 바 구성은 언어(골격)가 정한 정책 위에 사용자 설정을 덮은 결과다 — 설정이
    /// 바뀌거나 팩이 바뀌면 둘을 다시 합친다.
    fn refresh_suggester(&mut self) {
        let mut policy = self.language.suggestion_policy();
        if !self.preferences.annotation_candidates {
            policy.annotation_limit = 0;
        }
        self.suggester = Suggester::new(policy);
    }

    /// 문맥이 문장 첫 자리를 가리키면 shift를 미리 올린다. 프레임을 다시 그려야 하면
    /// true — 셸은 초점·필드가 바뀔 때와 입력을 적용한 뒤에 부른다.
    pub fn sync_auto_shift(&mut self, context: &EditorContext) -> bool {
        let text = context.text_before_cursor.as_deref();
        let engaged = self.preferences.auto_capitalization
            && !self.composer.is_composing()
            && self.keyboard.capitalizes(
                crate::policy::sentence_start(text),
                crate::policy::word_start(text),
            );
        self.keyboard.set_auto_shift(engaged)
    }

    /// 사용자 대치 표 주입 — 설정 앱이 값의 주인이고 코어는 조회만 한다.
    pub fn set_shortcuts(&mut self, shortcuts: BTreeMap<String, String>) {
        self.shortcuts = shortcuts;
    }

    /// 표시 환경 주입 — 셸이 자기 크기를 알게 될 때(첫 배치, 회전, 분할) 부른다.
    pub fn set_metrics(&mut self, metrics: KeyboardMetrics) {
        self.metrics = metrics;
        self.keyboard.set_metrics(metrics);
    }

    /// 편집 대상이 바뀔 때 셸이 알려 주는 필드 성격. 보조 기능은 이벤트마다 오는
    /// `EditorContext`가 정하지만, 화면(배열·리턴키·후보 바 자리)은 이벤트 없이도
    /// 그려야 하므로 별도로 주입받는다.
    pub fn set_field(&mut self, traits: FieldTraits) {
        self.keyboard.set_field(traits);
    }

    /// 이메일·URL처럼 라틴 글자를 넣게 되어 있는 칸인가. 어느 언어로 열지는 언어 목록을
    /// 가진 셸이 정하므로, 코어는 "라틴이 맞다"만 말한다(순정도 이 칸들을 라틴으로 연다).
    pub fn field_prefers_latin(&self) -> bool {
        matches!(
            self.keyboard.field(),
            FieldKind::Email | FieldKind::Url | FieldKind::Password
        )
    }

    pub fn frame_metrics(&self) -> FrameMetrics {
        self.keyboard.frame_metrics()
    }

    pub fn frame(&self) -> KeyboardFrame {
        self.keyboard.frame()
    }

    /// 좌표에 있는 키 — 셸이 길게 누르기 대상을 알아내는 통로. 스냅 규칙이 탭과
    /// 같아야 하므로 코어 히트 테스트를 그대로 쓴다.
    pub fn key_at(&self, x: f32, y: f32) -> FrameKey {
        self.keyboard.key_at(x, y)
    }

    /// 익스텐션 kill 대비 — 개인화 상태를 스냅샷해 컨테이너 저장소에 보관한다.
    pub fn personalization_snapshot(&self) -> PersonalizationState {
        self.personalization.snapshot()
    }

    pub fn restore_personalization(&mut self, state: PersonalizationState) {
        self.personalization = PersonalizationStore::restore(state);
    }

    /// 배운 것을 전부 잊는다 — 순정 키보드의 "키보드 사전 재설정"에 해당한다.
    /// 학습을 끄는 설정과 짝이다: 설정은 앞으로를 막고 이것은 지난 것을 지운다.
    pub fn reset_personalization(&mut self) {
        self.personalization = PersonalizationStore::new();
    }

    /// 통합 검색면의 "자주 쓰는" 목록만 비운다. 배운 어휘는 그대로 둔다 — 이모지가
    /// 남긴 자취를 지우려는 사람이 사전까지 잃을 이유가 없다.
    pub fn reset_recent_annotations(&mut self) {
        self.personalization.clear_recent_annotations();
    }
}
