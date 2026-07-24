# 크로스플랫폼 키보드 아키텍처 청사진 v2
(v1에 대한 3방향 적대 검토 — 플랫폼 제약 / IME 도메인 / 엔진 조달·FFI — 반영판)

## 목표
- iOS(Keyboard Extension) + Android(InputMethodService) 키보드 앱
- 각 플랫폼이 지원하는 모든 언어 지원 (장기). 구현 순서: 영어 → 한국어 → 일본어,
  나머지는 추후 — 단 계약·팩 포맷은 추후 언어 특성(RTL, 무공백 스크립트 등)을 선반영
- sans-io 코어로 플랫폼 간 코어 동작 동일성 보장
- 언어별 composing 관습(UX) 최대한 재현
- 사전은 언어별 분리, 미리 다운로드한 언어만 사용, 영어는 앱에 내장(기본 언어)

## 전체 구조
```
Shell (플랫폼별)
  iOS: UIInputViewController / textDocumentProxy
  Android: InputMethodService / InputConnection
FFI: UniFFI (측정 게이트 아래 채택, 핫패스 C ABI 예비안)
Core (Rust, sans-io)
  fn handle(event: InputEvent, context: EditorContext) -> Vec<Effect>
```

### 셸 원칙 (v2 재서술)
- 셸은 **입력 의미론**(composition·commit·삭제·후보 선택)에 분기를 갖지 않는다.
- 플랫폼 강제 분기는 화이트리스트로 명시하고 그 외 분기는 리뷰에서 거부:
  지구본 키 / inputType·UIKeyboardType별 레이아웃 요청(코어 입력으로 승격) /
  리턴키 라벨 / iPad 플로팅·분할 / 인라인 자동완성 제안(Android 11+) /
  Material You 동적 색상(테마 토큰 주입으로 코어 입력화) /
  IME_FLAG_NO_PERSONALIZED_LEARNING(EditorContext.incognito 플래그로 코어 전달)

### EditorContext (v2 — "앞뒤 N자" 계약 폐기)
- 문맥은 보장되지 않는다: `Option<ContextWindow>` + 신뢰도(fresh/stale/unavailable).
  iOS는 0~약300자·문단 절단·앱별 nil이 일상, Android InputConnection도 비동기·무응답 채널.
- 계약 기준은 "iOS"가 아니라 **양 플랫폼 실패 모드의 합집합**:
  문맥 비보장 + Effect 적용 비확인.
- Effect는 적용됐다고 가정하되, 다음 문맥 스냅샷과 불일치 시 재동기화하는
  reconciliation 규칙을 코어가 소유.
- 골든 코퍼스에 "문맥 0자" 케이스 필수 포함. 예측·교정 품질 설계도 문맥 0자에서 동작해야 함.

### composing 반영 전략 (v2 — "감지 후 fallback" 폐기)
- 호스트 앱의 marked text 지원 여부를 감지할 방법이 없다(양 플랫폼 공통).
- 따라서 fallback은 감지가 아니라 **언어별 정책**: 각 Composer가 iOS 안전 모드
  (예: 한국어는 marked text 대신 음절 단위 commit+delete)를 1급 출력 전략으로 제공.
  전략 선택은 코어 계약에 있고 셸 휴리스틱은 없다.

### 삭제 계약 (v2)
- `DeleteBackward`는 문자 수가 아니라 **코드포인트 수**(코어가 문맥 스냅샷에서
  그래핌·스크립트 규칙으로 산출)로 정의.
- iOS `deleteBackward()`는 count 미보장 API임을 셸 계약에 명시 — 삭제 후 문맥 재동기화 필수.
- 스크립트별 삭제 단위 정책 테이블(이모지 ZWJ=통째, 인도계 클러스터=코드포인트 등)을
  골든 코퍼스에 포함.

