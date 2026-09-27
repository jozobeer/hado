#!/usr/bin/env bash
# tier: full
set -uo pipefail
cd "$(dirname "$0")/.."

passed=0
total=0
check() {
    local label="$1"
    shift
    total=$((total + 1))
    if "$@"; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label"
    fi
}
workdir=$(mktemp -d)
trap 'rm -rf "$workdir"' EXIT
cargo build --offline -q --bin hado || exit 1

# 生成する探針の共通の道具。build と実行と比較を 1 箇所に閉じ、
# 比較が差を検出できることを control で先に確かめる（`docs/rounds/r43.md`）。
cat >"$workdir/harness.py" <<'PY'
import subprocess
from pathlib import Path

def assert_printed(actual, expected):
    assert actual == expected, (actual, expected)

def control():
    try:
        assert_printed(b'a\n', b'b\n')
    except AssertionError:
        return
    raise AssertionError('comparison control escaped')

def build_printed(root, name, source, expected):
    directory = Path(root) / name
    directory.mkdir(parents=True, exist_ok=False)
    (directory / 'main.hd').write_text(source)
    build = subprocess.run(
        ['cargo', 'run', '--offline', '-q', '--', 'build', str(directory / 'main.hd')],
        capture_output=True)
    assert build.returncode == 0 and build.stderr == b'', (name, build.returncode, build.stderr)
    result = subprocess.run([str(directory / 'main')], capture_output=True, cwd=directory)
    assert result.returncode == 0, (name, result.returncode, result.stderr)
    assert result.stderr == b'', (name, result.stderr)
    assert_printed(result.stdout, expected.encode())
    return directory
PY

# --- 43-b: log の正常系 ---
r43_log_success() {
    python3 - "$workdir" <<'PY'
import json
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
from harness import build_printed

texts = ['', 'a', 'hello world', 'line1\nline2', 'tab\there', 'quote"inside',
         'back\\slash', '日本語', 'snow ☃', 'x' * 1000]
for index, text in enumerate(texts):
    literal = json.dumps(text, ensure_ascii=False)
    source = f'def main =\n  log {literal} ->\n  0\n'
    build_printed(sys.argv[1], f'log-ok-{index}', source, text + '\n')
print(f'log success: {len(texts)} cases')
PY
}

# --- 43-c: log の失敗。出力先が壊れているので、捕捉の証拠はファイルで取る ---
r43_log_failure_program() {
    local dir="$workdir/log-fail"
    mkdir -p "$dir"
    printf 'def main = log "x"\n' >"$dir/main.hd"
    cargo run --offline -q -- build "$dir/main.hd" >"$dir/build.out" 2>"$dir/build.err" || { cat "$dir/build.err"; return 1; }
    [ "$(wc -c <"$dir/build.err")" -eq 0 ] || return 1
    printf 'IoFailure\n' >"$dir/expect"
}

r43_log_closed_stdout() {
    r43_log_failure_program || return 1
    local dir="$workdir/log-fail" status=0
    "$dir/main" >&- 2>"$dir/err" || status=$?
    [ "$status" -eq 1 ] && [ "$(wc -c <"$dir/out" 2>/dev/null || echo 0)" -eq 0 ] && cmp -s "$dir/err" "$dir/expect"
}

r43_log_full_device() {
    r43_log_failure_program || return 1
    local dir="$workdir/log-fail" status=0
    [ -w /dev/full ] || return 1
    "$dir/main" >/dev/full 2>"$dir/err" || status=$?
    [ "$status" -eq 1 ] && cmp -s "$dir/err" "$dir/expect"
}

r43_log_epipe() {
    r43_log_failure_program || return 1
    local dir="$workdir/log-fail" helper status=0
    helper=$(readlink -f scripts/exec-stdout-epipe.py)
    python3 "$helper" "$dir/main" 2>"$dir/err" || status=$?
    [ "$status" -eq 1 ] && [ "$status" -ne 141 ] && cmp -s "$dir/err" "$dir/expect"
}

