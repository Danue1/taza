//! 셸이 두드리는 문. 터치·타이머·제스처가 여기로 들어와 `InputEvent`로 갈리고,
//! 그 뒤에 붙는 랭킹·교정·학습은 `compose`가 맡는다.

use crate::contract::{ComposerEvent, EditorContext, Effect, InputEvent};
use crate::keyboard::KeySignal;
use crate::policy::PunctuationOutcome;

use super::{Engine, PressResult};

/// 방금 낸 Effect를 문맥에 미리 적용한 결과. 셸이 문맥을 다시 읽어 오기 전에 코어가
/// 스스로 판단해야 하는 것(자동 대문자화)이 쓴다. 문맥을 못 받는 앱에서는 None 그대로다.
fn text_after(context: &EditorContext, effects: &[Effect]) -> Option<String> {
    let mut text = context.text_before_cursor.clone()?;
    for effect in effects {
        match effect {
            Effect::CommitText(committed) => text.push_str(committed),
            Effect::DeleteBackward(count) => {
                for _ in 0..*count {
                    text.pop();
                }
            }
            _ => {}
        }
    }
    Some(text)
}

impl Engine {
    /// 터치 좌표 → 히트 테스트 → 합성까지 한 번에.
    pub fn press_at(&mut self, x: f32, y: f32, context: &EditorContext) -> PressResult {
        let outcome = self.keyboard.press_at(x, y);
        let mut effects = match outcome.event {
            Some(event) => self.handle(event, context),
            None => Vec::new(),
        };
        if let Some(milliseconds) = outcome.timer {
            effects.push(Effect::SetTimer(milliseconds));
        }
        // 방금 넣은 것까지 반영한 문맥으로 다음 글자의 shift를 정한다 — 셸이 문맥을
        // 다시 읽어 오기를 기다리면 마침표를 찍고 친 첫 글자가 소문자로 들어간다
        let applied = EditorContext {
            text_before_cursor: text_after(context, &effects),
            ..context.clone()
        };
        let shift_changed = self.sync_auto_shift(&applied);
        PressResult {
            effects,
            layout_changed: outcome.layout_changed || shift_changed,
            request: outcome.request,
        }
    }

    /// 멀티탭 시한이 다 됐다고 셸이 알려 준다 — 다음에 같은 키를 눌러도 주기가 아니라
    /// 새 글자로 시작한다. 이미 끝난 주기에 울린 타이머는 아무 일도 하지 않는다.
    pub fn timer_fired(&mut self) {
        self.keyboard.expire_multitap();
    }

    /// shift 키를 두 번 눌렀을 때(순정 관례) shift를 고정하거나 푼다. 고정할 수 없는
    /// 배열이면 false — 그때 셸은 두 번째 누름을 평범한 shift 토글로 남겨 둔다.
    pub fn toggle_shift_lock(&mut self) -> bool {
        self.keyboard.toggle_shift_lock()
    }

    /// 길게 눌러 연 팝업에서 고른 변형 문자 — 일반 키 입력과 같은 경로로 흐른다.
    pub fn select_alternate(&mut self, alternate: &str, context: &EditorContext) -> Vec<Effect> {
        match self.keyboard.select_alternate(alternate) {
            Some(event) => self.handle(event, context),
            None => Vec::new(),
        }
    }

    /// 스페이스바를 길게 눌러 끄는 커서 이동. 셸은 포인터 x(정규화)만 흘려보내고
    /// 몇 칸 움직일지는 코어가 판정한다.
    pub fn begin_cursor_drag(&mut self, x: f32) {
        self.keyboard.begin_cursor_drag(x);
    }

    pub fn update_cursor_drag(&mut self, x: f32, context: &EditorContext) -> Vec<Effect> {
        let steps = self.keyboard.update_cursor_drag(x);
        if steps == 0 {
            return Vec::new();
        }
        self.handle(InputEvent::CursorDrag(steps), context)
    }

    pub fn end_cursor_drag(&mut self) {
        self.keyboard.end_cursor_drag();
    }

