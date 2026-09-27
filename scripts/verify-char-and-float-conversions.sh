#!/usr/bin/env bash
# tier: core
# round: char-and-float-conversions
# Char module と Float.parse / Float.of_int、§54 の正準形の Float.to_string を
# 測る門番（docs/rounds/char-and-float-conversions.md の判定）。
#
# core にする理由: 判定はすべて決定的な意味論で、大きな入力や時間の比を測らない。
# 1 ケース 0.2 秒ほどの hado run を数十回走らせるだけで、PR を回す core 層に
# 置いても安い。Float の印字は §54 の全型の印字の一部なので、quick より後、
# full より前で毎 PR に測る。
#
# 期待値は判定器側で計算する。正準形は Python の repr（往復可能な最短の 10 進桁）
# から指数を使わない位取りへ展開し、parse の合否は「文法一致」と「有限」と
# 「0 でない桁があるのに 0.0 でない」から導く。コーパスのファイル名や行は
# 埋め込まず、`// spec: §55` と本文のトークンで選ぶ。
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
hado="target/debug/hado"
hado_abs="$(cd "$(dirname "$hado")" && pwd)/$(basename "$hado")"
root="$(pwd)"

cat >"$workdir/harness.py" <<'PY'
import math
import re
import subprocess
from pathlib import Path

HADO = None
WORK = None
_serial = [0]


def canonical(x):
    """§54 の Float の正準形。Python の repr が返す往復可能な最短の 10 進桁を、
    指数を使わず位取りで並べ、小数点の両側に最低 1 桁を置く。"""
    if math.isnan(x):
        return 'nan'
    if math.isinf(x):
        return 'inf' if x > 0 else '-inf'
    negative = math.copysign(1.0, x) < 0
    match = re.fullmatch(r'(\d+)(?:\.(\d+))?(?:[eE]([+-]?\d+))?', repr(abs(x)))
    assert match, repr(x)
    whole, fraction, exponent = match.group(1), match.group(2) or '', int(match.group(3) or 0)
    digits = whole + fraction
    point = len(whole) + exponent
    if point <= 0:
        body = '0.' + '0' * (-point) + digits
    elif point >= len(digits):
        body = digits + '0' * (point - len(digits)) + '.0'
    else:
        body = digits[:point] + '.' + digits[point:]
    return ('-' if negative else '') + body


def parse_expected(text):
    """受理文法 `"-"? digit+ "." digit+` と、丸めの境界。受理なら f64、拒否なら None。
    文法は文字列全体に一致することを要する。丸めた結果が無限大になる表記と、
    0 でない桁があるのに 0.0 に丸まる表記は拒否する。"""
    if re.fullmatch(r'-?[0-9]+\.[0-9]+', text) is None:
        return None
    value = float(text)
    if math.isinf(value):
        return None
    if value == 0.0 and re.search(r'[1-9]', text) is not None:
        return None
    return value


def hado_run(name, source):
    _serial[0] += 1
    directory = Path(WORK) / f'{name}-{_serial[0]}'
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / 'main.hd'
    path.write_text(source)
    return subprocess.run([HADO, 'run', str(path)], capture_output=True, timeout=30)


def expect(name, source, stdout, code=0, stderr=b''):
    done = hado_run(name, source)
    assert done.returncode == code, (name, done.returncode, done.stdout[:200], done.stderr[:200])
    assert done.stdout == stdout, (name, done.stdout[:200], stdout[:200])
    assert done.stderr == stderr, (name, done.stderr[:200], stderr[:200])


def quote(text):
    assert '"' not in text and '\\' not in text, text
    return '"' + text + '"'
PY

