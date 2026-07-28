//! 셸이 쥐는 손잡이 하나. 조립은 코어(`Engine`)가 하고 이 파일은 잠금·타입 번역과
//! 파일 IO만 맡는다. 개인화 스냅샷의 줄 형식도 여기가 주인이다 — 셸이 줄을 뜯어보기
//! 시작하면 형식이 두 곳에 살게 된다.

use std::sync::{Arc, Mutex};

use taza_engine::contract::CandidateGroup;
use taza_engine::engine::{Engine, PackBytes};
use taza_engine::keyboard::{KeyboardMetrics, ShellRequest};
use taza_engine::lang::LanguageDescriptor;
use taza_engine::personalization::PersonalizationState;

use crate::convert::*;
use crate::types::*;

/// 설정 화면이 "지우기" 앞에서 무엇이 남아 있는지 보여 주는 통로.
#[uniffi::export]
pub fn personalization_summary(lines: Vec<String>) -> FfiPersonalizationSummary {
    let count = |prefix: &str| lines.iter().filter(|line| line.starts_with(prefix)).count() as u32;
    FfiPersonalizationSummary {
        learned_words: count("w\t"),
        recent_annotations: count("a\t"),
    }
}

/// 키보드 익스텐션 프로세스당 하나 — 셸은 이 객체 하나로 입력·화면·팩을 오간다.
/// 조립은 코어(`Engine`)가 하고 이 계층은 타입 번역과 파일 IO만 맡는다.
#[derive(uniffi::Object)]
pub struct KeyboardSession {
    engine: Mutex<Engine>,
}

#[uniffi::export]
impl KeyboardSession {
    /// 언어 태그로 만든다. 내장 선언이 없는 태그는 팩을 받아야 쓸 수 있으므로
    /// 여기서 걸러진다 — 팩이 실리면 그 선언이 내장 선언을 대신한다.
    #[uniffi::constructor]
    pub fn new(language_tag: String) -> Result<Self, FfiLanguageError> {
        let language =
            LanguageDescriptor::builtin(&language_tag).ok_or(FfiLanguageError::Unsupported)?;
        let engine = Engine::new(language).ok_or(FfiLanguageError::Unsupported)?;
        Ok(KeyboardSession {
            engine: Mutex::new(engine),
        })
    }

    /// 지금 쓰고 있는 언어 선언 — 팩을 실으면 팩이 밝힌 값으로 바뀐다. 배열 이름은
    /// 언어가 아니라 지금 고른 배열의 것이다(배열은 팩이 아니라 코드에 있다).
    pub fn language(&self) -> FfiLanguageDescriptor {
        let engine = self.engine.lock().unwrap();
        let layout_name = engine.layout_name().to_string();
        let language = engine.language();
        FfiLanguageDescriptor {
            tag: language.tag.clone(),
            display_name: language.display_name.clone(),
            keycap_label: language.keycap_label.clone(),
            layout_name,
        }
    }

    /// 언어팩 파일을 mmap으로 연다. 파일 백드 clean page라 익스텐션 메모리
    /// 예산(jetsam footprint)에 산입되지 않는다. 레이아웃 섹션이 있으면 교체한다.
    pub fn load_pack(&self, path: String) -> Result<(), FfiPackError> {
        let file = std::fs::File::open(&path).map_err(|error| FfiPackError::Io {
            message: error.to_string(),
        })?;
        let bytes = unsafe { memmap2::Mmap::map(&file) }.map_err(|error| FfiPackError::Io {
            message: error.to_string(),
        })?;
        self.engine
            .lock()
            .unwrap()
            .load_pack(Arc::new(bytes) as Arc<dyn PackBytes>)
            .map_err(|error| FfiPackError::Invalid {
                message: error.to_string(),
            })
    }

    /// 사용자 설정 주입 — 셸은 키보드를 띄울 때마다 자기 저장소에서 읽어 넣는다.
    /// 설정 화면에서 바뀐 값은 다음 표시 때 이 호출로 반영된다.
    pub fn set_preferences(&self, preferences: FfiUserPreferences) {
        self.engine
            .lock()
            .unwrap()
            .set_preferences(convert_preferences_in(preferences));
    }

    /// 이 언어로 칠 수 있는 배열의 이름들 — 첫 항목이 기본 배열이다.
    pub fn available_layouts(&self) -> Vec<String> {
        self.engine.lock().unwrap().available_layouts()
    }

    /// 지금 치고 있는 배열의 이름.
    pub fn selected_layout(&self) -> String {
        self.engine.lock().unwrap().layout_name().to_string()
    }

    /// 배열을 바꾼다. 그런 이름이 없으면 false — 설정에 남은 이름이 팩 갱신으로
    /// 사라졌을 때 셸이 기본 배열로 되돌리는 신호다.
    pub fn select_layout(&self, name: String) -> bool {
        self.engine.lock().unwrap().select_layout(&name)
    }

