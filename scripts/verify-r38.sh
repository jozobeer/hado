#!/usr/bin/env bash
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

partial_run() {
    local started=$SECONDS name
    bash scripts/run-gates.sh "$workdir/partial" verify-r11 verify-r12 || return 1
    echo "partial run: $((SECONDS - started))s"
    python3 - "$workdir/partial" <<'PY'
from pathlib import Path
import sys
logs = list(Path(sys.argv[1]).iterdir())
assert sorted(p.name for p in logs) == ['verify-r11.log', 'verify-r12.log']
assert all(p.is_file() for p in logs)
PY
    [ "$?" -eq 0 ] || return 1
    for name in verify-r11 verify-r12; do
        grep -qx 'PASSED: 1/1' "$workdir/partial/$name.log" || return 1
        echo "$name.log: $(tail -1 "$workdir/partial/$name.log")"
    done
}
check '38-a selected real gates and flat logs' partial_run

check '38-a runner selection, serial execution and failures' python3 - "$workdir" <<'PY'
from pathlib import Path
import shutil, subprocess, sys
base = Path(sys.argv[1]) / 'runner'
(base / 'scripts').mkdir(parents=True)
shutil.copy('scripts/run-gates.sh', base / 'scripts/run-gates.sh')
for name, status in [('a', 1), ('b', 0), ('c', 0)]:
    (base / f'scripts/verify-{name}.sh').write_text(
        f'mkdir busy || exit 9\necho {name} >>trace\nsleep 0.02\nrmdir busy\n'
        f'echo stdout-{name}\necho stderr-{name} >&2\nexit {status}\n')
def run(output, selectors):
    return subprocess.run(['bash', str(base / 'scripts/run-gates.sh'), str(base / output), *selectors],
                          cwd=base.parent, capture_output=True)
for output, selectors, names, trace in [
    ('selected logs', ['verify-a.sh', 'scripts/verify-b.sh'], ['a', 'b'], 'a\nb\n'),
    ('all logs', [], ['a', 'b', 'c'], 'a\nb\na\nb\nc\n')]:
    result = run(output, selectors)
    assert result.returncode == 1 and not result.stderr, result
    assert sorted(p.name for p in (base / output).iterdir()) == [f'verify-{name}.log' for name in names]
    for name in names:
        assert (base / output / f'verify-{name}.log').read_text() == f'stdout-{name}\nstderr-{name}\n'
    assert (base / 'trace').read_text() == trace
result = run('unknown', ['verify-b', 'verify-missing'])
assert result.returncode == 1 and b'unknown gate' in result.stderr
assert not list((base / 'unknown').iterdir())
result = subprocess.run(['bash', str(base / 'scripts/run-gates.sh')], capture_output=True)
assert result.returncode == 1 and not result.stdout and b'usage:' in result.stderr
PY

seed=${SEED:-$(( (RANDOM << 15) | RANDOM ))}
echo "SEED=$seed"
if ! python3 - "$workdir" "$seed" <<'PY'
from pathlib import Path
import itertools, random, sys
base = Path(sys.argv[1])
rng = random.Random(int(sys.argv[2]))
manifest = []
def case(label, records, rows, empty=False):
    directory = base / str(len(manifest))
    logs = directory / 'logs'
    logs.mkdir(parents=True)
    for name, passed, total in reversed(records):
        noise = 'noise\n PASSED: 999/999\n'
        earlier = f'PASSED: {total+100}/{total+101}\n' * rng.randrange(1, 4)
        final = f'PASSED: {passed}/{total}\nfinished\n'
        content = '' if empty else noise + (earlier if rows > 1 else '') + (final if rows else '')
        (logs / name).write_text(content)
    invalid = bool(records) and rows == 0
    red = ' '.join(n for n, p, t in sorted(records) if p < t) or 'none'
    expected = 'InvalidLine' if invalid else f'{sum(p for _, p, _ in records)}/{sum(t for _, _, t in records)} passed; red: {red}'
    (directory / 'expect').write_text(expected + '\n')
    manifest.append([directory.name, label, int(invalid)])
