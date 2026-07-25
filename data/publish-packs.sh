#!/bin/bash
# 언어팩을 GitHub Release로 올린다. `taza-packs`를 먼저 돌려 산출물을 만들어 두어야 한다.
#
# 앱은 카탈로그 URL 하나만 알고, 아카이브는 **카탈로그 기준 상대 경로**로 찾는다
# (PackInstaller). 그래서 catalog.json과 *.tazapack.zst는 반드시 같은 릴리스에 나란히
# 올라가야 한다 — 나뉘면 앱이 아카이브를 찾지 못한다.
#
# 태그를 고정해 두고 자산을 덮어쓴다. `releases/latest`를 쓰지 않는 이유는 그것이
# 저장소 전체의 최신 릴리스를 가리키기 때문이다 — 나중에 앱 릴리스가 생기면 언어팩이
# 가려진다. 고정 태그는 앱 릴리스와 무관하게 언어팩만 갱신한다.
#
# 되돌릴 자리가 필요하면 판올림마다 불변 태그(packs-v2 등)를 따로 하나 더 만들어 두고,
# 이 고정 태그는 그중 하나를 가리키는 현재 판으로 쓴다.
set -euo pipefail

tag="${TAZA_PACK_TAG:-packs}"
script_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
packs="${script_directory}/packs"
notice="${script_directory}/sources/NOTICE.md"

if [[ ! -f "${packs}/catalog.json" ]]; then
    echo "카탈로그가 없다: ${packs}/catalog.json — 먼저 taza-packs를 돌린다" >&2
    exit 1
fi

assets=("${packs}/catalog.json")
while IFS= read -r archive; do
    assets+=("${archive}")
done < <(find "${packs}" -name '*.tazapack.zst' | sort)

if [[ ${#assets[@]} -lt 2 ]]; then
    echo "올릴 아카이브가 없다 (${packs}/*.tazapack.zst)" >&2
    exit 1
fi

# 원천 라이선스가 저작자 표시를 요구하므로 고지를 릴리스 본문에 함께 싣는다
if gh release view "${tag}" >/dev/null 2>&1; then
    gh release upload "${tag}" "${assets[@]}" --clobber
    gh release edit "${tag}" --notes-file "${notice}"
else
    gh release create "${tag}" "${assets[@]}" \
        --title "언어팩" \
        --notes-file "${notice}"
fi

repository="$(gh repo view --json nameWithOwner --jq .nameWithOwner)"
echo
echo "올림 완료. 앱이 볼 카탈로그 URL:"
echo "  https://github.com/${repository}/releases/download/${tag}/catalog.json"