## 메모리·생명주기 (v2)
- 예산은 **최악 기기 하한 48MB** 기준(실측 산포 48~70MB, 상한 비공개·기기 의존).
- "힙 로드 금지·mmap 조회 전용"은 **코어 자체 사전에 한함**. C++ 엔진은 언어별 힙 예산
  상한을 두고, 로드 실패 시 해당 언어 비활성.
- 익스텐션 재생성은 키보드 표시마다의 일상 — 콜드 스타트 예산(키보드 표시까지 X ms)을
  명시하고, 지연 로딩 순서(레이아웃 먼저, 사전 나중)와 스냅샷 크기 상한을 설계에 포함.
- 사전 다운로드는 컨테이너 앱 전담, App Group 전달 (v1 유지 — 검토 통과).

## FFI 계약 (v2 — 이원화)
- **키 확정 이벤트**: 이벤트당 1회 왕복 (µs 오더, 문제 없음).
- **포인터 스트림**(다운/무브/업, 글라이드): 프레임 단위 배칭으로 왕복.
  시각적 프레스 하이라이트는 셸이 KeyboardFrame에 포함된 키 바운드로 즉시 처리
  (판정은 여전히 코어, 표시만 셸 선반영).
- KeyboardFrame은 전체 재전송이 아니라 diff/버전 넘버.
- 측정 게이트: 실기기 keydown→setMarkedText p99 실측 후 채택 확정,
  초과 시 핫패스만 수동 C ABI로 하강.

## 렌더링 (v1 결론 유지 — 검토 통과)
- Flutter/Compose Multiplatform 기각(익스텐션 메모리·재생성 초기화 지연).
- 코어가 KeyboardFrame 소유, 셸은 네이티브 렌더러.
- **접근성은 비통일 영역이 아니라 계약의 일부** (v2): KeyboardFrame에 키별 접근성
  라벨·롤·상태 포함. VoiceOver/TalkBack 터치 탐색은 OS가 히트 테스트를 대신하므로,
  접근성 경로에서는 확률 판정 없이 명시적 키 활성화 이벤트로 코어 진입.

## Composition 설계 (v2)

```rust
trait Composer {
    fn feed(&mut self, event: ComposerEvent, dictionary: Option<&Dictionary>) -> ComposerOutput;
    fn finalize(&mut self) -> Option<CommittedText>;
    fn snapshot(&self) -> ComposerState;
}
enum ComposerEvent {
    Key(KeyId), Backspace, CandidateSelected(usize), Separator(char),
    TimerFired(TimerId),                    // v2: 멀티탭(천지인·케이타이) 타임아웃
    CandidatePageRequested,                 // v2: 후보 페이지네이션
}
struct ComposerOutput {
    delete_before_commit: usize,            // v2: 확정 텍스트 치환 채널 (그래핌 단위)
    commit: Option<CommittedText>,
    composing: Option<ComposingText>,
    candidates: Vec<Candidate>,
    timer: Option<TimerRequest>,            // v2: Effect::SetTimer로 번역
}
struct CommittedText {                      // v2: String → 메타데이터 보존
    surface: String,
    reading: Option<String>,                // 재변환·학습·확정 취소용
    corrected_from: Option<String>,         // 자동교정 되돌리기용
}
struct Candidate { text, kind /* 예측|변환|교정 */, commit_policy /* 후행 공백 등 */ }
```

- Backspace는 Composer 내부 처리, composing 비어 있을 때만 셸 위임 (v1 유지).
- **확정 취소·재변환**: 코어가 최근 CommittedText 이력을 보관, commit 직후 Backspace는
  delete_before_commit로 확정분을 지우고 composing 복원.
- 후보 선택 후 연속 타이핑: 버퍼 전체 소비 시 리셋(새 시퀀스), 일부 소비 시 잔여 버퍼로
  재시작 (v1 유지 — 검토 통과).
