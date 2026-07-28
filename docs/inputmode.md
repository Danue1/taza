# inputmode 기준선 — 순정 키보드 실측

빌트인 UX 계승 원칙에 따라 필드 성격(inputmode)별 화면을 순정 키보드에서 실측한 기록이다.
캡처는 `docs/keyboard-baseline/{en,ko}`에 있고, 파일명이 아래 표의 갈래와 같다.

측정 환경: iOS 26.5 · iPhone 17 Pro 시뮬레이터 · Safari · 세로. 웹 `inputmode`/`type`을
Safari가 `UIKeyboardType`으로 옮긴 결과이므로 네이티브 앱의 화면과 같다.
Android는 이 기기에 SDK가 없어 **미실측**이며, 아래 매핑 열은 문헌 기준이다.

## 플랫폼 매핑

| 의미 | 웹 | iOS | Android |
|---|---|---|---|
| 일반 텍스트 | `inputmode="text"` | `default` / `asciiCapable` | `TYPE_CLASS_TEXT` |
| 이메일 | `type="email"` | `emailAddress` | `TYPE_TEXT_VARIATION_EMAIL_ADDRESS` |
| URL | `type="url"` | `URL` | `TYPE_TEXT_VARIATION_URI` |
| 검색 | `type="search"` | `webSearch` | `TYPE_TEXT_VARIATION_WEB_EDIT_TEXT` |
| 숫자 | `inputmode="numeric"` | `numberPad` | `TYPE_CLASS_NUMBER` |
| 소수 | `inputmode="decimal"` | `decimalPad` | `+ TYPE_NUMBER_FLAG_DECIMAL` |
| 전화 | `type="tel"` | `phonePad` | `TYPE_CLASS_PHONE` |
| 비밀번호 | `type="password"` | `isSecureTextEntry` | `TYPE_TEXT_VARIATION_PASSWORD` |
| 입력 안 함 | `inputmode="none"` | — | `TYPE_NULL` |
| 리턴키 문구 | `enterkeyhint` | `returnKeyType` | `imeOptions IME_ACTION_*` |
| 학습 금지 | — | `isSecureTextEntry` | `IME_FLAG_NO_PERSONALIZED_LEARNING` |

## iOS 실측 — 무엇이 바뀌는가

| 갈래 | 후보 바 | 문자 배열 | 하단 행 | 리턴키 | 부가 |
|---|---|---|---|---|---|
| text | 예측 3칸 | QWERTY, shift 활성(대문자) | `123 · 😀 · space · ⏎` | 회색 ⏎ | 지구본·마이크 |
| email | **없음** | QWERTY, shift 비활성 외곽선 | space 축소 + **`@` `.` 삽입** | 회색 ⏎ | 지구본·마이크 |
| url | **없음** | QWERTY | **space 제거** → `.` `/` `.com` | 회색 ⏎ | 지구본만(**마이크 없음**) |
| search | 예측 3칸 | text와 동일 | text와 동일 | **파란 돋보기** | 지구본·마이크 |
| numeric | **없음** | **3×4 숫자패드** | 좌하단 빈칸 · `0` · `⌫` | 없음 | 없음 |
| decimal | 없음 | 숫자패드 | 좌하단 **`.`** | 없음 | 없음 |
| tel | 없음 | 숫자패드 | 좌하단 **`+ * #`** | 없음 | 없음 |
| password | **🔑 Passwords 한 줄** | QWERTY | **이모지 키 제거**, 레이어 키가 `.?123` | 회색 ⏎ | **지구본·마이크 없음** |
| none | — | **키보드가 뜨지 않음** | — | — | — |
| enterkeyhint | 예측 3칸 | 그대로 | 그대로 | **파란 아이콘**(go=→) | 그대로 |

숫자 패드 셋(numeric·decimal·tel)은 **좌하단 키 하나만 다르다**. 숫자 밑 `ABC`/`DEF`
서브라벨은 전화 전용이 아니라 세 갈래 모두에 붙는다. `.`·`+ * #`·`⌫`는 키 배경 없이
글자만 그린다.

## 한국어(두벌식) 실측

