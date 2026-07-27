//! 합성기 뒤에 붙는 것들 — 랭킹·자동교정·대치 표·학습. 합성기가 낸 결과를 받아
//! Effect 목록으로 옮기는 이 한 갈래에 어절의 운명이 모여 있다.
//!
//! `feed`는 순서만 세우고 판단은 단계가 한다: 어절이 끝났으면 `confirm_boundary`가
//! 그 자리에 무엇이 들어갈지 정하고, `finish_word`가 그것을 배우고 다음 낱말을 고른다.

use crate::contract::{
    Candidate, ComposerEvent, ComposerOutput, EditorContext, Effect, Pack, SuggestionRequest,
    WordBoundary,
};
use crate::policy::Assistance;
use crate::suggest::{Suggestion, SuggestionSources};

use super::Engine;

/// 자동교정이 갈아치운 내용. 되돌리기는 확정 텍스트를 원문으로 되돌려 놓기만 하고,
/// 이어지는 합성은 문맥 채택(adopt)이 알아서 잇는다.
pub(super) struct Correction {
    /// 사용자가 실제로 친 형태
    original: String,
    /// 그 자리에 들어간 교정 결과와 경계 문자를 합친 확정 텍스트
    committed: String,
}

/// 어절이 끝난 자리에 실제로 들어가는 것 — 사용자가 친 그대로일 수도, 대치 표나
/// 자동교정이 갈아치운 것일 수도 있다.
struct Confirmed {
    /// 학습·언어모델 문맥이 될 조회 키
    key: String,
    /// 갈아치우느라 지워야 하는 글자 수
    delete_before_commit: usize,
    /// 그 자리에 들어갈 텍스트 — 경계 문자까지 붙은 것
    text: String,
}

impl Engine {
    /// 합성기를 돌린 뒤 랭킹·자동교정·학습을 얹어 Effect로 옮긴다.
    /// `selected`는 후보 선택으로 어절이 끝난 경우의 그 후보다.
    pub(super) fn feed(
        &mut self,
        event: ComposerEvent,
        context: &EditorContext,
        selected: Option<Suggestion>,
    ) -> Vec<Effect> {
        // Arc를 지역으로 복제해 팩 바이트 대여와 엔진의 가변 대여를 분리한다
        let holder = self.pack.clone();
        let pack = holder
            .as_ref()
            .and_then(|holder| Pack::open(holder.bytes()).ok());
        let was_composing = self.composer.is_composing();
        let output = self.run_composer(event, context);
        let assistance =
            crate::policy::assistance(&self.preferences, self.keyboard.traits(), context);

        let ComposerOutput {
            mut delete_before_commit,
            commit,
            composing,
            boundary,
            suggest,
        } = output;
        let mut commit_text = commit.map(|text| text.surface).unwrap_or_default();

        // 되돌릴 교정은 바로 다음 입력까지만 유효하다 (순정 키보드 관습)
        self.reverted_correction = None;

        // 어절이 끝났는가 — 경계 문자를 쳤거나 후보를 골랐거나
        let confirmed = match boundary {
            Some(boundary) => {
                let resolved = self.confirm_boundary(boundary, pack.as_ref(), assistance);
                delete_before_commit += resolved.delete_before_commit;
                commit_text.push_str(&resolved.text);
                Some(resolved.key)
            }
            None => selected.map(|suggestion| suggestion.key),
        };

        let suggestions = match confirmed {
            Some(key) => self.finish_word(key, pack.as_ref(), assistance, context),
            None => match &suggest {
                SuggestionRequest::Word { key } if assistance.predicting => self
                    .suggester
                    .suggest(key, &self.sources(pack.as_ref(), assistance)),
                _ => Vec::new(),
            },
        };

        let mut effects = Vec::new();
        if delete_before_commit > 0 {
            effects.push(Effect::DeleteBackward(delete_before_commit));
        }
        if !commit_text.is_empty() {
            effects.push(Effect::CommitText(commit_text));
        }
        match composing {
            Some(composing) => effects.push(Effect::SetComposing(composing)),
            None if was_composing => effects.push(Effect::ClearComposing),
            None => {}
        }
        self.replace_suggestions(suggestions, &mut effects);
        effects
    }