- finalize의 언어별 정책을 명시: 병음=원문 로마자 확정, 일본어=1위 후보 또는 가나 확정,
  한국어=현재 음절 확정. 골든 코퍼스에 finalize 케이스 포함.

### Composer 골격 (v2 — 3개 → 4개)
| 골격 | 특성 | 언어 |
|---|---|---|
| Direct | composing 없음, 팝업/데드키 (CLDR 데이터) | 라틴 전반 |
| Validated | Direct + 문맥 1자 수용/거부 + composing 없는 사전 제안·치환 | 태국어(WTT 2.0), 크메르어(coeng 정규화), 버마어 |
| Automaton | 결정적 상태 기계 + 타이머 | 한국어(두벌식·천지인), 베트남어(텔렉스), 인도계 네이티브 |
| Convert | 원시 버퍼 + 사전 변환 + 후보 바 | 일본어, 중국어, 인도계 음역 |

- 한국어 도깨비불 (v2 수정): "즉시 확정" 폐기. **composing 창을 직전 1음절 + 현재 음절로
  유지**해 도깨비불 전이 시에도 marked text 안에서 처리 — Backspace로 "가바"→"갑" 복귀가
  Composer 내부에서 자연스럽게 성립. 확정은 창이 밀려날 때.
- 베트남어: old style(òa)/new style(oà) 성조 표기 양쪽 지원 + 외래어 이스케이프(더블 타이핑
  복원)를 요구사항에 포함 — 후자는 영어 사전 조회 필요(순수 오토마타 아님).
- Validated 골격의 순서 검증은 문맥 1자 필요 — 문맥 unavailable 시 검증 생략을 정책으로 명시.

## 엔진·컴포넌트 조달 (v2)
| 컴포넌트 | 방침 |
|---|---|
| 한국어·베트남어·인도계 네이티브 조합 | 직접 구현 (조합 오토마톤은 소규모 — 단 교정·예측은 별도 계상) |
| 라틴 Direct + CLDR 데이터 | 직접 구현 |
| **라틴(+한국어) 자동교정·예측 엔진** | **직접 구현, 최대 단일 컴포넌트로 승격 계상**: n-gram LM + 편집거리/FST + 공간(터치) 모델. v1은 키 기하 기반 Gaussian 프라이어 + 온디바이스 적응(스냅샷에 포함), 로그 부재로 인한 초기 품질 한계를 리스크에 기록. **1차 구현됨(LatinComposer)**: lexicon 완성 + OSA 거리 1 교정 + separator 자동교정 + 제안 치환 — LM·공간 모델·개인화는 미착수 |
| 중국어 | librime(BSD-3) 채택 검토 — 조건부 (아래) |
| 일본어 | **AzooKeyKanaKanjiConverter(MIT, 사전 Apache-2.0) 1순위** — iOS 상용 실증(azooKey). Android는 동일 사전 포맷(LOUDS) 자체 리더 또는 Swift-for-Android 스파이크. **mozc는 엔진 채택 탈락**(iOS 공식 미지원, OSS 사전은 IPAdic 라이선스·품질 하향판) — connection data 참고 자료로만 |
| 인도계 음역 | 직접 구현 |

### C++/외부 엔진 어댑터 조건 (v2 — "격리 선언"에서 강제 조건으로)
1. 엔진+데이터를 단일 소스에서 양 플랫폼 크로스 빌드, 산출물 해시 고정 (벤더링 드리프트 차단)
2. 학습(사용자 사전)은 엔진 밖 **코어 소유** — 엔진은 무상태 조회기로 강등
   (librime LevelDB 학습 사전은 익스텐션 쓰기 제약·mmap 원칙과 충돌하므로 비활성)
3. 스키마 배포 등 무거운 준비 작업은 컨테이너 앱 전담
4. 골든 코퍼스를 데스크톱 CI + **실기기/에뮬레이터 스모크** 양쪽에서 실행
5. Hamster는 "구동 가능성" 실증일 뿐 — 자체 스택 동시 탑재 메모리 버짓은 스파이크로 실측
- 이 조건들이 못 지켜지면 해당 언어는 "sans-io 예외"가 아니라 원칙 포기임을 인정하고 재검토.

