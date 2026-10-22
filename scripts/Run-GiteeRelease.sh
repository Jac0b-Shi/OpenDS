#!/usr/bin/env bash
set +x
set -euo pipefail

cd "${WORKSPACE:?Gitee Go WORKSPACE is not set}"
if [[ ! -f scripts/mirror_gitee_release.py ]]; then
    echo 'OpenDS source checkout is missing from WORKSPACE.' >&2
    exit 1
fi
if [[ -z "${OPENDS_GITEE_TOKEN:-}" ]]; then
    echo 'OPENDS_GITEE_TOKEN is unavailable. In the Gitee pipeline editor, add a variable, select Common Variables, and associate the encrypted variable.' >&2
    exit 1
fi
for tool in git python3; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "Required runner tool is missing: $tool" >&2
        exit 1
    fi
done
if ! command -v gpg >/dev/null 2>&1 || ! command -v gpgconf >/dev/null 2>&1; then
    if ! command -v apt-get >/dev/null 2>&1; then
        echo 'GPG is missing and this runner has no apt-get. Use an Ubuntu runner with GnuPG installed.' >&2
        exit 1
    fi
    root_command=()
    if [[ "$(id -u)" != 0 ]]; then
        if ! command -v sudo >/dev/null 2>&1 || ! sudo -n true; then
            echo 'Installing GnuPG requires root or noninteractive sudo on this runner.' >&2
            exit 1
        fi
        root_command=(sudo -n)
    fi
    echo 'Installing GnuPG from the runner Ubuntu package repositories.'
    "${root_command[@]}" env DEBIAN_FRONTEND=noninteractive apt-get update -qq
    "${root_command[@]}" env DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends gnupg ca-certificates
fi
for tool in gpg gpgconf; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "Required runner tool is still missing: $tool" >&2
        exit 1
    fi
done
exec python3 scripts/mirror_gitee_release.py "$@"