    /// 경계 문자로 끝난 어절의 자리에 무엇이 들어갈지 정한다. 손수 적은 대치가
    /// 먼저고, 없으면 자동교정이, 그것도 없으면 친 그대로가 들어간다.
    fn confirm_boundary(
        &mut self,
        boundary: WordBoundary,
        pack: Option<&Pack<'_>>,
        assistance: Assistance,
    ) -> Confirmed {
        // 어절이 비어 있으면 갈아치울 것도 없다 — 부호를 잇달아 치는 자리가 그렇다
        if boundary.surface.is_empty() {
            return Confirmed {
                key: boundary.key,
                delete_before_commit: 0,
                text: boundary.separator.to_string(),
            };
        }
        // 사용자가 손수 정한 대치가 사전 교정보다 세다 — 자동교정은 사전이 미루어
        // 짐작한 것이고 이쪽은 사람이 그렇게 하라고 적어 둔 것이다.
        let replaced = match self.shortcuts.get(&boundary.surface) {
            Some(replacement) => Some((replacement.clone(), boundary.key.clone())),
            None if assistance.correcting => self
                .suggester
                .autocorrection(&boundary.key, &self.sources(pack, assistance))
                .map(|correction| (correction.text, correction.key)),
            None => None,
        };
        let Some((text, key)) = replaced else {
            return Confirmed {
                key: boundary.key,
                delete_before_commit: 0,
                text: boundary.separator.to_string(),
            };
        };
        let committed = format!("{}{}", text, boundary.separator);
        self.reverted_correction = Some(Correction {
            original: boundary.surface.clone(),
            committed: committed.clone(),
        });
        Confirmed {
            key,
            delete_before_commit: boundary.surface.chars().count(),
            text: committed,
        }
    }

    /// 어절 하나가 끝났다 — 배우고, 언어모델 문맥으로 세우고, 다음 낱말을 고른다.
    fn finish_word(
        &mut self,
        key: String,
        pack: Option<&Pack<'_>>,
        assistance: Assistance,
        context: &EditorContext,
    ) -> Vec<Suggestion> {
        // 어절이 끝났으므로 다음 어절은 새 터치 신호로 시작한다
        self.touches.clear();
        // 시크릿 필드는 학습만 막고 조회는 그대로 둔다 — 순정 키보드도
        // 이미 배운 말은 시크릿에서 계속 제안한다
        if assistance.personalizing && !context.incognito && !key.is_empty() {
            self.personalization.record(&key);
        }
        self.previous_word = (!key.is_empty()).then_some(key);
        if !assistance.predicting {
            return Vec::new();
        }
        self.suggester.predict_next(&self.sources(pack, assistance))
    }

    /// 합성기를 돌리기 전에 언어와 무관한 규칙을 먼저 본다. 지금은 더블 스페이스
    /// 마침표 하나뿐이고, 조합 중에는 성립할 수 없으므로 합성기 상태를 건드리지 않는다.
    fn run_composer(&mut self, event: ComposerEvent, context: &EditorContext) -> ComposerOutput {
        if event == ComposerEvent::Separator(' ')
            && self.preferences.double_space_period
            && !self.composer.is_composing()
            && let Some(output) = crate::policy::double_space_period(context)
        {
            return output;
        }
        self.composer.feed(event, context)
    }

    /// 자동교정 직후의 Backspace — 교정 결과를 지우고 사용자가 친 원문을 되살린다.
    /// 이어지는 타이핑은 문맥 채택(adopt)이 알아서 그 어절을 잇는다.
    pub(super) fn revert(&mut self, correction: Correction) -> Vec<Effect> {
        self.previous_word = None;
        let mut effects = vec![
            Effect::DeleteBackward(correction.committed.chars().count()),
            Effect::CommitText(correction.original),
        ];
        self.replace_suggestions(Vec::new(), &mut effects);
        effects
    }

    pub(super) fn sources<'call>(
        &'call self,
        pack: Option<&'call Pack<'call>>,
        assistance: Assistance,
    ) -> SuggestionSources<'call> {
        SuggestionSources {
            pack,
            personalization: assistance.personalizing.then_some(&self.personalization),
            previous_word: self.previous_word.as_deref(),
            touches: &self.touches,
        }
    }

    pub(super) fn replace_suggestions(
        &mut self,
        suggestions: Vec<Suggestion>,
        effects: &mut Vec<Effect>,
    ) {
        if suggestions.is_empty() && self.suggestions.is_empty() {
            return;
        }
        let candidates = suggestions
            .iter()
            .map(|suggestion| Candidate {
                text: suggestion.text.clone(),
                kind: suggestion.kind.clone(),
                group: suggestion.group,
            })
            .collect();
        self.suggestions = suggestions;
        effects.push(Effect::UpdateCandidates(candidates));
    }
}