r43_log_caught() {
    local dir="$workdir/log-caught"
    mkdir -p "$dir"
    cat >"$dir/main.hd" <<'HD'
handler Error as ToUnit
abort raise e =>
  write_file "observed" (Bytes.of_string e.message) ->
  ()
return value => value
def main =
  with ToUnit ->
  log "x"
HD
    cargo run --offline -q -- build "$dir/main.hd" >"$dir/build.out" 2>"$dir/build.err" || { cat "$dir/build.err"; return 1; }
    [ "$(wc -c <"$dir/build.err")" -eq 0 ] || return 1
    local status=0
    ( cd "$dir" && ./main >/dev/full 2>"$dir/err" ) || status=$?
    [ "$status" -eq 1 ] && [ "$(cat "$dir/observed")" = "IoFailure" ]
}

# --- 43-d: String.trim と String.join ---
r43_string_ops() {
    python3 - "$workdir" <<'PY'
import json
import sys

sys.path.insert(0, sys.argv[1])
from harness import build_printed

def string_expression(text):
    """Write a String as a literal, or through Bytes when it holds a control
    byte: §5 has escapes only for \\n and \\t, so CR, VT and FF cannot be
    written down directly."""
    if all(character in '\n\t' or ord(character) >= 0x20 for character in text):
        return json.dumps(text, ensure_ascii=False)
    values = ', '.join(str(byte) for byte in text.encode())
    return f'(Bytes.to_string (Bytes.of_list [{values}]))'

trims = ['', ' ', '  abc  ', '\t\n\v\f\r abc \r\n\t', 'a b', '　abc　', '日本 語',
         ' 日本語 ', 'a', '\r\n', '　', 'x\ty\nz']
for index, text in enumerate(trims):
    source = f'def main = String.trim {string_expression(text)}\n'
    expected = text.strip(' \t\n\v\f\r') + '\n'
    build_printed(sys.argv[1], f'trim-{index}', source, expected)

joins = [([], '-'), ([''], '-'), (['a'], '-'), (['a', 'b'], '-'), (['a', '', 'b'], '-'),
         (['a', 'b'], ''), (['-', '-'], '-'), (['日本語', 'b'], '・'), (['a', 'b', 'c'], '--'),
         (['\r\n', 'x'], '\t')]
for index, (values, separator) in enumerate(joins):
    items = ', '.join(string_expression(value) for value in values)
    source = f'def main = String.join [{items}] {string_expression(separator)}\n'
    expected = separator.join(values) + '\n'
    build_printed(sys.argv[1], f'join-{index}', source, expected)
print(f'string ops: {len(trims)} trim, {len(joins)} join cases')
PY
}

# --- 43-e: Int.abs / min / max。i64 の最小値は literal に書けない（E112）ので Int.parse で作る ---
r43_int_ops() {
    python3 - "$workdir" <<'PY'
import sys

sys.path.insert(0, sys.argv[1])
from harness import build_printed

LOW, HIGH = -(2 ** 63), 2 ** 63 - 1

def wrap(value):
    return ((value + 2 ** 63) % 2 ** 64) - 2 ** 63

def expression(value):
    if value == LOW:
        return '(Int.parse "-9223372036854775808")'
    return f'({value})'

values = [LOW, LOW + 1, -1, 0, 1, HIGH - 1, HIGH, -123456789, 987654321]
for index, value in enumerate(values):
    source = f'def main = Int.to_string (Int.abs {expression(value)})\n'
    expected = str(value if value >= 0 else wrap(-value)) + '\n'
    build_printed(sys.argv[1], f'abs-{index}', source, expected)

pairs = [(LOW, 0), (LOW, HIGH), (-1, 1), (0, 0), (HIGH, HIGH), (7, -3), (LOW, LOW)]
for index, (left, right) in enumerate(pairs):
    for name, oracle in (('min', min), ('max', max)):
        source = f'def main = Int.to_string (Int.{name} {expression(left)} {expression(right)})\n'
        build_printed(sys.argv[1], f'{name}-{index}', source, str(oracle(left, right)) + '\n')
print(f'int ops: {len(values)} abs, {len(pairs) * 2} min/max cases')
PY
}

