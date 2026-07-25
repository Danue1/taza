# 데이터 출처·라이선스

이 문서는 `taza-packs`가 `data/recipes/*.toml`에서 생성한다 — 손으로 고치지 않는다.

## english (en) — 판 1

표제어 80000개

### 원천

- SCOWL 2020.12.07 (SCOWL permissive (Kevin Atkinson))
- Tatoeba sentences (eng) 2026-07-25 (CC-BY 2.0 FR)

### 저작자 표시

> Word lists from SCOWL 2020.12.07, Copyright 2000-2018 Kevin Atkinson. Used and redistributed under the SCOWL license.
> Word frequencies derived from the Tatoeba Project English sentence export (https://tatoeba.org), licensed CC-BY 2.0 FR.

## korean (ko) — 판 1

표제어 200000개

### 원천

- mecab-ko-dic 2.1.1-20180720 (Apache-2.0)
- Tatoeba sentences (kor) 2026-07-25 (CC-BY 2.0 FR)
- Korean Wikipedia 20260701 (CC BY-SA 4.0)

### 저작자 표시

> Korean morpheme dictionary from mecab-ko-dic 2.1.1-20180720 (https://bitbucket.org/eunjeon/mecab-ko-dic), licensed under Apache License 2.0.
> Word frequencies derived from the Tatoeba Project Korean sentence export (https://tatoeba.org), licensed CC-BY 2.0 FR.
> Word frequencies derived from the Korean Wikipedia article dump of 2026-07-01 (https://ko.wikipedia.org), licensed CC BY-SA 4.0.

## 재현

```
cargo run --release -p taza-toolchain --bin taza-packs
```