    pub fn handle(&mut self, event: InputEvent, context: &EditorContext) -> Vec<Effect> {
        match event {
            InputEvent::CursorMoved | InputEvent::FocusLost => self.finalize_composition(),
            InputEvent::CursorDrag(steps) => {
                // 이동 전에 진행 중 composing을 언어별 규칙으로 확정한다 — 커서가
                // 빠져나간 자리에 조합 중 텍스트를 남기지 않는다.
                let mut effects = self.handle(InputEvent::CursorMoved, context);
                if steps != 0 {
                    effects.push(Effect::MoveCursor(steps));
                }
                effects
            }
            InputEvent::Key(signal) => {
                let mut character = signal.character();
                // 부호 규칙은 합성기보다 먼저 본다 — 언어와 무관한 규칙을 언어 수만큼
                // 늘리지 않기 위해서다. 조합 중에는 성립하지 않는다: 조합 창 안의 글자를
                // 갈아치우면 합성기가 자기 상태와 어긋난 것을 보게 된다.
                if !self.composer.is_composing()
                    && let Some(outcome) = crate::policy::punctuation(
                        character,
                        &self.preferences,
                        self.keyboard.traits(),
                        context,
                    )
                {
                    match outcome {
                        // 짝맞춤 따옴표는 어절 안에 설 수 있다("don't") — 합성기를
                        // 건너뛰면 그 자리에서 어절이 끊겨 축약형이 사전에 닿지 못한다
                        PunctuationOutcome::Substitute(substituted) => character = substituted,
                        outcome => return self.emit_punctuation(outcome),
                    }
                }
                self.touches.push(signal);
                self.feed(ComposerEvent::Key(character), context, None)
            }
            // 여러 글자를 한 번에 넣는 키는 어절을 끊는다 — 조합 중이던 것을 언어별
            // 규칙으로 확정한 뒤에 넣어야 `.com`이 조합에 말려들지 않는다
            InputEvent::Text(text) => {
                let mut effects = self.finalize_composition();
                effects.push(Effect::CommitText(text));
                effects
            }
            // 이어 누른 멀티탭 — 방금 넣은 글자를 지우고 주기의 다음 글자를 넣는다.
            // 지우기와 넣기를 그대로 이어 쓰면 조합·어절 추적이 따로 볼 것이 없다.
            InputEvent::Retap(character) => {
                let mut effects = self.handle(InputEvent::Backspace, context);
                // 지운 글자는 아직 문서에 남아 있다 — 그 문맥을 그대로 넘기면 합성 재개가
                // 그것을 도로 주워 와 갈아 끼우는 대신 글자가 하나 더 붙는다
                effects.extend(self.handle(
                    InputEvent::Key(KeySignal::certain(character)),
                    &context.unapplied(),
                ));
                effects
            }
            InputEvent::Backspace => {
                if let Some(correction) = self.reverted_correction.take() {
                    return self.revert(correction);
                }
                self.touches.pop();
                self.feed(ComposerEvent::Backspace, context, None)
            }
            InputEvent::Separator(character) => {
                self.feed(ComposerEvent::Separator(character), context, None)
            }
            InputEvent::CandidateSelected(index) => {
                let Some(selected) = self.suggestions.get(index).cloned() else {
                    return Vec::new();
                };
                self.feed(
                    ComposerEvent::CandidateSelected(selected.text.clone()),
                    context,
                    Some(selected),
                )
            }
        }
    }

    /// 짝맞춤 부호·자동 짝 넣기의 결과를 Effect로 옮긴다. 합성기를 거치지 않으므로
    /// 어절 문맥은 여기서 끊는다 — 괄호나 따옴표 뒤는 새 어절이다.
    fn emit_punctuation(&mut self, outcome: PunctuationOutcome) -> Vec<Effect> {
        let PunctuationOutcome::Commit {
            output,
            cursor_offset,
        } = outcome
        else {
            unreachable!("글자 치환은 합성기를 거친다");
        };
        let mut effects = Vec::new();
        if output.delete_before_commit > 0 {
            effects.push(Effect::DeleteBackward(output.delete_before_commit));
        }
        if let Some(commit) = output.commit {
            effects.push(Effect::CommitText(commit.surface));
        }
        if cursor_offset != 0 {
            effects.push(Effect::MoveCursor(cursor_offset));
        }
        self.touches.clear();
        self.previous_word = None;
        self.reverted_correction = None;
        self.replace_suggestions(Vec::new(), &mut effects);
        effects
    }

    /// 진행 중 조합을 언어별 규칙으로 확정하고 후보 바를 비운다. 커서가 옮겨 가거나
    /// 초점을 잃을 때, 그리고 검색면에서 곁들일 것을 골라 어절을 끝낼 때 함께 쓴다.
    pub(super) fn finalize_composition(&mut self) -> Vec<Effect> {
        let was_composing = self.composer.is_composing();
        let mut effects = Vec::new();
        if let Some(committed) = self.composer.finalize() {
            effects.push(Effect::CommitText(committed.surface));
        }
        if was_composing {
            effects.push(Effect::ClearComposing);
        }
        // 어절이 여기서 끝났으므로 직전 어휘는 더 이상 문맥이 아니다
        self.previous_word = None;
        self.touches.clear();
        self.replace_suggestions(Vec::new(), &mut effects);
        effects
    }
}
