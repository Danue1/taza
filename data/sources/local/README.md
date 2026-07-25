# 손으로 갖다 놓는 원천

이용 신청·로그인을 거쳐야 해서 URL로 조달할 수 없는 원천이 여기 들어온다. 파일이
없으면 `taza-packs`가 그 원천만 건너뛰고 나머지로 팩을 만든다 — 받은 만큼만 반영된다.
받아 놓은 원천은 팩 고지(`data/sources/NOTICE.md`)에 자동으로 실린다.

이 디렉터리의 데이터 파일은 저장소에 넣지 않는다. 재배포 조건이 원천마다 다르고,
크기도 저장소에 담을 규모가 아니다.

## 모두의 말뭉치 (국립국어원)

<https://corpus.korean.go.kr> 회원 가입 → 이용 신청 → 승인 후 내려받기. 공공누리
제1유형이라 출처만 밝히면 상업적 이용과 2차 저작물 작성이 가능하다.

받은 zip을 이름만 맞춰 그대로 둔다 (`data/recipes/korean.sources.d/30-nikl-modu.toml`이
찾는 이름):

| 말뭉치 | 파일 이름 |
| --- | --- |
| 신문 | `NIKL_NEWSPAPER_2022.zip` |
| 구어 | `NIKL_SPOKEN_2020.zip` |
| 메신저 | `NIKL_MESSENGER_2021.zip` |

판이 다르면 조각 파일의 `version`·`file`을 그 판에 맞춰 고친다.

## 우리말샘 (국립국어원)

<https://opendict.korean.go.kr> 회원 가입 후 사전 전체를 XML로 내려받는다.
CC BY-SA 2.0 KR.

표제어만 한 줄에 하나씩 뽑아 `urimalsam-headwords.tsv`로 둔다. 붙임표(`-`)·구 경계
표시(`^`)·어깨번호는 빼고, 공백이 든 구(句) 표제어는 어절 사전에 들어갈 자리가 없으므로
버린다.

```bash
python3 - <<'PY' > data/sources/local/urimalsam-headwords.tsv
import re, glob, xml.etree.ElementTree as ElementTree
seen = set()
for path in glob.glob("<우리말샘 XML을 푼 자리>/*.xml"):
    for word in ElementTree.parse(path).iter("word"):
        headword = re.sub(r"[-^ㆍ\d]", "", (word.text or "").strip())
        if headword and " " not in headword and headword not in seen:
            seen.add(headword)
            print(headword)
PY
```

## 다른 말뭉치를 더할 때

- 문장이 `form` 필드에 담긴 JSON이면 `format = "nikl-corpus"`가 그대로 읽는다.
- 그 밖의 형식은 `낱말` 또는 `낱말<TAB>빈도` 목록으로 뽑아 `format = "word-list"`로
  들인다. `.zip`·`.gz`·평문을 모두 읽는다.
- 어느 쪽이든 `korean.sources.d/`에 조각 TOML을 하나 더하면 끝이다. 레시피 본문은
  건드리지 않는다.