## 착수 전 스파이크 목록 (v2 신설)
1. 익스텐션 내 UniFFI: 콜드 스타트·왕복 p99·바이너리 크기 실측 (48MB 예산 기기에서)
2. librime + 자체 코어 동시 탑재 메모리 실측
3. AzooKeyKanaKanjiConverter의 Android 경로 검증 (LOUDS 리더 자체 구현 vs Swift-for-Android)
4. commit+delete 전략(한국어 iOS 안전 모드)의 주요 앱 호환성 매트릭스

## 데이터 자산 전략 (v2.1 — 사전·언어모델·랭킹)

### 원칙: 데이터 파이프라인을 1급 서브시스템으로
- 산출물이 아니라 **컴파일러를 소유한다**: `원천 코퍼스/어휘 → 정제 → 언어팩 바이너리`를
  재현 가능한 오프라인 파이프라인(dictionary compiler)으로 구축. 언어 추가 = 이 파이프라인에
  데이터 소스를 꽂는 작업이 되도록.
- 원천별 라이선스·출처·버전을 팩 메타데이터에 기록 (라이선스 지뢰 대비 감사 추적).

### 언어팩 바이너리 포맷 (공용, mmap 조회 전용) — 컨테이너·lexicon 섹션 구현됨 (taza-pack, taza-lexicon-compiler)
- 헤더(포맷 버전·언어·서명) + 섹션 테이블. 섹션 타입은 확장 예약:
  lexicon(FST/DAWG) / n-gram LM(양자화 trie) / 오타 confusion 데이터 /
  이모지 어노테이션(CLDR) / 언어별 부가 섹션(가나-한자 매핑, 병음 테이블 등)
- 스크립트 특성 메타데이터 포함: 어절 분리 유무, 클러스터 규칙, RTL 여부,
  composing 골격 종류 — 추후 언어가 코드 수정 없이 데이터로 선언되도록.
- 개인화 데이터는 팩과 분리된 별도 스토어(사용자 unigram·최근성) — 팩은 항상 읽기 전용.

### 우선 3언어 조달 계획
| 언어 | lexicon/LM 원천 후보 | 비고 |
|---|---|---|
| 영어 | SCOWL, wordfreq 계열 빈도 데이터, Google Books n-gram 등 → KenLM 학습 후 양자화 | 원천별 라이선스 개별 검증. 내장 팩이므로 크기 예산 우선 |
| 한국어 | mecab-ko-dic(Apache-2.0) 형태소 사전 + 위키·공개 말뭉치(모두의 말뭉치는 이용 조건 확인 필수) | **어절 단위 사전은 교착어 특성상 폭발** — 형태소 단위 LM + 조사·어미 결합 모델로 설계. 오타 모델은 자모 레벨 |
| 일본어 | AzooKeyKanaKanjiConverter 사전(Apache-2.0)을 기반, 부족분 보강 | 랭킹은 엔진 내장 + 아래 개인화 레이어를 코어 측에 |

### 랭킹: 오프라인 평가 하네스가 튜닝의 전제
- 평가 셋: 언어별 (입력 시퀀스 → 의도 텍스트) 쌍 코퍼스. 실사용 로그가 없는 초기에는
  공개 오타 코퍼스 + 코퍼스에서 합성한 오타(터치 기하 기반 노이즈 주입)로 구성.
- 메트릭: top-1/top-3 정확도, MRR, keystroke savings. CI 회귀 게이트로 등록 —
  랭킹 가중치 변경은 이 게이트를 통과해야 병합.
- 랭킹 함수 구조: `score = LM 확률 + 터치 공간 모델 + 편집거리 + 개인화 부스트`의
  가중 결합으로 시작(가중치는 평가 하네스로 튜닝), 학습형 랭커는 데이터 축적 후.
