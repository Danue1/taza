# 데이터 출처·라이선스

## FrequencyWords (en_50k.txt, ko_50k.txt)

- 출처: https://github.com/hermitdave/FrequencyWords (2018, OpenSubtitles 2018 코퍼스 파생)
- 라이선스: **CC-BY-SA 4.0** — 저작자 표시 + 동일조건변경허락(share-alike)
- 용도: **개발·평가용**. 제품 출시 배포에 포함하려면 CC-BY-SA의 share-alike 조건이
  파생 데이터(언어팩)에 미치는 범위를 최종 검토해야 한다. 대안 후보: SCOWL(영어,
  관대한 커스텀 라이선스), Google Books n-gram(CC-BY 3.0), mecab-ko-dic(Apache-2.0).
- 한국어 목록은 어절 단위(조사 포함) — 형태소 단위 파이프라인(mecab-ko-dic 기반)으로
  교체 예정. 현재는 자모 인코딩 v1 검증용.

파생 산출물: `data/packs/english.tazapack`, `data/packs/korean.tazapack`
(taza-lexicon-compiler로 생성, 재현 커맨드는 아래)

```
cargo run --release -p taza-lexicon-compiler -- en data/en-words.tsv data/packs/english.tazapack
cargo run --release -p taza-lexicon-compiler -- ko data/ko-words.tsv data/packs/korean.tazapack --hangul-jamo --layout data/dubeolsik-layout.txt
```