for count, color, rows, names in itertools.product((1, 2, 'many'), ('green', 'red', 'mixed'), (0, 1, 2), ('ascii', 'utf8')):
    records = []
    for index in range(rng.randrange(3, 9) if count == 'many' else count):
        total = rng.randrange(1, 10000)
        passed = rng.randrange(total) if color == 'red' or (color == 'mixed' and index % 2 == 0) else total
        name = ('門番' if names == 'utf8' and index % 2 == 0 else 'gate') + f'-{index}-{rng.randrange(100000)}.log'
        records.append((name, passed, total))
    case(f'{count}/{color}/rows={rows}/{names}', records, rows)
case('empty-directory', [], 0)
case('empty-log', [('empty.log', 0, 0)], 0, empty=True)
case('zero-total', [('zero.log', 0, 0)], 1)
assert len(manifest) == len(list(itertools.product((1, 2, 'many'), ('green', 'red', 'mixed'), (0, 1, 2), ('ascii', 'utf8')))) + 3
(base / 'manifest.tsv').write_text(''.join(f'{i}\t{label}\t{status}\n' for i, label, status in manifest))
PY
then
    echo 'NG: log generator' >&2
    exit 1
fi

output_matches() {
    local status="$1" expected_status="$2" dir="$3"
    [ "$status" -eq "$expected_status" ] || return 1
    if [ "$expected_status" -eq 0 ]; then
        [ "$(wc -c <"$dir/err")" -eq 0 ] && cmp -s "$dir/out" "$dir/expect"
    else
        [ "$(wc -c <"$dir/out")" -eq 0 ] && cmp -s "$dir/err" "$dir/expect"
    fi
}
output_controls() {
    local dir="$workdir/control" status good empty
    mkdir "$dir" || return 1
    printf 'value\n' >"$dir/expect"
    for status in 0 1; do
        good=out; empty=err
        if [ "$status" -eq 1 ]; then good=err; empty=out; fi
        cp "$dir/expect" "$dir/$good"
        : >"$dir/$empty"
        output_matches "$status" "$status" "$dir" || return 1
        if output_matches "$((1-status))" "$status" "$dir"; then return 1; fi
        printf '\n' >"$dir/$empty"
        if output_matches "$status" "$status" "$dir"; then return 1; fi
        : >"$dir/$empty"
        printf 'different\n' >"$dir/$good"
        if output_matches "$status" "$status" "$dir"; then return 1; fi
        : >"$dir/$good"
        if output_matches "$status" "$status" "$dir"; then return 1; fi
    done
}
check 'output controls: equal, empty, different, newline, status' output_controls
oracle() (
    export LC_ALL=C
    shopt -s nullglob dotglob
    local file line counts sum_passed=0 sum_total=0 red=()
    for file in "$1"/*; do
        line=$(grep -E '^PASSED:' "$file" | tail -1)
        if [[ ! "$line" =~ ^PASSED:\ ([0-9]+)/([0-9]+)$ ]]; then
            echo InvalidLine >&2
            return 1
        fi
        counts=("${BASH_REMATCH[1]}" "${BASH_REMATCH[2]}")
        sum_passed=$((sum_passed + 10#${counts[0]}))
        sum_total=$((sum_total + 10#${counts[1]}))
        if (( 10#${counts[0]} < 10#${counts[1]} )); then red+=("${file##*/}"); fi
    done
    printf '%s/%s passed; red: %s\n' "$sum_passed" "$sum_total" "${red[*]:-none}"
)
oracle_case() {
    local dir="$1" expected_status="$2" status=0
    oracle "$dir/logs" >"$dir/out" 2>"$dir/err" || status=$?
    output_matches "$status" "$expected_status" "$dir"
}
while IFS=$'\t' read -r id label status; do
    check "38-a oracle $label" oracle_case "$workdir/$id" "$status"
done <"$workdir/manifest.tsv"

