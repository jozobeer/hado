"""Corpus source enumeration and root-relative CLI diagnostics."""
from pathlib import Path
import re


def source_files(root):
    return sorted((path for path in Path(root).rglob('*')
                   if path.is_file() and path.suffix in ('.hd', '.hdi')),
                  key=lambda path: path.as_posix().encode())


def diagnostic(stderr, entry, code):
    match = re.fullmatch(r'([^\n]+):([1-9][0-9]*):([1-9][0-9]*): error\['
                         + re.escape(code) + r'\]: [^\n]+\n', stderr)
    if not match:
        return None
    relative = Path(match[1])
    if relative.is_absolute() or '..' in relative.parts or relative.as_posix() != match[1]:
        return None
    path = entry.parent / relative
    if path not in source_files(entry.parent):
        return None
    return path, int(match[2]), int(match[3])