- 온디바이스 개인화: 사용자 unigram 빈도 + 최근성 부스트. 코어 스냅샷에 포함,
  incognito 플래그 시 학습 중단. 원격 로그 수집은 하지 않음 — 개선 신호가 필요해지면
  온디바이스 집계 요약 + 명시 동의 방식으로 별도 설계.

## 언어별 확장 기능 카탈로그와 아키텍처 훅 (v2.2)
기능 자체는 추후 구현. 지금 확보할 것은 "이 기능이 나중에 와도 계약이 안 깨진다"는 훅이다.

### 기능 카탈로그 (구현 시점 무관, 계약 영향 기준으로 정리)
| 언어권 | 기능 | 요구하는 아키텍처 훅 |
|---|---|---|
| 라틴 공통 | 글라이드 타이핑 | 포인터 스트림 배칭(기확보) + **탭 경로와 병렬인 제스처 디코더 단계**(lexicon FST 빔서치). Composer 앞단의 InputDecoder 추상화 |
| 라틴 공통 | 자동 대문자화, 더블스페이스 마침표, 어퍼스트로피 축약(l', don't) | 자동교정 규칙 레이어(기확보). 단 casing은 **로케일 인지**(터키어 ı/İ) — 케이스 변환을 코어 유틸로 통일 |
| 한국어 | 천지인·나랏글·단모음 배열 | 타이머 이벤트(기확보) + 배열=언어팩 데이터 |
| 한국어 | 한자 변환(온디맨드), 자동 띄어쓰기 제안 | **골격의 능력 플래그화**: Automaton이면서 온디맨드 Convert 능력 — 4골격을 고정 분류가 아니라 조합 가능한 capability로 재해석. 띄어쓰기 제안은 형태소 LM(기확보)이 전제 |
| 일본어 | 플릭 입력, 12키 토글 | 타이머+레이아웃 데이터(기확보) |
| 일본어 | 문자종 전환(ひら/カタ/英数), 전각/반각 | Composer 상태 + 후보 변환 규칙. 후보 바에 "같은 reading의 문자종 변형" 생성기 |
| 일본어 | 顔文字·기호 사전, 予測変換 학습 | 언어팩 부가 섹션(기확보) + 개인화 스토어(기확보) |
| 중국어(추후) | 필기 인식, 획순 입력, 창힐/주음 스킴 | **KeyboardFrame에 캔버스형 패널 타입 예약**. 인식 엔진은 후보 바로 합류하는 별도 InputDecoder |
| 중국어(추후) | 간체/번체 변환, 모호 병음(fuzzy), 클라우드 후보 | 클라우드 후보=지연 후보 갱신(기확보)+Full Access 게이트. 변환·fuzzy는 엔진 설정 |
| RTL(추후) | 아랍어·히브리어, 모음 부호 레이어(harakat/niqqud) | MoveCursor 논리적 이동(기확보) + KeyboardFrame 좌우 미러링 플래그 + 부호 레이어=레이어 전환(기확보) |
| 인도계(추후) | 음역↔네이티브 배열 토글 | 한 언어에 복수 Composer 등록 + 전환 이벤트 |
| 공통 | 다국어 동시 예측(언어 자동 감지) | **LM 점수의 언어 간 비교 가능성** — 아래 LM 추상화의 캘리브레이션 계약이 전제 |
| 공통 | 음성 입력 | 셸 서비스가 확정 텍스트를 기존 Commit Effect 채널로 주입 (iOS 서드파티 키보드는 시스템 받아쓰기 불가 — 자체 엔진 또는 미지원 명시) |
| 공통 | 사용자 단축어·사용자 사전 | 개인화 스토어(기확보)에 언어별+전역 네임스페이스 |