| 관찰 | 캡처 |
|---|---|
| 두벌식 기본 — 후보 바는 구분선만 있고 비어 있다. 스페이스에 작게 `한` | `ko/01-text.png` |
| `안녕` 조합 중 — 후보가 **`"안녕"`(원문) · `안녕하` · `안녕히`**, 조합 밑줄 없음 | `ko/02-text-composing.png` |
| email 포커스 **직후 영어 키보드로 자동 전환**된다 | `ko/03-email.png` |
| 지구본으로 한글에 되돌아와도 **`@`·`.` 키는 유지** | `ko/04-email-hangul.png` |
| url도 한글 전환 가능하고 `.`·`/`·`.com` 유지 | `ko/05-url-hangul.png` |
| search는 두벌식 + 파란 돋보기 | `ko/06-search.png` |
| password는 **지구본이 사라져 한글로 바꿀 수 없다** | `ko/07-password.png` |

숫자·소수·전화·none은 문자 배열이 없어 언어와 무관하다 — `en/` 캡처가 그대로 적용된다.

## 순정이 지키는 규칙

1. **배열을 통째로 바꾸는 것은 숫자 계열뿐이다.** 나머지는 문자면을 그대로 두고 하단 행
   한두 키와 리턴키만 갈아 끼운다 — 손 위치를 다시 배우게 하지 않는다.
2. **후보 바 자리를 비우지 않는다.** 예측을 끄는 필드에서도 자리는 남기고 다른 것으로
   채운다(비밀번호 → 암호 관리자 진입).
3. **필드가 키를 걷어낸다.** url·password에서 마이크가, password에서 이모지 키와 지구본이
   사라진다. 레이어 키 라벨도 `123` → `.?123`으로 바뀐다.
4. **필드가 초기 언어를 지정한다.** 라틴이 필요한 필드는 영어로 열고, 사용자가 되돌리는
   것은 막지 않는다. 다만 비밀번호는 되돌리는 길까지 막는다.

## taza 대응 상태

필드 성격은 `FieldKind`(Text/Email/Url/Search/Number/Decimal/Phone/Password)로 들어오고,
`keyboard::field`가 그것을 배열·리턴키·후보 바 자리로 옮긴다. 셸은 플랫폼 값을 옮겨
`set_field`로 알려 주기만 한다.

| 항목 | 대응 |
|---|---|
| 숫자·소수·전화 | 3열 숫자 패드로 열고 좌하단만 갈아 끼운다(빈칸 / `.` / `+*#`) |
| 이메일 | 스페이스를 줄이고 양옆에 `@`·`.` |
| URL | 스페이스를 `.`·`/`·`.com`으로 바꾼다 |
| 검색 | 배열은 그대로, 리턴키가 "검색"이 되고 강조색으로 그려진다 |
| 비밀번호 | 이모지 키·언어 키를 걷어내고 레이어 키가 `.?123`이 된다 |
| 후보 바 | 예측을 내지 않는 필드에서는 자리 자체가 없어지고, 비밀번호에서는 남는다 |
| 초기 언어 | 이메일·URL·비밀번호는 셸이 라틴 배열로 연다(`Engine::field_prefers_latin`) |
| 리턴키 문구 | 코어는 `KeyLegend`로 동작만 밝히고(아홉 갈래) 낱말은 셸의 문자열 카탈로그가 짓는다 |

아직 다르게 두는 것:

- **비밀번호의 후보 바 자리를 비워 둔다.** 순정은 그 자리에 🔑 Passwords 한 줄을 놓는데
  암호 관리자 진입은 시스템 권한이라 서드파티가 대신할 수 없다. 자리만 남겨 키보드 높이를
  맞춘다.
- **`inputmode="none"`을 갈래로 두지 않았다.** 키보드를 띄울지 말지는 시스템이 정하므로
  서드파티 키보드가 할 일이 없다 — 웹 셸이 생기면 그때 다시 본다.
- **`enterkeyhint`와 `IME_ACTION_*`의 이름을 맞추지 않았다.** 코어의 `ReturnKey` 아홉
  갈래는 갖춰졌지만 셸이 플랫폼 값을 그리로 옮기는 표는 iOS `returnKeyType`뿐이다.

## 아직 확인하지 않은 것

- **Android 실측** — SDK·에뮬레이터 미설치. inputType 조합(`TYPE_CLASS_NUMBER` +
  `FLAG_SIGNED`, `VISIBLE_PASSWORD` 등)은 iOS보다 갈래가 많아 별도 실측이 필요하다.
- iPad·가로 폼팩터에서의 같은 표.
- `type="datetime-local"` 등 iOS가 자체 피커를 띄우는 필드.
- 서드파티 키보드에서 `.?123`·Passwords 바 같은 시스템 제공 요소를 어디까지 흉내 낼 수
  있는지(암호 관리자 진입은 시스템 권한).