# --- a: Char.to_int / of_int / to_string の受理域と UTF-8 ---
char_check() {
    python3 - "$workdir" "$hado_abs" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
harness.WORK = sys.argv[1]
from harness import expect

accepted = [0, 55295, 57344, 1114111, 12354, 128512]
rejected = [-1, 55296, 57343, 1114112]
values = '[' + ', '.join(str(n) for n in accepted) + ']'

expect('char_roundtrip', f'def main = List.map {values} fn(n => Char.to_int (Char.of_int n))',
       ('[' + ', '.join(str(n) for n in accepted) + ']\n').encode())
expect('char_lengths',
       f'def main = List.map {values} fn(n => String.length (Char.to_string (Char.of_int n)))',
       ('[' + ', '.join('1' for _ in accepted) + ']\n').encode())
expect('char_bytes',
       f'def main = List.map {values} '
       'fn(n => Bytes.to_list (Bytes.of_string (Char.to_string (Char.of_int n))))',
       (str([list(chr(n).encode()) for n in accepted]) + '\n').encode())
expect('char_to_int_literal', "def main = Char.to_int 'a'", b'97\n')
# R1 の残り: Char.to_string の 1 文字は Unicode スカラ値 1 個である。
expect('char_to_string_length', "def main = String.length (Char.to_string 'あ')", b'1\n')
for n in rejected:
    expect(f'char_invalid_{n}', f'def main = Char.of_int ({n})',
           b'', code=1, stderr=b'InvalidScalar\n')
print(f'char: {len(accepted)} accepted scalars, {len(rejected)} InvalidScalar, UTF-8 bytes')
PY
}

# --- b: Float.to_string が §54 の正準形である ---
float_to_string_check() {
    python3 - "$workdir" "$hado_abs" <<'PY'
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
harness.WORK = sys.argv[1]
from harness import canonical, expect

cases = [
    ('zero', '0.0', 0.0),
    ('negative_zero', '-0.0', -0.0),
    ('tenth', '0.1', 0.1),
    ('three', '3.0', 3.0),
    ('ten_pow_300', '1' + '0' * 300 + '.0', float('1' + '0' * 300 + '.0')),
    ('max_finite', '17976931348623157' + '0' * 292 + '.0',
     float('17976931348623157' + '0' * 292 + '.0')),
    ('min_subnormal', '0.' + '0' * 323 + '5', 5e-324),
    ('infinity', '1.0 / 0.0', float('inf')),
    ('negative_infinity', '-1.0 / 0.0', float('-inf')),
    ('not_a_number', '0.0 / 0.0', float('nan')),
]
for name, expression, value in cases:
    expected = canonical(value) + '\n'
    assert 'e' not in expected and 'E' not in expected, expected
    expect(f'float_to_string_{name}', f'def main = Float.to_string ({expression})',
           expected.encode())
    expect(f'float_to_string_{name}_no_exponent',
           f'def main = String.contains (Float.to_string ({expression})) "e"', b'false\n')
# R1 の残り: 正準形の 0.1 は 3 文字である。
expect('float_to_string_length', 'def main = String.length (Float.to_string 0.1)', b'3\n')
print(f'float_to_string: {len(cases)} canonical spellings, no exponent, nan/inf')
PY
}

# --- c: main の Float の印字が同じ正準形である（Float.to_string だけでは通らない）---
main_print_check() {
    python3 - "$workdir" "$hado_abs" <<'PY'
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
harness.WORK = sys.argv[1]
from harness import canonical, expect

cases = [
    ('zero', '0.0', 0.0),
    ('negative_zero', '-0.0', -0.0),
    ('tenth', '0.1', 0.1),
    ('ten_pow_300', '1' + '0' * 300 + '.0', float('1' + '0' * 300 + '.0')),
    ('max_finite', '17976931348623157' + '0' * 292 + '.0',
     float('17976931348623157' + '0' * 292 + '.0')),
    ('infinity', '1.0 / 0.0', float('inf')),
    ('negative_infinity', '-1.0 / 0.0', float('-inf')),
    ('not_a_number', '0.0 / 0.0', float('nan')),
]
for name, expression, value in cases:
    expect(f'main_print_{name}', f'def main = {expression}', (canonical(value) + '\n').encode())
print(f'main print: {len(cases)} canonical spellings through the CanonicalFloat path')
PY
}