    /// 문맥이 문장 첫 자리를 가리키면 shift를 미리 올린다. 초점·필드가 바뀌었을 때와
    /// `handle_event`로 입력을 넣은 뒤에 부른다(`press_at`은 스스로 한다).
    /// 프레임을 다시 그려야 하면 true.
    pub fn sync_auto_shift(&self, context: FfiEditorContext) -> bool {
        self.engine
            .lock()
            .unwrap()
            .sync_auto_shift(&convert_context(&context))
    }

    /// 사용자 대치 표 주입(순정의 "텍스트 대치"). 값의 주인은 설정 앱이다.
    pub fn set_shortcuts(&self, shortcuts: Vec<FfiShortcut>) {
        self.engine.lock().unwrap().set_shortcuts(
            shortcuts
                .into_iter()
                .filter(|entry| !entry.trigger.is_empty())
                .map(|entry| (entry.trigger, entry.replacement))
                .collect(),
        );
    }

    /// 표시 환경 주입 — 셸이 자기 크기를 알게 될 때(첫 배치, 회전, 분할) 부른다.
    /// 이후 프레임의 치수는 이 값을 따른다.
    pub fn set_metrics(&self, form_factor: FfiFormFactor, width_points: f32, text_scale: f32) {
        self.engine.lock().unwrap().set_metrics(KeyboardMetrics {
            form_factor: convert_form_factor(form_factor),
            width_points,
            text_scale,
        });
    }

    /// 편집 대상이 바뀔 때(초점 이동, 앱 전환) 셸이 알려 주는 필드 성격. 배열·리턴키·
    /// 후보 바 자리가 여기서 갈리므로 프레임을 받기 전에 넣는다.
    pub fn set_field(&self, traits: FfiFieldTraits) {
        self.engine
            .lock()
            .unwrap()
            .set_field(convert_field_traits(&traits));
    }

    /// 이메일·URL·비밀번호처럼 라틴 글자를 넣게 되어 있는 칸인가. 어느 언어로 열지는
    /// 언어 목록을 가진 셸이 정하므로 코어는 "라틴이 맞다"만 말한다.
    pub fn field_prefers_latin(&self) -> bool {
        self.engine.lock().unwrap().field_prefers_latin()
    }

    /// 프레임 전체를 받지 않고 치수만 필요할 때(입력 뷰 높이 제약).
    pub fn frame_metrics(&self) -> FfiFrameMetrics {
        convert_frame_metrics(self.engine.lock().unwrap().frame_metrics())
    }

    /// 이벤트당 1회 왕복 — 반환된 Effect 목록을 셸이 순서대로 플랫폼 API로 번역한다.
    pub fn handle_event(&self, event: FfiInputEvent, context: FfiEditorContext) -> Vec<FfiEffect> {
        let Some(input_event) = convert_event(event) else {
            return Vec::new();
        };
        let effects = self
            .engine
            .lock()
            .unwrap()
            .handle(input_event, &convert_context(&context));
        effects.into_iter().map(convert_effect).collect()
    }

    /// 멀티탭 시한이 다 됐다고 셸이 알려 준다 — 다음에 같은 키를 눌러도 주기가 아니라
    /// 새 글자로 시작한다. 이미 끝난 주기에 울린 타이머는 아무 일도 하지 않는다.
    pub fn timer_fired(&self) {
        self.engine.lock().unwrap().timer_fired();
    }

    /// 터치 좌표(정규화) → 코어 히트 테스트 → 합성까지 한 번에.
    pub fn press_at(&self, x: f32, y: f32, context: FfiEditorContext) -> FfiPressResult {
        let result = self
            .engine
            .lock()
            .unwrap()
            .press_at(x, y, &convert_context(&context));
        FfiPressResult {
            effects: result.effects.into_iter().map(convert_effect).collect(),
            layout_changed: result.layout_changed,
            requests_next_language: result.request == Some(ShellRequest::NextLanguage),
            requests_language: match result.request {
                Some(ShellRequest::Language(tag)) => Some(tag),
                _ => None,
            },
        }
    }

    /// 좌표에 있는 키 — 셸이 길게 누르기 대상을 알아내는 통로. 스냅 규칙이 탭과
    /// 같아야 하므로 코어 히트 테스트를 그대로 쓴다.
    pub fn key_at(&self, x: f32, y: f32) -> FfiFrameKey {
        convert_frame_key(self.engine.lock().unwrap().key_at(x, y))
    }

    /// shift 키를 두 번 누른 것 — 두 번 누름을 알아보는 것은 플랫폼 제스처라 셸이 하고,
    /// 고정할 수 있는 배열인지와 그 뒤 상태는 코어가 정한다. 고정되면 true.
    pub fn toggle_shift_lock(&self) -> bool {
        self.engine.lock().unwrap().toggle_shift_lock()
    }