mkdir "$workdir/e2" || exit 1
cp tools/gates/main.hd "$workdir/e2/main.hd" || exit 1
cp -r tools/gates/report "$workdir/e2/report" || exit 1
cargo run --offline -q -- check "$workdir/e2/main.hd" || exit 1
cargo run --offline -q -- build "$workdir/e2/main.hd" || exit 1
e2_case() {
    local dir="$1" expected_status="$2" status=0
    shift 2
    "$workdir/e2/main" "$@" >"$dir/out" 2>"$dir/err" || status=$?
    cat "$dir/out" "$dir/err"
    output_matches "$status" "$expected_status" "$dir"
}
while IFS=$'\t' read -r id label status; do
    check "38-b E2 $label" e2_case "$workdir/$id" "$status" "$workdir/$id/logs"
done <"$workdir/manifest.tsv"
mkdir -p "$workdir/missing-argv" "$workdir/subdirectory/logs/child" || exit 1
printf 'MissingDirectory\n' >"$workdir/missing-argv/expect"
printf 'IoFailure\n' >"$workdir/subdirectory/expect"
check '38-b empty argv' e2_case "$workdir/missing-argv" 1
check '38-b subdirectory is IoFailure' e2_case "$workdir/subdirectory" 1 "$workdir/subdirectory/logs"

if ! python3 - "$workdir" "$seed" "$((1+RANDOM%7))" "$((1+RANDOM%9))" "$((1+RANDOM%7))" "$((1+RANDOM%9))" <<'PY'
from pathlib import Path
import json, random, re, sys
base = Path(sys.argv[1]) / 'probes'
base.mkdir()
rng = random.Random(int(sys.argv[2]))
manifest = []
def add(label, source, expected, status=0, mode='run'):
    directory = base / str(len(manifest))
    directory.mkdir()
    (directory / 'main.hd').write_text(source)
    (directory / 'expect').write_text(expected + '\n')
    manifest.append([directory.name, label, status, mode])
    return directory
value = rng.randrange(1, 1000)
pairs = [([value], [value, value+1]), ([value, value+1], [value]), ([value], [value]), ([value, value], [value+1])]
calls = [f'compare (Array.of_list {a}) (Array.of_list {b})' for a, b in pairs]
add('38-c(1) §45 Array compare', 'def main = [' + ', '.join(calls) + ']\n', str([(a > b) - (a < b) for a, b in pairs]))
lower, upper = -(2**63), 2**63 - 1
for value in ['007', '+42', ' 42', '1_000', '1.0', '４２', *map(str, [upper, lower, upper+1, lower-1])]:
    accepted = bool(re.fullmatch(r'-?[0-9]+', value)) and lower <= int(value) <= upper
    add('38-c(2) §49 Int.parse ' + json.dumps(value, ensure_ascii=False),
        'def main = Int.parse ' + json.dumps(value, ensure_ascii=False) + '\n',
        str(int(value)) if accepted else 'InvalidInt', int(not accepted))
names = ['.hidden', 'z' + str(rng.randrange(10000)), 'A', 'あ', 'é', 'child']
directory = add('38-c(3) §49 read_dir', 'def main = read_dir "entries"\n',
                json.dumps(sorted(names, key=lambda s: s.encode()), ensure_ascii=False))
(directory / 'entries').mkdir()
for name in reversed(names):
    (directory / 'entries' / name).touch()
(directory / 'entries' / 'child').unlink()
(directory / 'entries' / 'child').mkdir()
data = bytes(rng.randrange(256) for _ in range(rng.randrange(1, 2048)))
directory = add('38-c(4) §49 read_stdin twice',
                'def main = let first = Bytes.length read_stdin -> let second = Bytes.length read_stdin -> [first, second]\n', str([len(data), 0]))
(directory / 'stdin').write_bytes(data)
add('38-c(5) §49 env non-UTF8 value', 'def main = env "HADO_R38_BAD_VALUE"\n', 'InvalidUtf8', 1)
for name, blank, indent in [('EnvDefault', int(sys.argv[3]), int(sys.argv[4])), ('IoDefault', int(sys.argv[5]), int(sys.argv[6]))]:
    source = 'def main = 0\n' + '\n' * blank + ' ' * indent + 'module ' + name + '\n'
    add('38-c(6) §47 module ' + name, source, json.dumps(['E205', blank+2, indent+8]), 1, 'reject')
