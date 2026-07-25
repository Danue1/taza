# 데이터 출처·라이선스

이 문서는 `taza-packs`가 `data/recipes/*.toml`에서 생성한다 — 손으로 고치지 않는다.

## english (en) — 판 2

표제어 80000개

### 원천

- SCOWL 2020.12.07 (SCOWL permissive (Kevin Atkinson))
- Tatoeba sentences (eng) 2026-07-25 (CC-BY 2.0 FR)
- CLDR emoji annotations (en) 48.2 (Unicode-3.0)

### 저작자 표시

> Word lists from SCOWL 2020.12.07, Copyright 2000-2018 Kevin Atkinson. Used and redistributed under the SCOWL license.
> Word frequencies derived from the Tatoeba Project English sentence export (https://tatoeba.org), licensed CC-BY 2.0 FR.
> Emoji annotations from Unicode CLDR 48.2 (https://cldr.unicode.org), Copyright © 1991-2025 Unicode, Inc., licensed under the Unicode License v3.

## korean (ko) — 판 2

표제어 120000개

### 원천

- mecab-ko-dic 2.1.1-20180720 (Apache-2.0)
- Tatoeba sentences (kor) 2026-07-25 (CC-BY 2.0 FR)
- Korean Wikipedia 20260701 (CC BY-SA 4.0)
- 우리말샘 20260702 (CC BY-SA 2.0 KR)
- CLDR 이모지 주석 (ko) 48.2 (Unicode-3.0)
- 타자 얼굴 문자 목록 (ko) 1 (MIT)

### 저작자 표시

> Korean morpheme dictionary from mecab-ko-dic 2.1.1-20180720 (https://bitbucket.org/eunjeon/mecab-ko-dic), licensed under Apache License 2.0.
> Word frequencies derived from the Tatoeba Project Korean sentence export (https://tatoeba.org), licensed CC-BY 2.0 FR.
> Word frequencies derived from the Korean Wikipedia article dump of 2026-07-01 (https://ko.wikipedia.org), licensed CC BY-SA 4.0.
> Headwords from 우리말샘 (https://opendict.korean.go.kr), 국립국어원, licensed CC BY-SA 2.0 KR.
> Emoji annotations from Unicode CLDR 48.2 (https://cldr.unicode.org), Copyright © 1991-2025 Unicode, Inc., licensed under the Unicode License v3.
> Emoticon annotations curated for taza.

## 재현

```
cargo run --release -p taza-toolchain --bin taza-packs
```