    /// 길게 눌러 연 팝업에서 고른 변형 문자 — 일반 키 입력과 같은 경로로 흐르므로
    /// 누름과 같은 결과를 낸다. 일회성 shift가 이 입력으로 풀리면 판을 다시 그려야 한다.
    pub fn select_alternate(&self, alternate: String, context: FfiEditorContext) -> FfiPressResult {
        let result = self
            .engine
            .lock()
            .unwrap()
            .select_alternate(&alternate, &convert_context(&context));
        FfiPressResult {
            effects: result.effects.into_iter().map(convert_effect).collect(),
            layout_changed: result.layout_changed,
            requests_next_language: result.request == Some(ShellRequest::NextLanguage),
            requests_language: match result.request {
                Some(ShellRequest::Language(tag)) => Some(tag),
                _ => None,
            },
        }
    }

    /// 스페이스바를 길게 눌러 끄는 커서 이동. 셸은 포인터 x(정규화)만 흘려보내고
    /// 몇 칸 움직일지는 코어가 판정한다.
    pub fn begin_cursor_drag(&self, x: f32) {
        self.engine.lock().unwrap().begin_cursor_drag(x);
    }

    pub fn update_cursor_drag(&self, x: f32, context: FfiEditorContext) -> Vec<FfiEffect> {
        let effects = self
            .engine
            .lock()
            .unwrap()
            .update_cursor_drag(x, &convert_context(&context));
        effects.into_iter().map(convert_effect).collect()
    }

    pub fn end_cursor_drag(&self) {
        self.engine.lock().unwrap().end_cursor_drag();
    }

    pub fn keyboard_frame(&self) -> FfiKeyboardFrame {
        let frame = self.engine.lock().unwrap().frame();
        FfiKeyboardFrame {
            rows: frame
                .rows
                .into_iter()
                .map(|row| row.into_iter().map(convert_frame_key).collect())
                .collect(),
            metrics: convert_frame_metrics(frame.metrics),
            panel_height_ratio: frame.panel_height_ratio,
        }
    }

    /// 통합 검색면 내용 — 검색어가 비면 자주 쓰는 것과 갈래별 목록이 온다.
    pub fn annotation_panel(&self, query: String) -> FfiAnnotationPanel {
        let panel = self.engine.lock().unwrap().annotation_panel(&query);
        FfiAnnotationPanel {
            groups: panel
                .groups
                .into_iter()
                .map(|group| FfiAnnotationPanelGroup {
                    group: group.group.map(convert_candidate_group),
                    category: group.category.map(convert_emoji_category),
                    items: group
                        .items
                        .into_iter()
                        .map(|item| FfiAnnotationPanelItem {
                            group: convert_candidate_group(item.group),
                            text: item.text,
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    /// 검색면에서 고른 것을 넣는다 — 진행 중 조합 확정과 최근 사용 기록은 코어가 한다.
    pub fn select_annotation(
        &self,
        group: FfiCandidateGroup,
        text: String,
        context: FfiEditorContext,
    ) -> Vec<FfiEffect> {
        let group = convert_candidate_group_in(group);
        let effects =
            self.engine
                .lock()
                .unwrap()
                .select_annotation(group, &text, &convert_context(&context));
        effects.into_iter().map(convert_effect).collect()
    }

    /// 개인화 상태 직렬화 — 셸이 컨테이너 저장소(App Group 등)에 보관한다.
    pub fn personalization_snapshot(&self) -> Vec<String> {
        let snapshot = self.engine.lock().unwrap().personalization_snapshot();
        let mut lines = vec![snapshot.clock.to_string()];
        for (word, count, last_used) in snapshot.entries {
            lines.push(format!("w\t{word}\t{count}\t{last_used}"));
        }
        for (group, text) in snapshot.recent_annotations {
            let Some(tag) = group.tag() else { continue };
            lines.push(format!("a\t{tag}\t{text}"));
        }
        lines
    }

    /// 통합 검색면의 "자주 쓰는" 목록만 비운다 — 배운 어휘는 그대로 둔다.
    pub fn reset_recent_annotations(&self) {
        self.engine.lock().unwrap().reset_recent_annotations();
    }

    /// 배운 것을 전부 잊는다 — 순정의 "키보드 사전 재설정". 셸은 이 호출과 함께
    /// 보관 중인 스냅샷도 지운다.
    pub fn reset_personalization(&self) {
        self.engine.lock().unwrap().reset_personalization();
    }

    pub fn restore_personalization(&self, lines: Vec<String>) {
        let Some((clock_line, entry_lines)) = lines.split_first() else {
            return;
        };
        let Ok(clock) = clock_line.parse() else {
            return;
        };
        let mut entries = Vec::new();
        let mut recent_annotations = Vec::new();
        for line in entry_lines {
            match line.split('\t').collect::<Vec<&str>>().as_slice() {
                ["w", word, count, last_used] => {
                    let (Ok(count), Ok(last_used)) = (count.parse(), last_used.parse()) else {
                        continue;
                    };
                    entries.push((word.to_string(), count, last_used));
                }
                ["a", tag, text] => {
                    let Some(group) = tag.parse().ok().and_then(CandidateGroup::from_tag) else {
                        continue;
                    };
                    recent_annotations.push((group, text.to_string()));
                }
                _ => continue,
            }
        }
        self.engine
            .lock()
            .unwrap()
            .restore_personalization(PersonalizationState {
                entries,
                clock,
                recent_annotations,
            });
    }
}