# --- 43-f: List.head / nth / take / drop。空のリストは List.filter で型を決めて作る ---
r43_list_ops() {
    python3 - "$workdir" <<'PY'
import json
import sys

sys.path.insert(0, sys.argv[1])
from harness import build_printed

def canon(value):
    if isinstance(value, str):
        return json.dumps(value, ensure_ascii=False)
    return str(value)

def canon_list(values):
    return '[' + ', '.join(canon(value) for value in values) + ']'

def canon_option(value):
    return 'None' if value is None else 'Some(' + canon(value) + ')'

int_lists = [[], [1], [1, 2], [1, 2, 3], [-1, 0, 7]]
string_lists = [[], ['a'], ['a', 'bb'], ['', 'x']]

def source_list(values):
    if values and isinstance(values[0], str):
        empty = 'List.filter [""] fn(value => String.length value > 0)'
    else:
        empty = 'List.filter [0] fn(value => value > 0)'
    return empty if not values else '[' + ', '.join(canon(value) for value in values) + ']'

index = 0
for values in int_lists + string_lists:
    expression = source_list(values)
    source = f'def main = List.head ({expression})\n'
    build_printed(sys.argv[1], f'head-{index}', source, canon_option(values[0] if values else None) + '\n')
    for position in sorted({-1, 0, len(values) - 1, len(values), len(values) + 1, 5}):
        expected = values[position] if 0 <= position < len(values) else None
        source = f'def main = List.nth ({expression}) ({position})\n'
        build_printed(sys.argv[1], f'nth-{index}-{position}', source, canon_option(expected) + '\n')
    for count in sorted({-1, 0, 1, len(values) - 1, len(values), len(values) + 1, 9}):
        taken = values[:max(0, count)]
        dropped = values[max(0, count):]
        source = f'def main = List.take ({expression}) ({count})\n'
        build_printed(sys.argv[1], f'take-{index}-{count}', source, canon_list(taken) + '\n')
        source = f'def main = List.drop ({expression}) ({count})\n'
        build_printed(sys.argv[1], f'drop-{index}-{count}', source, canon_list(dropped) + '\n')
    index += 1
print(f'list ops: {index} list shapes over head, nth, take, drop')
PY
}

# --- 43-g: 受理コーパスと lock ---
r43_corpus() {
    python3 - "$workdir" <<'PY'
from pathlib import Path
import hashlib, re, shutil, subprocess, sys

root = Path(sys.argv[1]) / 'corpus'
root.mkdir(parents=True, exist_ok=False)
lock = {}
for line in Path('tests/corpus.sha256').read_text().splitlines():
    digest, path = line.split('  ', 1)
    lock[path] = digest

# 判定器はファイル名を持たない。新しい prelude の member を使う受理
# コーパスを本文から選び、札と期待出力と lock をその場で見る。
patterns = ['String.join', 'String.trim', 'Int.abs', 'Int.min', 'Int.max',
            'List.head', 'List.nth', 'List.take', 'List.drop', r'\blog\b']
# 対象は R43 の prelude の 2 項目。閉じの印を測る受理コーパスも `log` を使うので、
# member の語だけで選ぶと項目が混ざる。項目の札で絞ってから本文を見る。
wanted = {'Env と Io', 'String 操作'}
selected = {path: path.read_text() for path in sorted(Path('tests/accept').glob('*/main.hd'))
            if any(re.search(pattern, path.read_text()) for pattern in patterns)
            and any(f'// item: {item}' in path.read_text().splitlines() for item in wanted)}
assert selected, 'no accept case uses the new prelude members'
items = set()
for path, source in selected.items():
    lines = source.splitlines()
    assert '// spec: §55' in lines, path
    assert '// needs: prelude' in lines, path
    expects = [(line.startswith('// expect-error:'), line.split(':', 1)[1].strip())
               for line in lines if line.startswith(('// expect:', '// expect-error:'))]
    assert len(expects) == 1, (path, expects)
    is_error, expected = expects[0]
    assert not is_error, path
    assert lock.get(str(path)) == hashlib.sha256(source.encode()).hexdigest(), f'lock mismatch: {path}'
    for line in lines:
        if line.startswith('// item: '):
            items.add(line[len('// item: '):])
    directory = root / path.parent.name
    shutil.copytree(path.parent, directory)
    copied = directory / path.name
    build = subprocess.run(['cargo', 'run', '--offline', '-q', '--', 'build', str(copied)], capture_output=True)
    assert build.returncode == 0 and build.stderr == b'', (path, build.stderr)
    result = subprocess.run([str(copied.with_suffix(''))], capture_output=True, stdin=subprocess.DEVNULL)
    assert result.returncode == 0 and result.stderr == b'', (path, result.returncode, result.stderr)
    assert result.stdout == (expected + '\n').encode(), (path, result.stdout, expected)
assert items == wanted, items
print(f'corpus: {len(selected)} accept cases derived by content, headers, lock and stdout verified')
PY
}