### 이번에 계약에 추가되는 것 (기능 없이 훅만)
1. **InputDecoder 추상화**: 탭 히트테스트 / 글라이드 / 필기 / 음성이 모두
   "후보 or 확정 텍스트를 만들어 Composer·후보 바에 합류"하는 병렬 단계.
2. **골격 → capability 조합**: composing(유/무), validation, conversion(상시/온디맨드),
   timer 를 Composer의 선언적 능력으로. 한국어 한자, 일본어 문자종 변환이 자연 수용.
3. KeyboardFrame에 패널 타입(키 그리드 외 캔버스 등)과 RTL 미러링 플래그 예약.
4. 케이스 변환·정규화는 로케일 인지 코어 유틸로 단일화.

## 언어모델 추상화 (v2.2 — 교체 가능 구조)

```rust
trait LanguageModel {
    fn score(&self, context: &[Token], candidate: &str) -> LogProb;  // 공통 스케일
    fn predict(&self, context: &[Token], limit: usize) -> Vec<Prediction>;
    fn metadata(&self) -> LmMetadata;  // 종류·버전·캘리브레이션 파라미터
}
```

- **랭킹 함수는 이 trait만 소비** — n-gram이든 신경망이든 랭킹·Composer·평가 하네스는 불변.
- 언어팩의 LM 섹션은 **타입 태그 레지스트리**: `ngram-v1`(현재), `neural-v1`(추후, 양자화
  가중치 mmap) 등. 로더가 태그로 디스패치, 미지 태그는 해당 섹션 무시 + 폴백.
- **구현 현황**: `ngram-v1`(bigram) 섹션 + `Pack::language_model()` 디스패치 지점 구현됨.
  Composer는 `Pack` 핸들만 받으므로(feed 시그니처) LM 교체는 팩 배포로 끝난다.
  다음 단어 예측이 LatinComposer의 단어 경계(separator·후보 선택·자동교정 직후)에 연결됨.
- **토크나이저도 팩에 포함** — LM 교체는 토큰화 교체를 수반하므로 (형태소/자모/BPE)
  LM 섹션과 토크나이저 섹션을 쌍으로 버저닝.
- **폴백 체인**: neural 로드 실패(메모리 예산 초과 포함) → ngram → lexicon-only.
  48MB 하한 기기에서 neural은 선택 로드.
- **캘리브레이션 계약**: score는 언어 간 비교 가능한 로그확률 스케일로 정규화
  (다국어 동시 예측의 전제). 캘리브레이션 파라미터는 팩 메타데이터에.
- 평가 하네스가 교체 게이트: 같은 평가 셋에서 LM A/B 비교 → 회귀 없을 때만 팩 배포.

### 파이프라인 적정성 재점검 (우선 3언어)
- 영어: lexicon+ngram 파이프라인이 글라이드(FST 빔서치가 같은 lexicon 사용)까지 커버 — 적정.
- 한국어: 형태소 단위 LM이 자동 띄어쓰기 제안·자모 오타 보정·한자 변환(형태소→한자 후보)의
  공통 기반 — 적정. 단 형태소 분석기 품질이 파이프라인 상류 리스크로 추가됨.
- 일본어: AzooKey 사전+엔진에 랭킹 개인화만 코어 측 — 엔진 내부 랭킹과 코어 LM 점수의
  이중 구조가 생기므로, 엔진 출력 후보에 코어 개인화 부스트를 더하는 "후처리 재랭킹"으로
  경계를 고정 (엔진 내부에는 개입하지 않음).

## 커서 이동 (v2.1)
- `InputEvent::CursorMoved { context: EditorContext }`를 코어 이벤트로 추가.
  기본 정책: 진행 중 composing은 finalize (언어별 finalize 정책 적용).
- iOS는 커서 이동 통지가 불완전(selection 변경 콜백 비신뢰) — 통지를 신뢰하지 않고
  **문맥 스냅샷 불일치 감지(reconciliation)를 커서 이동 추정의 주 신호**로 삼는다.