# --- d: Float.parse の受理文法・往復・丸め ---
parse_accepted_check() {
    python3 - "$workdir" "$hado_abs" <<'PY'
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
harness.WORK = sys.argv[1]
from harness import canonical, expect, parse_expected, quote

max_finite = '17976931348623157' + '0' * 292 + '.0'
spellings = ['0.0', '-0.0', '0.1', '1.0', '007.5', '-012.0', '1.00',
             '9007199254740993.0', max_finite, '-' + max_finite,
             '0.' + '0' * 323 + '3', '0.' + '0' * 323 + '5',
             '-' + '0.' + '0' * 323 + '3', '-' + '0.' + '0' * 323 + '5']
for index, text in enumerate(spellings):
    value = parse_expected(text)
    assert value is not None, text
    expect(f'parse_accepted_{index}',
           f'def main = Float.to_string (Float.parse {quote(text)})',
           (canonical(value) + '\n').encode())
# 往復: parse (to_string x) は有限値で x に戻る。文字列で比べる。
round_trips = ['0.0', '-0.0', '0.1', '3.0', '1' + '0' * 300 + '.0']
checks = ', '.join(
    f'Float.to_string (Float.parse (Float.to_string ({text}))) == {quote(canonical(float(text)))}'
    for text in round_trips)
expect('parse_roundtrip', f'def main = [{checks}]',
       ('[' + ', '.join('true' for _ in round_trips) + ']\n').encode())
print(f'parse accepted: {len(spellings)} spellings and {len(round_trips)} round trips')
PY
}

# --- e: Float.parse の文法外の入力・オーバーフロー・アンダーフロー ---
parse_rejected_check() {
    python3 - "$workdir" "$hado_abs" <<'PY'
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
harness.WORK = sys.argv[1]
from harness import expect, parse_expected, quote

grammar = ['', '-', '1', '1.', '.', '.5', '+1.0', '1e3', '1.0e3', ' 1.0', '1.0 ',
           '0x1', '0x1.0', '1.2.3']
overflow = ['2' + '0' * 308 + '.0', '-' + '2' + '0' * 308 + '.0']
underflow = ['0.' + '0' * 323 + '1', '-' + '0.' + '0' * 323 + '1',
             '0.' + '0' * 323 + '2', '-' + '0.' + '0' * 323 + '2']
# 2^-1075（最小の非正規数のちょうど半分）。ties-to-even で 0 に丸まる。
halfway = '0.' + '0' * 323 + str(5 ** 1075)
underflow += [halfway, '-' + halfway]
rejected = grammar + overflow + underflow
for index, text in enumerate(rejected):
    assert parse_expected(text) is None, text
    expect(f'parse_rejected_{index}',
           f'def main = Float.parse {quote(text)}',
           b'', code=1, stderr=b'InvalidFloat\n')
print(f'parse rejected: {len(grammar)} grammar, {len(overflow)} overflow, {len(underflow)} underflow')
PY
}

# --- f: Float.of_int の i64 から f64 への丸め ---
of_int_check() {
    python3 - "$workdir" "$hado_abs" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
harness.WORK = sys.argv[1]
from harness import canonical, expect

values = [9007199254740992, 9007199254740993, 9007199254740995,
          -9007199254740993, -9007199254740995, 9223372036854775807]
# i64 の最小値はリテラルに書けない（E112）ので引き算で作る（exit-code-from-main と同じ）。
i64_min = '0 - 9223372036854775807 - 1'
source = ('def main = [' +
          ', '.join(f'Float.to_string (Float.of_int ({n}))' for n in values) +
          f', Float.to_string (Float.of_int ({i64_min}))]\n')
expected = [canonical(float(n)) for n in values] + [canonical(float(-9223372036854775808))]
expect('of_int_rounding', source,
       ('[' + ', '.join('"' + text + '"' for text in expected) + ']\n').encode())
print(f'of_int: {len(values) + 1} i64 values rounded to nearest, ties to even')
PY
}

# --- g: §55 の関数集合・署名・失敗の表・受理文法 ---
spec_check() {
    python3 - "$root" <<'PY'
import re
import sys
from pathlib import Path

spec = Path(sys.argv[1], 'docs/spec.md').read_text()
section = re.search(r'^## 55\.[^\n]*\n(.*?)(?=^## \d+\.|\Z)', spec, re.M | re.S)
assert section, 'missing §55'
section = section.group(1)
block = re.search(r'関数集合は次とする。\s*```text\n(.*?)```', section, re.S)
assert block, 'missing the §55 function set'
required = {}
for line in block.group(1).splitlines():
    if ':' not in line:
        continue
    names, members = line.split(':', 1)
    for name in re.split(r'\s*/\s*', names.strip()):
        required[name] = members.split()
assert set(required.get('Char', [])) == {'to_int', 'of_int', 'to_string'}, required.get('Char')
assert {'to_string', 'parse', 'of_int'} <= set(required.get('Float', [])), required.get('Float')
signatures = [
    'def to_int (value: Char): Int Pure',
    'def of_int (value: Int): Char {Error}',
    'def to_string (value: Char): String Pure',
    'def parse (value: String): Float {Error}',
    'def of_int (value: Int): Float Pure',
]
for signature in signatures:
    assert signature in section, signature
rows = [line for line in section.splitlines() if line.startswith('|')]
assert any('Float.parse' in row and '"InvalidFloat"' in row for row in rows), rows
assert any('Char.of_int' in row and '"InvalidScalar"' in row for row in rows), rows
assert '"-"? digit+ "." digit+' in section, 'missing the Float.parse grammar'
print('spec: §55 function set, 5 signatures, 2 failure rows, grammar')
PY
}

