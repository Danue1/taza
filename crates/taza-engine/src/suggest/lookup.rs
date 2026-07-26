//! 친 어절 → 조회 키, 그리고 사전이 낸 표제어 → 화면에 낼 꼴.
//!
//! 사전은 표제어를 한 벌만 담는다 — 소문자에 곧은 아포스트로피다. 같은 낱말을 꼴마다
//! 싣는 것은 팩 크기를 몇 배로 불리는 일이고, 그렇게 해도 새 꼴이 나오면 또 놓친다.
//! 대신 여기서 친 것을 사전이 아는 꼴로 접고, 사전이 낸 것에 친 꼴을 되씌운다.
//!
//! 접지 않으면 어떻게 되는지가 이 모듈이 있는 이유다:
//! - 자동 대문자화가 문장 첫 자리마다 shift를 올리므로, 대문자를 접지 않으면 문장을
//!   여는 낱말마다 예측이 끊기고 대문자 하나가 오타로 계산돼 엉뚱한 낱말이 올라온다
//!   ("Thi" → "hi", "chi").
//! - 짝맞춤 부호가 아포스트로피를 U+2019로 바꾸므로, 접지 않으면 축약형이 통째로
//!   사전에 닿지 못한다("don't", "it's").

const STRAIGHT_APOSTROPHE: char = '\'';
const CURLY_APOSTROPHE: char = '\u{2019}';

/// 조회 키로 접으면서 덜어낸 것 — 사전이 낸 표제어에 그대로 되씌운다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Restore {
    casing: Casing,
    /// 친 어절이 짝맞춤 아포스트로피를 쓰고 있었는가. 후보도 같은 글자로 보여야
    /// 고른 것과 친 것이 화면에서 달라 보이지 않는다.
    curly_apostrophe: bool,
}

/// 되씌울 것이 없는 상태 — 접힌 적 없는 키가 쓴다.
pub(crate) const VERBATIM: Restore = Restore {
    casing: Casing::Lower,
    curly_apostrophe: false,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Casing {
    /// 되씌울 것이 없다 — 표제어를 그대로 낸다
    Lower,
    /// 첫 글자만 대문자 (문장 첫 자리·고유명사)
    Initial,
    /// 전부 대문자
    Upper,
}

impl Restore {
    /// 표제어에 친 꼴을 되씌운다.
    pub(crate) fn apply(self, text: &str) -> String {
        let text = match self.casing {
            Casing::Lower => text.to_string(),
            Casing::Upper => text.to_uppercase(),
            Casing::Initial => {
                let mut characters = text.chars();
                match characters.next() {
                    Some(first) => first.to_uppercase().chain(characters).collect(),
                    None => String::new(),
                }
            }
        };
        match self.curly_apostrophe {
            true => text.replace(STRAIGHT_APOSTROPHE, &CURLY_APOSTROPHE.to_string()),
            false => text,
        }
    }
}

/// 친 어절을 조회 키로 접는다. 접을 것이 없거나 접기가 안전하지 않으면 None이며,
/// 그때는 어절을 그대로 조회한다.
///
/// 소문자가 여러 글자가 되는 글자(İ)는 접지 않는다 — 그런 글자가 섞인 어절은 어차피
/// 사전에 없고, 접어 봐야 되씌울 때 원문이 망가진다.
pub(crate) fn fold(word: &str) -> Option<(String, Restore)> {
    let casing = detect(word);
    let curly_apostrophe = word.contains(CURLY_APOSTROPHE);
    // 접을 것이 대소문자에도 아포스트로피에도 없다
    if matches!(casing, Some(Casing::Lower)) && !curly_apostrophe {
        return None;
    }
    // 대소문자 꼴을 알아볼 수 없는 어절(McD)이라도 아포스트로피는 접어야 한다
    let casing = casing.unwrap_or(Casing::Lower);
    let mut key = String::with_capacity(word.len());
    for character in word.chars() {
        if character == CURLY_APOSTROPHE {
            key.push(STRAIGHT_APOSTROPHE);
            continue;
        }
        let mut lower = character.to_lowercase();
        match (lower.next(), lower.next()) {
            (Some(single), None) => key.push(single),
            _ => return None,
        }
    }
    Some((
        key,
        Restore {
            casing,
            curly_apostrophe,
        },
    ))
}

/// 되씌울 대소문자 꼴. 소문자뿐이면 `Lower`이고, 꼴을 알아볼 수 없게 섞여 있으면(McD)
/// None — 그런 어절에 첫 글자 대문자를 되씌우면 원문이 망가진다.
fn detect(word: &str) -> Option<Casing> {
    let mut cased = 0usize;
    let mut upper = 0usize;
    let mut first_upper = false;
    for (index, character) in word.chars().enumerate() {
        if !character.is_uppercase() && !character.is_lowercase() {
            continue;
        }
        cased += 1;
        if character.is_uppercase() {
            upper += 1;
            first_upper |= index == 0;
        }
    }
    match (upper, cased) {
        (0, _) => Some(Casing::Lower),
        (1, _) if first_upper => Some(Casing::Initial),
        (upper, cased) if upper == cased && cased > 1 => Some(Casing::Upper),
        _ => None,
    }
}
