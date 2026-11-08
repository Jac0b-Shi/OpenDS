#!/usr/bin/env bash
# Build only the experimental bundle. Never invokes the stable installer/build.
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
project=$(cd -- "$root/../.." && pwd)
if [[ ! -f "$root/backend/opends" ]]; then
    for required in opends libVIIPER.so opends.png setup-ubuntu.sh ui/graphics.py; do
        if [[ ! -f "$project/target/release/$required" ]]; then
            "$project/scripts/build-linux.sh"
            break
        fi
    done
    /usr/bin/python3 - "$project" "$root/backend" <<'PYBACKEND'
from pathlib import Path
import shutil,sys
project,destination=map(Path,sys.argv[1:])
destination.mkdir(parents=True,exist_ok=True)
for name in ['opends','libVIIPER.so','opends.png','setup-ubuntu.sh','ui/graphics.py']:
    target=destination/name;target.parent.mkdir(parents=True,exist_ok=True)
    shutil.copy2(project/'target/release'/name,target)
for name in ['LICENSE','THIRD-PARTY.md']:
    shutil.copy2(project/name,destination/name)
shutil.copy2(project/'docs/guides/ubuntu.md',destination/'ubuntu.md')
PYBACKEND
fi
if [[ ! -d "$root/vendor/python/OpenGL" ]]; then
    python3 -m pip download --require-hashes --no-deps -r "$root/requirements.txt" --dest "$root/vendor/wheels"
    /usr/bin/python3 - "$root" <<'PY'
from pathlib import Path
import sys,zipfile
root=Path(sys.argv[1])
with zipfile.ZipFile(next((root/'vendor/wheels').glob('*.whl'))) as archive:
    archive.extractall(root/'vendor/python')
PY
fi
/usr/bin/python3 - "$root" <<'PYMODEL'
import hashlib,json,sys
from pathlib import Path
root=Path(sys.argv[1]);source=json.loads((root/'assets/SOURCE.json').read_text())
assert hashlib.sha256((root/'assets/dualsense.glb').read_bytes()).hexdigest()==source['sha256'], '3D asset checksum mismatch'
PYMODEL
/usr/bin/python3 -c 'import gi,numpy; gi.require_version("Gtk","3.0"); from gi.repository import Gtk'
printf 'Prepared development bundle. Run %s/launch.sh\n' "$root"