# --- h: §55 の受理コーパス（expect と expect-error）を実走する ---
corpus_check() {
    python3 - "$root" "$hado_abs" <<'PY'
import subprocess
import sys
from pathlib import Path

hado, root = sys.argv[2], Path(sys.argv[1])
tokens = ('Float.parse', 'Float.of_int', 'Char.of_int', 'Char.to_string')
selected = []
for path in sorted((root / 'tests/accept').glob('*/main.hd')):
    text = path.read_text()
    if '// spec: §55' in text and any(token in text for token in tokens):
        selected.append((path, text))
assert selected, 'no accept corpus case for the §55 conversions'
errors = {}
for path, text in selected:
    heads = []
    for line in text.splitlines():
        if line.startswith('// expect-error:'):
            heads.append((1, line[len('// expect-error:'):].strip()))
        elif line.startswith('// expect:'):
            heads.append((0, line[len('// expect:'):].strip()))
    assert len(heads) == 1, path
    status, value = heads[0]
    output = (value + '\n').encode() if value else b''
    stdout, stderr = (output, b'') if status == 0 else (b'', output)
    done = subprocess.run([str(hado), 'run', str(path)], capture_output=True, timeout=30)
    assert (done.returncode, done.stdout, done.stderr) == (status, stdout, stderr), \
        (path, done.returncode, done.stdout[:200], done.stderr[:200])
    if status == 1:
        errors[value] = errors.get(value, 0) + 1
for message in ('InvalidFloat', 'InvalidScalar'):
    assert errors.get(message, 0) >= 1, (message, errors)
print(f'corpus: {len(selected)} §55 cases run, errors={errors}')
PY
}

# --- i: 判定器そのものの較正（期待値の計算が正準形と往復を満たす）---
judge_check() {
    python3 - "$workdir" <<'PY'
import math
import sys

sys.path.insert(0, sys.argv[1])
from harness import canonical, parse_expected

samples = [0.0, -0.0, 0.1, 3.0, 1e300, -1e300, 1.7976931348623157e308,
           5e-324, -5e-324, 2.2250738585072014e-308]
for value in samples:
    text = canonical(value)
    assert 'e' not in text and 'E' not in text, text
    assert math.copysign(1.0, float(text)) == math.copysign(1.0, value), (value, text)
    assert float(text) == value, (value, text)
accepted = ['0.0', '007.5', '1.00', '9007199254740993.0',
            '-0.' + '0' * 323 + '3', '-0.' + '0' * 323 + '5']
for text in accepted:
    assert parse_expected(text) is not None, text
rejected = ['', '1', '1.', '.5', '+1.0', '1e3', '0x1', '1.2.3',
            '2' + '0' * 308 + '.0', '0.' + '0' * 323 + '1',
            '-' + '0.' + '0' * 323 + '2']
for text in rejected:
    assert parse_expected(text) is None, text
print('judge: canonical round-trips, no exponent, and parse_expected agrees')
PY
}

check 'a char: boundary scalars, round trip, UTF-8 bytes and InvalidScalar' char_check
check 'b float_to_string: §54 canonical spellings, no exponent, nan and inf' float_to_string_check
check 'c main print: the same canonical spellings through CanonicalFloat' main_print_check
check 'd parse accepted: grammar, round trip and correct rounding' parse_accepted_check
check 'e parse rejected: grammar, overflow and underflow are InvalidFloat' parse_rejected_check
check 'f of_int: i64 to f64 round to nearest, ties to even' of_int_check
check 'g spec: §55 inventory, signatures, failure rows and grammar' spec_check
check 'h corpus: the §55 accept cases with expect and expect-error run' corpus_check
check 'i judge: canonical and parse_expected are calibrated' judge_check

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
