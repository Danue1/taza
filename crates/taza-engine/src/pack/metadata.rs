//! metadata 섹션 — 팩의 출처·라이선스·버전과 스크립트 특성을 담는 키/값 목록.
//! 라이선스 고지 의무(원천별 저작자 표시)와 감사 추적이 팩 자체에 붙어 다니게 한다.
//! 섹션 레이아웃 (little-endian):
//! ```text
//! entry_count u16 | (key_length u8, key utf-8, value_length u16, value utf-8) × n
//! ```
//! 키는 오름차순 정렬 — 이진 탐색 조회. 예약 키는 `keys` 모듈에 모아 둔다.

/// 파이프라인과 리더가 함께 쓰는 예약 키. 그 외 키도 자유롭게 담을 수 있다.
pub mod keys {
    /// 팩 데이터의 판 번호 — 같은 언어의 갱신 배포를 구분한다.
    pub const PACK_VERSION: &str = "pack_version";
    /// 팩을 만든 레시피 이름
    pub const RECIPE: &str = "recipe";
    /// 원천 목록 — 한 줄에 원천 하나이고, 탭으로 나눈 네 칸이 순서대로
    /// `이름 · 판 · 라이선스 · 저작자 표시 문구`다. 이름과 문구를 따로 두면 짝이
    /// 위치로만 맞아, 문구가 빈 원천이 하나 생기는 순간 전부 어긋난다.
    pub const SOURCES: &str = "sources";
    /// lexicon 표제어 수
    pub const WORD_COUNT: &str = "word_count";
    /// 언어모델 섹션에 담긴 bigram 수 (0이면 섹션 없음)
    pub const BIGRAM_COUNT: &str = "bigram_count";
    /// lexicon 표제어의 저장 인코딩 — `utf8` 또는 `hangul-jamo-dubeolsik`
    pub const LEXICON_ENCODING: &str = "lexicon_encoding";
    /// 언어가 자기를 부르는 이름 — 스페이스바와 언어 목록에 그대로 나간다
    pub const DISPLAY_NAME: &str = "display_name";
    /// 언어 키에 찍히는 짧은 표기
    pub const KEYCAP_LABEL: &str = "keycap_label";
    /// 조합 골격 — `direct` / `latin` / `hangul`
    pub const COMPOSER_SKELETON: &str = "composer_skeleton";
    /// 어절 뒤에 붙어 활용형을 만드는 접사 — 줄바꿈으로 나눈 표시 형태 목록.
    /// 학습한 어휘에 이것을 붙여 사전에도 스토어에도 없는 결합형을 제안한다.
    /// 교착어가 아닌 언어의 팩에는 없다.
    pub const AFFIXES: &str = "affixes";
    /// 어절을 공백으로 나누는 스크립트인지 — `true`/`false`
    pub const WORD_SEPARATED: &str = "word_separated";
    /// 오른쪽에서 왼쪽으로 쓰는 스크립트인지 — `true`/`false`
    pub const RIGHT_TO_LEFT: &str = "right_to_left";
}

/// 바이트 슬라이스 위의 zero-copy 메타데이터 뷰. 손상된 데이터는 조회 실패로 처리한다.
pub struct Metadata<'bytes> {
    bytes: &'bytes [u8],
}

impl<'bytes> Metadata<'bytes> {
    pub(crate) fn new(bytes: &'bytes [u8]) -> Self {
        Metadata { bytes }
    }

    fn entry_count(&self) -> usize {
        self.bytes
            .get(0..2)
            .map(|slice| u16::from_le_bytes(slice.try_into().unwrap()) as usize)
            .unwrap_or(0)
    }

    fn entry_at(&self, index: usize) -> Option<(&'bytes str, &'bytes str)> {
        let mut offset = 2;
        for _ in 0..index {
            offset = self.skip_entry(offset)?;
        }
        self.read_entry(offset)
    }

    fn read_entry(&self, offset: usize) -> Option<(&'bytes str, &'bytes str)> {
        let key_length = *self.bytes.get(offset)? as usize;
        let key_start = offset + 1;
        let key = std::str::from_utf8(self.bytes.get(key_start..key_start + key_length)?).ok()?;
        let value_length_offset = key_start + key_length;
        let value_length = u16::from_le_bytes(
            self.bytes
                .get(value_length_offset..value_length_offset + 2)?
                .try_into()
                .unwrap(),
        ) as usize;
        let value_start = value_length_offset + 2;
        let value =
            std::str::from_utf8(self.bytes.get(value_start..value_start + value_length)?).ok()?;
        Some((key, value))
    }

    fn skip_entry(&self, offset: usize) -> Option<usize> {
        let key_length = *self.bytes.get(offset)? as usize;
        let value_length_offset = offset + 1 + key_length;
        let value_length = u16::from_le_bytes(
            self.bytes
                .get(value_length_offset..value_length_offset + 2)?
                .try_into()
                .unwrap(),
        ) as usize;
        Some(value_length_offset + 2 + value_length)
    }

    pub fn get(&self, key: &str) -> Option<&'bytes str> {
        let mut low = 0usize;
        let mut high = self.entry_count();
        while low < high {
            let middle = (low + high) / 2;
            let (entry_key, value) = self.entry_at(middle)?;
            match entry_key.cmp(key) {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
                std::cmp::Ordering::Equal => return Some(value),
            }
        }
        None
    }

    pub fn entries(&self) -> Vec<(&'bytes str, &'bytes str)> {
        (0..self.entry_count())
            .filter_map(|index| self.entry_at(index))
            .collect()
    }
}