# --- 43-h: Io の 5 つ目の操作の handler と、r37 の門番の追随 ---
r43_io_handler() {
    python3 - "$workdir" <<'PY'
from pathlib import Path
import subprocess
import sys

root = Path(sys.argv[1]) / 'io-handler'
root.mkdir(parents=True, exist_ok=False)
arms = ['once read_file _ => Bytes.of_string "virtual"',
        'once write_file _ _ => ()',
        'once log _ => ()',
        'once log_error _ => ()',
        'once read_dir _ => ["virtual"]',
        'once read_stdin => Bytes.of_string "virtual"']
full = 'handler Io as MyIo\n' + '\n'.join(arms) + '\ndef main = with MyIo -> log "virtual" -> 0\n'
missing = 'handler Io as MyIo\n' + '\n'.join(arm for arm in arms if arm != 'once log _ => ()') + '\ndef main = 0\n'

def check(name, source, expected_code):
    directory = root / name
    directory.mkdir()
    (directory / 'main.hd').write_text(source)
    result = subprocess.run(['cargo', 'run', '--offline', '-q', '--', 'check', str(directory / 'main.hd')], capture_output=True)
    if expected_code is None:
        assert result.returncode == 0 and result.stderr == b'', (name, result.returncode, result.stderr)
        return None
    assert result.returncode == 1 and result.stdout == b'', (name, result.returncode, result.stdout)
    assert f'error[{expected_code}]'.encode() in result.stderr, (name, result.stderr)
    return result

check('full', full, None)
check('missing-log', missing, 'E605')
r37 = Path('scripts/verify-r37.sh').read_text()
assert "'once log _ => ()'" in r37, 'verify-r37.sh does not model the log operation'
assert "'once log_error _ => ()'" in r37, 'verify-r37.sh does not model the log_error operation'
world = Path('crates/hado/tests/r37_world.rs').read_text()
assert '"log"' in world, 'r37_world.rs does not check the log default'
print('io handler: 6 arms accepted, missing log is E605, r37 gates follow')
PY
}

check '43-b log success' r43_log_success
check '43-c log closed stdout is IoFailure' r43_log_closed_stdout
check '43-c log /dev/full is IoFailure' r43_log_full_device
check '43-c log epipe is 1 and not 141' r43_log_epipe
check '43-c log failure is caught as an Error value' r43_log_caught
check '43-d String.trim and String.join' r43_string_ops
check '43-e Int.abs min max' r43_int_ops
check '43-f List head nth take drop' r43_list_ops
check '43-g accept corpus' r43_corpus
check '43-h Io handler and r37 follow-up' r43_io_handler

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