assert {label.split()[0] for _, label, _, _ in manifest} == {f'38-c({i})' for i in range(1, 7)}
(base / 'manifest.tsv').write_text(''.join(f'{i}\t{label}\t{status}\t{mode}\n' for i, label, status, mode in manifest))
PY
then
    echo 'NG: probe generator' >&2
    exit 1
fi
probe() {
    local dir="$1" expected_status="$2" mode="$3" status=0
    if [ "$mode" = reject ]; then
        cargo run --offline -q -- check "$dir/main.hd" >"$dir/out" 2>"$dir/err" || status=$?
        cat "$dir/err"
        [ "$status" -eq 1 ] && [ "$(wc -c <"$dir/out")" -eq 0 ] || return 1
        python3 - "$dir" <<'PY'
from pathlib import Path
import json, re, sys
directory = Path(sys.argv[1])
code, line, col = json.loads((directory / 'expect').read_text())
actual = (directory / 'err').read_text()
assert re.fullmatch(re.escape(f'main.hd:{line}:{col}: error[{code}]: ') + r'[^\n]+\n', actual), actual
PY
        return $?
    fi
    cargo run --offline -q -- build "$dir/main.hd" >"$dir/build.out" 2>"$dir/build.err" || { cat "$dir/build.err"; return 1; }
    [ "$(wc -c <"$dir/build.err")" -eq 0 ] || return 1
    if [ ! -f "$dir/stdin" ]; then : >"$dir/stdin"; fi
    (cd "$dir" && HADO_R38_BAD_VALUE=$'\xff' ./main <stdin >out 2>err) || status=$?
    cat "$dir/out" "$dir/err"
    output_matches "$status" "$expected_status" "$dir"
}
while IFS=$'\t' read -r id label status mode; do
    check "$label" probe "$workdir/probes/$id" "$status" "$mode"
done <"$workdir/probes/manifest.tsv"

# 38-c coverage: each added rule belongs to the section named by its probes.
check '38-c specification sentence coverage' python3 - "$workdir/probes/manifest.tsv" <<'PY'
from pathlib import Path
import re, sys
rules = {
    '38-c(1)': (45, r'^Array の `compare` は要素を先頭から比較して最初に異なる要素の順序を返し、共通部分がすべて等しいときだけ短い方を先とする。$'),
    '38-c(2)': (49, r'^`Int\.parse` の受理文法は、往復から導いた下限として `Int\.to_string` が返す全ての Int の10進表記を含み、'
                   r'本節が決めた上限として ASCII の `-\?\[0-9\]\+` に一致して値が -9223372036854775808 以上 9223372036854775807 以下である文字列だけを受理し、'
                   r'先行ゼロの個数は制限せず、`\+`・空白・`_`・小数点・全角数字は拒否する。$'),
    '38-c(3)': (49, r'^`read_dir` は指定したディレクトリの各項目の名前だけを返し、`\.` と `\.\.` を除いて UTF-8 のバイト順に整列する。$'),
    '38-c(4)': (49, r'^`read_stdin` は標準入力を EOF まで読み、同じプロセスでの2回目の呼び出しは空の `Bytes` を返す。$'),
    '38-c(5)': (49, r'^`env` の値が非 UTF-8 のときも、`message` が `"InvalidUtf8"` の `Error` を raise する。$'),
    '38-c(6)': (47, r'^既定実装の契約名 `EnvDefault` と `IoDefault` は契約の名前空間で予約し、利用者の同名の契約宣言は E205 とする。$'),
}
def coverage(spec, manifest):
    sections = dict(re.findall(r'^# (\d+)\.[^\n]*\n(.*?)(?=^# \d+\.|\Z)', spec, re.M | re.S))
    probes = set(re.findall(r'\t(38-c\(\d+\)) §(\d+) ', manifest))
    assert probes == {(key, str(section)) for key, (section, _) in rules.items()}, probes
    for key, (section, pattern) in rules.items():
        assert len(re.findall(pattern, sections[str(section)], re.M)) == 1, key
