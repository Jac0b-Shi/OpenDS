#!/usr/bin/env bash
set +x
set -euo pipefail

cd -- "${BASH_SOURCE[0]%/*}/.."
for tool in curl git timeout sha256sum sed sort diff mktemp wc mkdir cp rm; do
    command -v "$tool" >/dev/null 2>&1 || { echo "Required runner tool is missing: $tool" >&2; exit 1; }
done

tag="${OPENDS_RELEASE_TAG:-}"
if [[ -z "$tag" ]]; then
    tag="${GITEE_BRANCH:-}"
    if [[ "$tag" != v* ]]; then
        tag="v$(sed -n '/^\[package\]$/,/^version = /s/^version = "\([^"]*\)".*/\1/p' Cargo.toml)"
    fi
fi
if [[ ! "$tag" =~ ^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]]; then
    echo 'Select an existing stable version tag, for example v0.2.0.' >&2
    exit 1
fi
version="${tag#v}"

# Fail before downloading archives if the mirrored signed tag is not available.
remote_tag="$(timeout 15 git ls-remote --exit-code origin "refs/tags/$tag" 2>/dev/null)" || {
    echo "Gitee tag $tag is unavailable. Wait for repository synchronization." >&2; exit 1;
}
remote_tag="${remote_tag%%[[:space:]]*}"
if [[ ! "$remote_tag" =~ ^[0-9a-f]{40}$ ]]; then
    echo 'Unexpected mirrored tag identity.' >&2; exit 1
fi

work="$(mktemp -d)"
trap 'rm -rf -- "$work"' EXIT
files=("OpenDS-$version-windows-x64.zip" "OpenDS-$version-source.zip" TAG-SIGNATURE.json SHA256SUMS.txt)
base="https://github.com/Jac0b-Shi/OpenDS/releases/download/$tag"
pids=()
for name in "${files[@]}"; do
    curl --fail --silent --show-error --location --proto '=https' --proto-redir '=https' \
        --connect-timeout 5 --max-time 30 "$base/$name" --output "$work/$name" &
    pids+=("$!")
done
failed=0
for pid in "${pids[@]}"; do
    wait "$pid" || failed=1
done
if (( failed )); then
    echo "Download failed for $tag. Confirm GitHub has published it and this runner can reach GitHub; no automatic retry." >&2
    exit 1
fi

# Accept exactly the three expected manifest entries; never follow arbitrary paths.
printf '%s\n' "OpenDS-$version-source.zip" "OpenDS-$version-windows-x64.zip" TAG-SIGNATURE.json | sort > "$work/expected"
sed -n 's/^[0-9a-f]\{64\}  \(\.\/\)\{0,1\}//p' "$work/SHA256SUMS.txt" | sort > "$work/listed"
if [[ "$(wc -l < "$work/SHA256SUMS.txt")" -ne 3 ]] || ! diff -q "$work/expected" "$work/listed" >/dev/null; then
    echo 'Unexpected GitHub checksum manifest.' >&2; exit 1
fi
(cd "$work" && sha256sum --check --strict SHA256SUMS.txt)
receipt_tag="$(sed -n 's/^  "tag": "\([^"]*\)",\{0,1\}$/\1/p' "$work/TAG-SIGNATURE.json")"
receipt_object="$(sed -n 's/^  "tag_object": "\([0-9a-f]*\)",\{0,1\}$/\1/p' "$work/TAG-SIGNATURE.json")"
if [[ "$receipt_tag" != "$tag" || "$receipt_object" != "$remote_tag" ]]; then
    echo 'Published GitHub receipt differs from the mirrored Gitee tag.' >&2; exit 1
fi

mkdir -p release-assets
for name in "${files[@]}"; do
    cp -- "$work/$name" "release-assets/$name"
done
printf 'OPENDS_RELEASE_TAG=%s\nOPENDS_RELEASE_VERSION=%s\n' "$tag" "$version" >> GITEE_PARAMS
echo "Prepared four verified attachments for $tag; Gitee's native Release step handles publication."
