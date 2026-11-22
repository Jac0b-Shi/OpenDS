"""Shell-color previews and separately stored viewer preferences."""
import json
import os
from pathlib import Path
import tempfile

# RGB previews, not manufacturer-calibrated finishes or special-edition artwork.
COLORS = {
    'white': ('White', None, None),
    'midnight-black': ('Midnight Black', '#25272d', None),
    'cosmic-red': ('Cosmic Red', '#9b284b', None),
    'nova-pink': ('Nova Pink', '#ec4288', None),
    'starlight-blue': ('Starlight Blue', '#53b7e5', None),
    'galactic-purple': ('Galactic Purple', '#6350ad', None),
    'volcanic-red': ('Volcanic Red', '#a82e35', .42),
    'cobalt-blue': ('Cobalt Blue', '#234eb9', .42),
    'sterling-silver': ('Sterling Silver', '#aeb2ba', .64),
    'grey-camouflage': ('Grey Camouflage', '#787e86', None),
}
DEFAULTS = {'color': 'white', 'transparent': True}


def camouflage_surface(part):
    """Edition-specific regions: printed front, plain rear, dark controls."""
    if not part:return None
    name,control=part['name'],part['control']
    if name.startswith('back-shell__'):return 'rear'
    if name.startswith(('white-shell-left__','white-shell-right__','touchpad__')):return 'pattern'
    if control in ('triangle','square','cross','circle'):
        return 'decal' if 'decal' in name else 'face-cap'
    if control in ('up','right','down','left'):
        return 'decal' if 'decal' in name else 'dpad-cap' if '__cap__' in name else 'plastic'
    if part['material']==4 or control in ('create','options','left-stick','right-stick','ps'):return 'plastic'
    return None


def linear_rgb(color):
    values = [int(color[i:i+2], 16)/255 for i in (1, 3, 5)]
    return [v/12.92 if v <= .04045 else ((v+.055)/1.055)**2.4 for v in values]


def load(path):
    result = dict(DEFAULTS)
    try:
        data = json.loads(Path(path).read_text(encoding='utf-8'))
        if isinstance(data, dict):
            if isinstance(data.get('color'), str) and data['color'] in COLORS:result['color'] = data['color']
            if type(data.get('transparent')) is bool:result['transparent'] = data['transparent']
    except (OSError, ValueError):
        pass
    return result


def save(path, settings):
    path = Path(path);path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix='.'+path.name, dir=path.parent)
    try:
        with os.fdopen(fd, 'w', encoding='utf-8') as output:
            json.dump(settings, output, indent=2);output.write('\n')
        os.replace(temporary, path)
    finally:
        Path(temporary).unlink(missing_ok=True)
