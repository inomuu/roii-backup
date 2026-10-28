"""Stage the licensed fire-prefab dependency closure for editor-only previews."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import tarfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('package', type=Path)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    destination = repo / 'unity/ROI/Assets/Editor/Vfx/References/MaykerStudio/FreeSlashVFX'
    prefix = 'Assets/Free Slash VFX/'
    entries = {}
    with tarfile.open(args.package, 'r:gz') as archive:
        members = {m.name.rstrip('/'): m for m in archive.getmembers()}
        for name, member in members.items():
            if not name.endswith('/pathname'):
                continue
            guid = name.split('/')[0]
            path = archive.extractfile(member).read().decode().splitlines()[0].rstrip('\0')
            if not path.startswith(prefix) or guid + '/asset' not in members:
                continue
            relative = Path(path[len(prefix):])
            if relative.is_absolute() or '..' in relative.parts:
                raise ValueError(path)
            entries[guid] = (relative, archive.extractfile(members[guid+'/asset']).read(),
                             archive.extractfile(members[guid+'/asset.meta']).read())
    root = next(g for g, e in entries.items() if str(e[0]) == 'Prefabs/Slash Fire VFX.prefab')
    selected, pending = set(), [root]
    while pending:
        guid = pending.pop()
        if guid in selected:
            continue
        selected.add(guid)
        path, data, meta = entries[guid]
        if path.parts[0] == 'Demo' or path.suffix == '.cs':
            raise ValueError('Unexpected executable/demo dependency: ' + str(path))
        # GUID references occur in Unity YAML and embedded Shader Graph JSON strings.
        text = (data + meta).decode('utf-8', errors='ignore')
        pending.extend(g for g in re.findall(r'\b[0-9a-f]{32}\b', text) if g in entries and g not in selected)
    manifest = []
    for guid in sorted(selected):
        path, data, meta = entries[guid]
        target = destination / path
        for file, content in ((target, data), (Path(str(target)+'.meta'), meta)):
            if file.exists() and file.read_bytes() != content:
                raise ValueError('Refusing to overwrite changed reference: ' + str(file))
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_bytes(content)
        manifest.append({'path': str(path), 'guid': guid, 'sha256': hashlib.sha256(data).hexdigest()})
    (destination / 'source-manifest.json').write_text(json.dumps(manifest, indent=2)+'\n')
    print(json.dumps({'destination': str(destination), 'assets': len(manifest), 'files': [x['path'] for x in manifest]}, indent=2))


if __name__ == '__main__':
    main()