spec = Path('docs/spec.md').read_text()
manifest = Path(sys.argv[1]).read_text()
coverage(spec, manifest)
for key, (_, pattern) in rules.items():
    try:
        coverage(re.sub(pattern, '', spec, flags=re.M), manifest)
    except AssertionError:
        print(f'{key}: missing sentence detected')
    else:
        raise AssertionError(f'{key}: missing sentence accepted')
for invalid in ['', manifest.replace('§45', '§49')]:
    try:
        coverage(spec, invalid)
    except AssertionError:
        pass
    else:
        raise AssertionError('missing or mismatched coverage accepted')
print('coverage: §45=1 §47=1 §49=4')
PY

# 38-d runs the commands printed in each bug section, without a second probe copy.
check '38-d documented reproductions and controls' python3 - <<'PY'
from pathlib import Path
import re, subprocess

def reproductions(document):
    result = []
    for section in re.split(r'^## ', document, flags=re.M)[1:]:
        title = section.splitlines()[0]
        commands = re.findall(r'^```bash\n(.*?)^```$', section, re.M | re.S)
        expected = re.findall(r'^```text\n(.*?)^```$', section, re.M | re.S)
        assert len(commands) == len(expected) == 1, title
        result.append((title, commands[0], expected[0].encode()))
    return result

def reproduced(command, expected):
    result = subprocess.run(['bash', '-euo', 'pipefail', '-c', command], capture_output=True, timeout=60)
    return result.returncode == 0 and result.stdout == expected and result.stderr == b''

assert reproductions('# 既知の不具合\n') == []
for malformed in ['## broken\n', '## broken\n```bash\ntrue\n```\n']:
    try:
        reproductions(malformed)
    except AssertionError:
        pass
    else:
        raise AssertionError('section without a complete reproduction accepted')
assert reproduced("printf 'value\\n'", b'value\n')
for command in ['true', "printf 'wrong\\n'", "printf 'value\\n'; printf '\\n' >&2", "printf 'value\\n'; exit 1"]:
    assert not reproduced(command, b'value\n'), command
document = Path('docs/bug.md').read_text()
cases = reproductions(document)
assert len(cases) == len(re.findall(r'^## ', document, flags=re.M)), 'section/reproduction count mismatch'
for title, command, expected in cases:
    assert reproduced(command, expected), title
    print(f'{title}: reproduced')
    print(expected.decode(), end='')
    assert not reproduced(command, expected + b'wrong\n'), title
    print(f'{title}: changed expectation detected')
print(f'documented reproductions: {len(cases)}')
PY

# 38-e finishing checks.
corpus_lock() {
    local status=0 ok ng
    LC_ALL=C sha256sum --strict -c tests/corpus.sha256 >"$workdir/lock.log" 2>&1 || status=$?
    ok=$(grep -c ': OK$' "$workdir/lock.log")
    ng=$(grep -cv ': OK$' "$workdir/lock.log")
    echo "corpus lock: OK=$ok NG=$ng status=$status"
    if [ "$status" -ne 0 ] || [ "$ng" -ne 0 ]; then cat "$workdir/lock.log"; fi
    [ "$status" -eq 0 ] && [ "$ok" -gt 0 ] && [ "$ng" -eq 0 ]
}
check '38-e corpus lock' corpus_lock
specification_version() {
    local title
    IFS= read -r title <docs/spec.md || return 1
    echo "specification version: $title"
    [ "$title" = '# 言語仕様案 v0.3' ]
}
check '38-e specification version v0.3' specification_version
ledger_section() {
    local count
    count=$(grep -c '^## R38 ' docs/ledger.md) || return 1
    echo "R38 ledger sections: $count"
    [ "$count" -gt 0 ]
}
check '38-e R38 ledger section' ledger_section

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
