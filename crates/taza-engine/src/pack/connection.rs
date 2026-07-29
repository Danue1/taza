//! connection 섹션 — 말과 말이 이어질 때 드는 값.
//!
//! 변환의 어려움은 낱말을 고르는 데 있지 않고 **어디서 끊을지**에 있다. 「にわにはにわ」를
//! 「庭には二羽」로 끊는 근거는 낱말 각각의 흔함이 아니라 그 낱말들이 이어질 만한가이고,
//! 그것을 재는 것이 이 표다. 앞말이 나가는 자리(`right_id`)와 뒷말이 들어오는 자리
//! (`left_id`)가 만나는 칸의 값이 그 이음의 비용이다.
//!
//! 섹션 레이아웃 (little-endian):
//! ```text
//! row_count u16 (= right_id 가짓수) | column_count u16 (= left_id 가짓수)
//! | cost i16 × (row_count × column_count)
//! ```
//! 행이 앞말, 열이 뒷말이다. 표가 없는 팩에서는 이음마다 같은 값이 들므로 변환은
//! 낱말 비용만으로 이뤄진다 — 품질은 떨어지지만 성립은 한다.

/// 표가 없을 때 모든 이음에 드는 값. 0으로 두면 낱말을 잘게 쪼갤수록 유리해져
/// 「はし」가 「は」+「し」로 갈리므로, 마디 하나를 세우는 값을 매긴다.
pub const DEFAULT_CONNECTION_COST: i32 = 500;

/// 바이트 슬라이스 위의 zero-copy 연접 행렬.
#[derive(Debug, Clone, Copy)]
pub struct ConnectionMatrix<'bytes> {
    bytes: &'bytes [u8],
}

impl<'bytes> ConnectionMatrix<'bytes> {
    pub(crate) fn new(bytes: &'bytes [u8]) -> Self {
        ConnectionMatrix { bytes }
    }

    fn read_u16(&self, offset: usize) -> Option<u16> {
        self.bytes
            .get(offset..offset + 2)
            .map(|slice| u16::from_le_bytes(slice.try_into().unwrap()))
    }

    pub fn row_count(&self) -> usize {
        self.read_u16(0).unwrap_or(0) as usize
    }

    pub fn column_count(&self) -> usize {
        self.read_u16(2).unwrap_or(0) as usize
    }

    /// 앞말이 `right_id`로 나가고 뒷말이 `left_id`로 들어올 때 드는 값. 표 밖의 자리는
    /// 기본값으로 물러난다 — 사전과 표의 판이 어긋나도 변환이 멈추지 않아야 한다.
    pub fn cost(&self, previous_right_id: u16, next_left_id: u16) -> i32 {
        let columns = self.column_count();
        let (row, column) = (previous_right_id as usize, next_left_id as usize);
        if row >= self.row_count() || column >= columns {
            return DEFAULT_CONNECTION_COST;
        }
        let offset = 4 + (row * columns + column) * 2;
        self.bytes
            .get(offset..offset + 2)
            .map(|slice| i16::from_le_bytes(slice.try_into().unwrap()) as i32)
            .unwrap_or(DEFAULT_CONNECTION_COST)
    }
}
