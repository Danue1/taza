//! 오프라인 평가 하네스 — 랭킹·교정 품질의 회귀 게이트.
//!
//! 실사용 로그가 없는 초기에는 레이아웃 기하로 오타를 합성해 (입력, 의도) 평가 셋을
//! 만든다. 시드 고정 xorshift로 결정론을 보장하므로 CI에서 임계값 검증에 쓸 수 있다.
//! 랭킹 가중치·사전 변경은 이 게이트를 통과해야 병합한다.

pub mod synthesis;

use std::sync::Arc;
use taza_engine::contract::{EditorContext, Effect, InputEvent};
use taza_engine::engine::{Engine, PackBytes};
use taza_engine::lang::Language;

/// typed는 실제 입력 시퀀스(한국어는 자모), intended는 화면 표시 형태의 의도 단어.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationCase {
    pub typed: String,
    pub intended: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CorrectionReport {
    pub case_count: usize,
    /// 오타를 다 친 시점에 의도 단어가 제안 1위인 비율
    pub top1_accuracy: f64,
    pub top3_accuracy: f64,
    pub mean_reciprocal_rank: f64,
    /// separator 자동교정이 의도 단어를 확정한 비율
    pub autocorrect_accuracy: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompletionReport {
    pub word_count: usize,
    /// 1 - (사용한 입력 수 / 전체 타이핑 입력 수). 후보 선택 탭은 입력 1회로 계산하며
    /// 선택 시 후행 공백이 따라오므로 기준선은 입력 시퀀스 길이 + 공백 1회다.
    pub keystroke_savings: f64,
}

/// 완성 평가 과제 — typed 시퀀스를 치는 동안 intended가 top-3에 오르면 선택한 것으로 본다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionTask {
    pub typed: String,
    pub intended: String,
}

struct Typist {
    engine: Engine,
    committed: String,
    candidates: Vec<String>,
}

impl Typist {
    /// 평가는 항상 빈 상태에서 시작한다 — 과제마다 엔진을 새로 만든다.
    fn new(language: Language, pack: &Arc<dyn PackBytes>) -> Self {
        let mut engine = Engine::new(language).expect("이 빌드에 없는 언어");
        engine.load_pack(Arc::clone(pack)).expect("팩 열기 실패");
        Typist {
            engine,
            committed: String::new(),
            candidates: Vec::new(),
        }
    }

    fn send(&mut self, event: InputEvent) {
        let context = EditorContext {
            text_before_cursor: Some(self.committed.clone()),
            incognito: false,
            field: taza_engine::contract::FieldKind::Text,
        };
        for effect in self.engine.handle(event, &context) {
            match effect {
                Effect::CommitText(text) => self.committed.push_str(&text),
                Effect::DeleteBackward(count) => {
                    for _ in 0..count {
                        self.committed.pop();
                    }
                }
                Effect::UpdateCandidates(candidates) => {
                    self.candidates = candidates
                        .into_iter()
                        .map(|candidate| candidate.text)
                        .collect();
                }
                Effect::SetComposing(_) | Effect::ClearComposing => {}
                Effect::MoveCursor(_) => panic!("평가 하네스는 커서를 옮기지 않는다"),
            }
        }
    }

    fn type_word(&mut self, word: &str) {
        for character in word.chars() {
            self.send(InputEvent::Key(character));
        }
    }
}

pub fn evaluate_corrections(
    pack: &Arc<dyn PackBytes>,
    language: Language,
    cases: &[EvaluationCase],
) -> CorrectionReport {
    let mut top1 = 0usize;
    let mut top3 = 0usize;
    let mut reciprocal_rank_sum = 0.0f64;
    let mut autocorrected = 0usize;
    for case in cases {
        let mut typist = Typist::new(language, pack);
        typist.type_word(&case.typed);
        if let Some(rank) = typist
            .candidates
            .iter()
            .position(|candidate| candidate == &case.intended)
        {
            if rank == 0 {
                top1 += 1;
            }
            if rank < 3 {
                top3 += 1;
            }
            reciprocal_rank_sum += 1.0 / (rank + 1) as f64;
        }
        typist.send(InputEvent::Separator(' '));
        if typist.committed == format!("{} ", case.intended) {
            autocorrected += 1;
        }
    }
    let count = cases.len().max(1) as f64;
    CorrectionReport {
        case_count: cases.len(),
        top1_accuracy: top1 as f64 / count,
        top3_accuracy: top3 as f64 / count,
        mean_reciprocal_rank: reciprocal_rank_sum / count,
        autocorrect_accuracy: autocorrected as f64 / count,
    }
}

pub fn evaluate_completions(
    pack: &Arc<dyn PackBytes>,
    language: Language,
    tasks: &[CompletionTask],
) -> CompletionReport {
    let mut savings_sum = 0.0f64;
    for task in tasks {
        let typed_length = task.typed.chars().count();
        let baseline = (typed_length + 1) as f64;
        let mut typist = Typist::new(language, pack);
        let mut used: Option<usize> = None;
        for (typed_count, character) in task.typed.chars().enumerate() {
            typist.send(InputEvent::Key(character));
            let in_top3 = typist
                .candidates
                .iter()
                .take(3)
                .any(|candidate| candidate == &task.intended);
            if in_top3 && typed_count + 1 < typed_length {
                used = Some(typed_count + 1 + 1); // 입력한 글자 + 선택 탭
                break;
            }
        }
        let used = used.unwrap_or(typed_length + 1) as f64;
        savings_sum += 1.0 - used / baseline;
    }
    CompletionReport {
        word_count: tasks.len(),
        keystroke_savings: savings_sum / tasks.len().max(1) as f64,
    }
}
