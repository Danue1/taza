//! 오프라인 평가 하네스 — 랭킹·교정 품질의 회귀 게이트.
//!
//! 실사용 로그가 없는 초기에는 레이아웃 기하로 오타를 합성해 (입력, 의도) 평가 셋을
//! 만든다. 시드 고정 xorshift로 결정론을 보장하므로 CI에서 임계값 검증에 쓸 수 있다.
//! 랭킹 가중치·사전 변경은 이 게이트를 통과해야 병합한다.

pub mod synthesis;

use synthesis::TypedSequence;

use std::sync::Arc;
use taza_engine::contract::{CandidateKind, EditorContext, Effect, InputEvent};
use taza_engine::engine::{Engine, PackBytes};
use taza_engine::lang::LanguageDescriptor;

/// typed는 실제로 친 입력(글자와 터치 좌표, 한국어는 자모), intended는 의도 단어의
/// 화면 표시 형태다. 좌표까지 담는 이유는 공간 모델이 읽는 신호가 좌표이기 때문이다.
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluationCase {
    pub typed: TypedSequence,
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

/// 오교정 평가 결과 — 제대로 친 미등재 낱말을 자동교정이 얼마나 건드리는가.
/// 교정 정확도만 재면 "무엇이든 교정할수록 좋다"는 잘못된 방향으로 튜닝하게 된다.
#[derive(Debug, Clone, PartialEq)]
pub struct FalseCorrectionReport {
    pub word_count: usize,
    /// 원문 그대로 확정되지 않고 다른 낱말로 갈아치워진 비율
    pub false_correction_rate: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompletionReport {
    pub word_count: usize,
    /// 1 - (사용한 입력 수 / 전체 타이핑 입력 수). 후보 선택 탭은 입력 1회로 계산하며
    /// 선택 시 후행 공백이 따라오므로 기준선은 입력 시퀀스 길이 + 공백 1회다.
    pub keystroke_savings: f64,
}

/// 완성 평가 과제 — typed 시퀀스를 치는 동안 intended가 top-3에 오르면 선택한 것으로 본다.
#[derive(Debug, Clone, PartialEq)]
pub struct CompletionTask {
    pub typed: TypedSequence,
    pub intended: String,
}

struct Typist {
    engine: Engine,
    committed: String,
    /// 원문 슬롯을 뺀 후보들 — 그 자리는 랭킹의 산물이 아니라 "친 대로 두기"라서
    /// 순위에 섞으면 어떤 사전으로도 top1이 오르지 않는다. 슬롯이 자리를 차지해
    /// 표시되는 후보가 하나 줄어든 것은 이 목록에 그대로 반영된다.
    candidates: Vec<String>,
}

impl Typist {
    /// 평가는 항상 빈 상태에서 시작한다 — 과제마다 엔진을 새로 만든다.
    fn new(language: &LanguageDescriptor, pack: &Arc<dyn PackBytes>) -> Self {
        let mut engine = Engine::new(language.clone()).expect("이 빌드에 없는 골격");
        engine.load_pack(Arc::clone(pack)).expect("팩 열기 실패");
        Typist {
            engine,
            committed: String::new(),
            candidates: Vec::new(),
        }
    }

    fn context(&self) -> EditorContext {
        EditorContext {
            text_before_cursor: Some(self.committed.clone()),
            incognito: false,
            field: taza_engine::contract::FieldKind::Text,
        }
    }

    /// 실제 타이핑처럼 좌표로 누른다 — 코어가 그 자리에서 키 신호를 만든다.
    fn press(&mut self, x: f32, y: f32) {
        let context = self.context();
        let effects = self.engine.press_at(x, y, &context).effects;
        self.apply(effects);
    }

    fn send(&mut self, event: InputEvent) {
        let context = self.context();
        let effects = self.engine.handle(event, &context);
        self.apply(effects);
    }

    fn apply(&mut self, effects: Vec<Effect>) {
        for effect in effects {
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
                        .filter(|candidate| candidate.kind != CandidateKind::Typed)
                        .map(|candidate| candidate.text)
                        .collect();
                }
                Effect::SetComposing(_) | Effect::ClearComposing => {}
                // 평가 하네스에는 시간이 흐르지 않는다 — 멀티탭 주기가 끊기는 일이 없다
                Effect::SetTimer(_) => {}
                Effect::MoveCursor(_) => panic!("평가 하네스는 커서를 옮기지 않는다"),
            }
        }
    }

    fn type_sequence(&mut self, typed: &TypedSequence) {
        for &(x, y) in &typed.touches {
            self.press(x, y);
        }
    }
}

pub fn evaluate_corrections(
    pack: &Arc<dyn PackBytes>,
    language: &LanguageDescriptor,
    cases: &[EvaluationCase],
) -> CorrectionReport {
    let mut top1 = 0usize;
    let mut top3 = 0usize;
    let mut reciprocal_rank_sum = 0.0f64;
    let mut autocorrected = 0usize;
    for case in cases {
        let mut typist = Typist::new(language, pack);
        typist.type_sequence(&case.typed);
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

/// 사전에 없지만 올바른 낱말들을 **오타 없이** 친 뒤 어절을 끝냈을 때, 자동교정이
/// 원문을 갈아치우는 비율. 낮을수록 좋다.
/// 평가 사례의 `typed`는 오타 없이 친 입력이고 `intended`는 화면에 나와야 할 형태다 —
/// 한국어는 입력이 자모열이고 표시가 한글이라 둘을 따로 두어야 한다.
pub fn evaluate_false_corrections(
    pack: &Arc<dyn PackBytes>,
    language: &LanguageDescriptor,
    cases: &[EvaluationCase],
) -> FalseCorrectionReport {
    let mut changed = 0usize;
    for case in cases {
        let mut typist = Typist::new(language, pack);
        typist.type_sequence(&case.typed);
        typist.send(InputEvent::Separator(' '));
        if typist.committed != format!("{} ", case.intended) {
            changed += 1;
        }
    }
    FalseCorrectionReport {
        word_count: cases.len(),
        false_correction_rate: changed as f64 / cases.len().max(1) as f64,
    }
}

pub fn evaluate_completions(
    pack: &Arc<dyn PackBytes>,
    language: &LanguageDescriptor,
    tasks: &[CompletionTask],
) -> CompletionReport {
    let mut savings_sum = 0.0f64;
    for task in tasks {
        let typed_length = task.typed.touches.len();
        let baseline = (typed_length + 1) as f64;
        let mut typist = Typist::new(language, pack);
        let mut used: Option<usize> = None;
        for (typed_count, &(x, y)) in task.typed.touches.iter().enumerate() {
            typist.press(x, y);
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