- 코어가 커서를 움직이는 Effect `MoveCursor(offset)` 추가 (iOS adjustTextPosition /
  Android setSelection). 스페이스바 길게 눌러 커서 이동(양 플랫폼 관습)은
  포인터 스트림 → 코어 판정 → MoveCursor 경로로 처리.
- offset 단위는 코드포인트로 하되 그래핌 경계 스냅은 코어가 산출.
  RTL(아랍어·히브리어, 추후)의 논리적/시각적 방향 구분은 Effect 의미를
  "논리적 이동"으로 고정해 미리 봉합.
- 확정 텍스트 내부로 커서 이동 시 재변환(일본어 관습)은 CommittedText 이력이
  남아 있는 범위에서만 지원 — v1 범위 제외, 계약만 예약.
- **합성 재개(adoption, 구현됨)**: 커서 이동 후에도 그 위치에서 합성·자모 분해 삭제가
  이어져야 한다. composing이 없을 때 Composer가 EditorContext의 커서 앞 글자를 분해해
  composing으로 되가져오고(delete_before_commit로 원문 치환), 이후 입력·Backspace는
  일반 합성 규칙을 따른다. 통로(feed의 context 파라미터)는 언어 공통, 무엇을 어떻게
  분해할지는 언어별 정책 — 한국어는 음절→자모(복합 모음·겹받침 포함) 분해로 도깨비불
  재개와 자모 단위 삭제를 재현하고, 분해 불가 문자는 delete_before_commit=1 passthrough.
  텔렉스 성조 소급 수정, 인도계 클러스터 재진입도 같은 통로를 쓴다.

## 특수문자·기호 (v2.1)
- 심볼 레이어는 KeyboardFrame의 레이어 전환으로 표현 (숫자/기호 1·2면, 이모지).
  레이어 구성·기호 배치는 언어팩 데이터 — 언어별 관습(일본어 「」·。、, 스페인어 ¿¡,
  프랑스어 «» 등)을 코드가 아닌 데이터로 수용.
- composing 중 특수문자 입력의 언어별 의미를 `Separator` 처리 정책으로 명시:
  한국어=음절 확정 후 기호 삽입, 일본어=현재 후보 확정 후 기호(。、는 변환 트리거 겸용),
  영어=단어 경계로 자동교정 발동.
- 프랑스어 구두점 앞 공백, 스마트 구두점(더블 스페이스→마침표), 자동 페어링은
  자동교정 엔진의 규칙 레이어 소관 — composition과 분리.
- 이모지: 검색·최근 사용은 코어 소관(CLDR 어노테이션 섹션), 삭제는 ZWJ 시퀀스
  통째 삭제 정책 테이블 적용.

## 테스트 전략
- 언어별 골든 코퍼스: 이벤트 시퀀스 → (delete_before_commit, commit, composing, candidates).
  도깨비불 백스페이스 복귀, kk→촉음, ss→원문자, 클러스터 백스페이스, 천지인 타임아웃,
  finalize 정책, 문맥 0자 케이스 포함.
- KeyboardFrame 동일성 검증 (접근성 필드 포함).
- 외부 엔진 언어는 통합 테스트 등급 분리 + 실기기 스모크.

## 리스크 (v2 갱신)
1. 문맥 비보장(0자 포함)이 예측·교정 품질의 구조적 상한 — 흡수 불가, 설계 전제
2. 일본어 Android 경로(AzooKey 사전 리더) 미실증 — 스파이크 1순위
3. 자동교정·예측 엔진이 최대 단일 컴포넌트 — 초기 데이터 부재로 v1 품질 한계
4. librime 의존성 무게(Boost/LevelDB/OpenCC)와 메모리 예산 충돌 가능성
5. 사전·코퍼스 라이선스 (mozc-OSS=IPAdic 계열 고지 의무 등, 언어별 개별 검토 필요)
