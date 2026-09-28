#!/usr/bin/env bash
# tier: full
# round: string-split-linear
# String.split / String.join の時間を入力長に線形にし、String.length を O(1) に
# したことを測る門番（docs/rounds/string-split-linear.md の判定）。
#
# full にする理由: 判定が実測の比そのもので、入力と実行ファイルを作る費用が
# 大きい。wide の対照は 30 MB〜60 MB の入力を生成し、各実行ファイルを
# warm-up 1 回のあと 5 回測る。quick / core ではその費用に見合う変更が
# 入らない（このラウンドの成果物は runtime と prelude の 2 点だけ）。
#
# 時間の比だけでは走査を隠せるので、String.length の判定は runtime の
# バイト走査を数える計器（hado_byte_scans）で行う。計器は構築のあと・
# String.length の直前に 0 に戻し、length 実装が読んだバイトごとに増える。
#
# 他のラウンドの門番と同じ計算機で走るので、比は子プロセスの CPU 時間
# （user + sys）で測り、対照と対象を 1 回ずつ交互に測って各回の差の中央値を
# 取る。大きい側は wall の 10 秒 timeout で別に押さえる。
#
# 「本文が a 以外で候補位置が 1 文字で外れる」入力を避けるため、区切りの
# 長さを入力と一緒に増やす対照（wide）の本文は a（多バイト側は あ）の
# 連続 1 本とし、区切りは末尾に 1 回だけ置く。
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
import resource
import statistics
import subprocess
import time
from pathlib import Path

HADO = None
# 実行ファイルの実行時間だけを測る（build は 1 回だけ別に呼ぶ）。大きい側には
# 比とは別に 10 秒の timeout を置く。
LARGE_TIMEOUT = 10.0
RATIO = 2.5
RUNS = 5
SMALL_FLOOR = 0.2


def build(root, name, source):
    directory = Path(root) / name
    directory.mkdir(parents=True, exist_ok=True)
    source_path = directory / 'main.hd'
    source_path.write_text(source)
    binary = directory / 'program'
    done = subprocess.run([HADO, 'build', '-o', str(binary), str(source_path)],
                          capture_output=True)
    assert done.returncode == 0, (name, done.stderr.decode()[:800])
    return binary


def run(binary, stdin=b'', timeout=LARGE_TIMEOUT, args=()):
    return subprocess.run([str(binary), *args], input=stdin, capture_output=True,
                          timeout=timeout)


def expect(binary, stdout, stdin=b'', code=0, stderr=b'', timeout=LARGE_TIMEOUT, args=()):
    done = run(binary, stdin, timeout, args)
    assert done.returncode == code, (binary, done.returncode, done.stderr[:400])
    assert done.stdout == stdout, (binary, done.stdout[:200], stdout[:200])
    assert done.stderr == stderr, (binary, done.stderr[:200], stderr[:200])


def child_cpu_seconds():
    """待たされた子プロセスの CPU 時間（user + sys）。層の違う門番が同じ計算機で
    走っていても、順番待ちの時間は入らない。timeout は wall 時計で別に掛ける。"""
    usage = resource.getrusage(resource.RUSAGE_CHILDREN)
    return usage.ru_utime + usage.ru_stime


def median(binary, stdin=b''):
    """warm-up 1 回のあと RUNS 回測り、中央値を返す。"""

    def once():
        start = child_cpu_seconds()
        subprocess.run([str(binary)], input=stdin, capture_output=True,
                       timeout=LARGE_TIMEOUT, check=True)
        return child_cpu_seconds() - start

    once()
    return statistics.median(once() for _ in range(RUNS))


def medians(control_binary, target_binary, stdin=b''):
    """対照と対象を 1 回ずつ交互に測り、(対照の中央値, 対象の中央値, 差の中央値)
    を返す。差は各回の (対象 - 対照) の中央値である。順番待ちや GC の揺れを各回で
    打ち消すため、中央値どうしの差ではなく組ごとの差の中央値を使う。warm-up は
    両方に 1 回。"""

    def once():
        start = child_cpu_seconds()
        subprocess.run([str(control_binary)], input=stdin, capture_output=True,
                       timeout=LARGE_TIMEOUT, check=True)
        control = child_cpu_seconds() - start
        start = child_cpu_seconds()
        subprocess.run([str(target_binary)], input=stdin, capture_output=True,
                       timeout=LARGE_TIMEOUT, check=True)
        target = child_cpu_seconds() - start
        return control, target

    once()
    controls = []
    targets = []
    for _ in range(RUNS):
        control, target = once()
        controls.append(control)
        targets.append(target)
    differences = [target - control for control, target in zip(controls, targets)]
    return (statistics.median(controls), statistics.median(targets),
            statistics.median(differences))


def ratio_verdict(small_total, small_control, large_total, large_control):
    """対照を引いた時間の比で線形かどうかを決める（issue の完了条件）。"""
    small = small_total - small_control
    large = large_total - large_control
    if small <= 0:
        return False
    return large / small <= RATIO


def assert_linear(label, small_total, small, large_total, large):
    assert small_total >= SMALL_FLOOR, \
        f'{label}: small median {small_total:.3f}s is below the {SMALL_FLOOR:.3f}s floor'
    ratio = large / small if small > 0 else float('inf')
    assert ratio <= RATIO, (
        f'{label}: control-subtracted ratio {ratio:.2f} > {RATIO} '
        f'(small={small:.3f}s large={large:.3f}s)')
    assert large_total < LARGE_TIMEOUT, \
        f'{label}: large median {large_total:.3f}s reached the {LARGE_TIMEOUT:.0f}s timeout'
    print(f'{label}: small={small_total:.3f}s large={large_total:.3f}s '
          f'subtracted {small:.3f}->{large:.3f} ratio {ratio:.2f}')
PY

# --- a: split と join の意味（組み込みへ移す前後で変えない）---
semantics() {
    python3 - "$workdir" "$hado_abs" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import build, expect

root = Path(sys.argv[1]) / 'a'
root.mkdir(parents=True, exist_ok=True)
cases = [
    ('split_overlap', 'String.split "aaaaa" "aa" == ["", "", "a"]', 'true\n'),
    ('split_empty_subject', 'String.split "" "," == [""]', 'true\n'),
    ('split_trailing', 'String.split "a,b," "," == ["a", "b", ""]', 'true\n'),
    ('split_single', 'String.split "a" "," == ["a"]', 'true\n'),
    ('split_no_separator', 'String.split "abc" "-" == ["abc"]', 'true\n'),
    ('split_multibyte', 'String.split "あいう" "いう" == ["あ", ""]', 'true\n'),
    ('split_multibyte_separator', 'String.split "あ😀い😀" "😀" == ["あ", "い", ""]', 'true\n'),
    ('join_empty', 'String.join [] "," == ""', 'true\n'),
    ('join_single', 'String.join ["a"] "," == "a"', 'true\n'),
    ('join_many', 'String.join ["a", "b", "c"] "," == "a,b,c"', 'true\n'),
    ('join_multibyte', 'String.join ["あ", "い"] "、" == "あ、い"', 'true\n'),
    ('join_empty_separator', 'String.join ["a", "b"] "" == "ab"', 'true\n'),
    ('join_elements_with_separator',
     'String.join (String.split "a,b," ",") "," == "a,b,"', 'true\n'),
]
for name, expression, expected in cases:
    expect(build(root, name, f'def main = {expression}'), expected.encode())
# 空の区切りは §55 のとおり EmptySeparator（stderr、終了コード 1）。
expect(build(root, 'empty_separator', 'def main = String.split "abc" ""'),
       b'', code=1, stderr=b'EmptySeparator\n')
print(f'semantics: {len(cases)} split/join equations and the EmptySeparator error')
PY
}

# --- b: String.length が走査しないこと（計器）と、時間が長さに依らないこと ---
length_probe() {
    local source="$workdir/length_probe.c"
    cat >"$source" <<'C'
#include "hado_runtime.c"
#include <assert.h>
#include <string.h>
#include <time.h>

/* この探針は runtime を直接組み、Rust の Float 整形器をリンクしない。
   Float の整形は測らないので、hado_runtime.c が参照する宣言だけ満たす控えを
   置く（char-and-float-conversions で hado_float_to_string が共有した）。 */
size_t hado_format_float(double value, unsigned char *bytes, size_t capacity) {
  int written = snprintf((char *)bytes, capacity, "%.17g", value);
  if (written < 0 || (size_t)written >= capacity) return 0;
  return (size_t)written;
}

static double now_seconds(void) {
  struct timespec ts;
  clock_gettime(CLOCK_MONOTONIC, &ts);
  return (double)ts.tv_sec + (double)ts.tv_nsec / 1e9;
}

static HadoString *repeat_bytes(unsigned char fill, size_t length) {
  HadoString *bytes = hado_bytes_alloc((int64_t)length);
  memset(bytes->bytes, fill, length);
  return bytes;
}

/* 1 万回のうち、構築だけの時間と構築 + String.length の時間を足す。各回は
   新しく作った文字列への 1 回だけの呼び出しで、確保した直後の最初の呼び出し
   も含める（warm-up で初回の走査を隠さない）。 */
static double length_extra(HadoString *(*make)(void)) {
  const int rounds = 20;
  double control = 0.0;
  double target = 0.0;
  for (int i = 0; i < rounds; ++i) {
    double start = now_seconds();
    HadoString *value = make();
    (void)value;
    control += now_seconds() - start;

    start = now_seconds();
    value = make();
    hado_byte_scans = 0;
    int64_t measured = hado_string_length(value);
    assert(measured >= 0);
    assert(hado_byte_scans == 0);
    target += now_seconds() - start;
  }
  return (target - control) / rounds;
}

static HadoString *make_ascii_1(void) { return hado_int_to_string(7); }
static HadoString *make_ascii_1m(void) {
  return hado_bytes_to_string_unchecked(repeat_bytes('a', 1000000));
}
static HadoString *make_japanese_1(void) { return hado_char_to_string(0x3042); }
static HadoString *make_japanese_1m(void) {
  HadoString *bytes = hado_bytes_alloc(3 * 333333);
  for (size_t i = 0; i < 333333; ++i) {
    bytes->bytes[3 * i] = 0xe3;
    bytes->bytes[3 * i + 1] = 0x81;
    bytes->bytes[3 * i + 2] = 0x82;
  }
  return hado_bytes_to_string_unchecked(bytes);
}

int32_t hado_entry(void) {
  struct { HadoString *(*make)(void); size_t scalars; const char *name; } cases[] = {
      {make_ascii_1, 1, "ascii small"},
      {make_ascii_1m, 1000000, "ascii 1000000"},
      {make_japanese_1, 1, "japanese small"},
      {make_japanese_1m, 333333, "japanese 333333"},
  };
  for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); ++i) {
    HadoString *value = cases[i].make();
    assert(value->scalars == cases[i].scalars);
    hado_byte_scans = 0;
    assert(hado_string_length(value) == (int64_t)cases[i].scalars);
    assert(hado_byte_scans == 0);
  }
  double small = length_extra(make_ascii_1);
  double large = length_extra(make_ascii_1m);
  /* 0.2 ms の床は、比の分母が測定の分解能に埋もれるのを防ぐ。走査する実装は
     100 万バイトで床を大きく超える。 */
  assert(large <= small * 2.5 + 0.0002);
  return 0;
}
C
    if ! cc -std=c11 -Wall -Werror -I runtime "$source" -lgc -o "$workdir/length_probe"; then
        return 1
    fi
    "$workdir/length_probe"
}

# --- c: HadoString の配置を移行した runtime の単体テスト ---
runtime_tests() {
    cargo test --offline -q \
        --test runtime_string --test r34_bytes --test r36_int --test r37_world --test repr
}

# --- d: HadoString * を返す関数をソースから列挙し、全部の不変条件を測る ---
abi_probe() {
    python3 - "$workdir" "$root" <<'PY'
import re
import subprocess
import sys
from pathlib import Path

workdir = Path(sys.argv[1])
root = Path(sys.argv[2])
sources = {
    'hado_runtime.c': (root / 'runtime/hado_runtime.c').read_text(),
    'runtime_print.c': (root / 'crates/hado/runtime_print.c').read_text(),
}
pattern = re.compile(r'^(?:static\s+)?HadoString\s*\*\s*(\w+)\s*\(', re.M)
enumerated = set()
for text in sources.values():
    enumerated |= set(pattern.findall(text))

# 返り値の意味は 2 つに分かれる。String を返す関数はスカラ数がバイト列の
# スカラ数と一致し、Bytes を返す関数は未知値を持つ。String.split だけは
# String の Array を返す。分類は正規表現では決まらない（同じ Bytes でも
# Bytes.of_string は String のスカラ数を保つ）ので、ここにデータとして置き、
# ソースからの列挙と両方向で照合する。名前を落とすと列挙側にだけ現れて赤になる。
kind = {
    'hado_string_concat': 'string',
    'hado_string_slice': 'string',
    'hado_string_trim': 'string',
    'hado_int_to_string': 'string',
    'hado_float_to_string': 'string',
    'hado_char_to_string': 'string',
    'hado_bytes_display': 'string',
    'hado_bytes_of_string': 'string',
    'hado_bytes_to_string_unchecked': 'string',
    'hado_string_join': 'string',
    'hado_string_split': 'array',
    'make_piece': 'string',
    'quote_text': 'string',
    'hado_quote_string': 'string',
    'hado_quote_char': 'string',
    'hado_canonical_float': 'string',
    'bytes_from_c_string': 'bytes',
    'hado_argv_get': 'bytes',
    'hado_env_get': 'bytes',
    'hado_cwd': 'bytes',
    'hado_io_take_bytes': 'bytes',
    'hado_io_dir_get': 'bytes',
    'hado_bytes_alloc': 'bytes',
    'hado_bytes_fill_unchecked': 'bytes',
    'grow_io_bytes': 'bytes',
}
missing = sorted(enumerated - set(kind))
extra = sorted(set(kind) - enumerated)
assert not missing, f'HadoString *-returning functions without a classification: {missing}'
assert not extra, f'classified functions that no longer return HadoString *: {extra}'

expression = {
    'hado_string_concat': 'hado_string_concat(make_string("ab"), make_string("あ"))',
    'hado_string_slice': 'hado_string_slice(make_string("aあb"), 1, 2)',
    'hado_string_trim': 'hado_string_trim(make_string(" あ "))',
    'hado_int_to_string': 'hado_int_to_string(-12345)',
    'hado_float_to_string': 'hado_float_to_string(0.1)',
    'hado_char_to_string': 'hado_char_to_string(0x1f600)',
    'hado_bytes_display': 'display_probe()',
    'hado_bytes_of_string': 'hado_bytes_of_string(make_string("あ"))',
    'hado_bytes_to_string_unchecked': 'to_string_probe()',
    'hado_string_join': 'join_probe()',
    'hado_string_split': 'split_probe()',
    'make_piece': 'make_piece(make_string("a,b,c"), 0, 3)',
    'quote_text': 'quote_text(make_string("あ\\n"), \'"\')',
    'hado_quote_string': 'hado_quote_string(make_string("a\\tb"))',
    'hado_quote_char': 'quote_char_probe()',
    'hado_canonical_float': 'canonical_float_probe()',
    'bytes_from_c_string': 'bytes_from_c_string("x")',
    'hado_argv_get': 'hado_argv_get(0)',
    'hado_env_get': 'hado_env_get(make_string("PATH"))',
    'hado_cwd': 'hado_cwd(0)',
    'hado_io_take_bytes': 'take_bytes_probe()',
    'hado_io_dir_get': 'dir_get_probe()',
    'hado_bytes_alloc': 'hado_bytes_alloc(4)',
    'hado_bytes_fill_unchecked': 'hado_bytes_fill_unchecked(hado_bytes_alloc(4), 0, 65)',
    'grow_io_bytes': 'grow_io_bytes(hado_bytes_alloc(4), 8)',
}
assert set(expression) == set(kind), 'every classified function needs a probe'

body = []
for name in sorted(kind):
    call = expression[name]
    if kind[name] == 'array':
        body.append(f'  assert_string_array({call});')
    elif kind[name] == 'string':
        body.append(f'  assert_string({call});')
    else:
        body.append(f'  assert_bytes({call});')

probe = f'''#include "hado_runtime.c"
#include <assert.h>
#include <string.h>

size_t hado_format_float(double value, unsigned char *bytes, size_t capacity);
size_t hado_format_float(double value, unsigned char *bytes, size_t capacity) {{
  int written = snprintf((char *)bytes, capacity, "%.17g", value);
  if (written < 0 || (size_t)written >= capacity) return 0;
  return (size_t)written;
}}
#include "{root}/crates/hado/runtime_print.c"

static HadoString *make_string(const char *text) {{
  size_t length = strlen(text);
  HadoString *value = hado_alloc(sizeof(HadoString) + length);
  value->length = length;
  value->scalars = hado_scalar_count((const unsigned char *)text, length);
  memcpy(value->bytes, text, length);
  return value;
}}

static HadoString *make_bytes(const char *text) {{
  size_t length = strlen(text);
  HadoString *value = hado_bytes_alloc((int64_t)length);
  memcpy(value->bytes, text, length);
  return value;
}}

static void assert_string(HadoString *value) {{
  assert(value->scalars == hado_scalar_count(value->bytes, value->length));
  hado_byte_scans = 0;
  assert(hado_string_length(value) == (int64_t)value->scalars);
  assert(hado_byte_scans == 0);
}}

static void assert_bytes(HadoString *value) {{
  assert(value->scalars == HADO_SCALARS_UNKNOWN);
}}

static void assert_string_array(HadoString *value) {{
  HadoArray *array = (HadoArray *)value;
  for (int64_t i = 0; i < array->count; ++i) {{
    assert_string((HadoString *)(uintptr_t)array->slots[i]);
  }}
}}

static HadoString *display_probe(void) {{ return hado_bytes_display(make_bytes("hi")); }}

static HadoString *to_string_probe(void) {{
  HadoString *bytes = hado_bytes_alloc(3);
  bytes->bytes[0] = 0xe3;
  bytes->bytes[1] = 0x81;
  bytes->bytes[2] = 0x82;
  return hado_bytes_to_string_unchecked(bytes);
}}

static HadoString *join_probe(void) {{
  HadoArray *values = hado_array_alloc(3);
  values->slots[0] = (uint64_t)(uintptr_t)make_string("a");
  values->slots[1] = (uint64_t)(uintptr_t)make_string("b");
  values->slots[2] = (uint64_t)(uintptr_t)make_string("c");
  return hado_string_join(values, make_string(","));
}}

static HadoString *split_probe(void) {{
  return hado_string_split(make_string("a,b,c"), make_string(","));
}}

static HadoString *quote_char_probe(void) {{ return hado_quote_char('\\n'); }}

static HadoString *canonical_float_probe(void) {{ return hado_canonical_float(0.1); }}

static HadoString *take_bytes_probe(void) {{
  io_bytes = make_bytes("payload");
  return hado_io_take_bytes(0);
}}

static HadoString *dir_get_probe(void) {{
  static HadoString *names[1];
  names[0] = make_bytes("entry");
  io_names = names;
  io_name_count = 1;
  return hado_io_dir_get(0);
}}

int32_t hado_entry(void) {{
{chr(10).join(body)}
  return 0;
}}
'''
source = workdir / 'abi_probe.c'
source.write_text(probe)
compile = subprocess.run(
    ['cc', '-std=c11', '-Wall', '-Werror', '-I', str(root / 'runtime'),
     str(source), '-lgc', '-o', str(workdir / 'abi_probe')],
    capture_output=True)
assert compile.returncode == 0, compile.stderr.decode()[:1500]
done = subprocess.run([str(workdir / 'abi_probe'), 'argument'], capture_output=True)
assert done.returncode == 0, done.stderr.decode()[:800]
print(f'abi: {len(kind)} HadoString *-returning functions classified and measured '
      '(string scalar counts, bytes unknown marker, split String Array)')
PY
}

# --- e: split（ASCII の行）が行数に比例すること ---
perf_split_ascii() {
    python3 - "$workdir" "$hado_abs" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import assert_linear, build, medians

root = Path(sys.argv[1]) / 'e'
root.mkdir(parents=True, exist_ok=True)
control = ('def main =\n'
           '  IoDefault.read_stdin as bytes ->\n'
           '  Bytes.to_string bytes as text ->\n'
           '  String.length text |> Int.to_string |> IoDefault.log\n')
target = ('def main =\n'
          '  IoDefault.read_stdin as bytes ->\n'
          '  Bytes.to_string bytes |> String.split "\\n" as values ->\n'
          '  values |> List.length |> Int.to_string |> IoDefault.log\n')


def lines(count, width):
    return (('a' * width + '\n') * (count - 1) + 'a' * width).encode()


control_binary = build(root, 'control', control)
target_binary = build(root, 'target', target)
_, small_total, small = medians(control_binary, target_binary, lines(1000000, 64))
_, large_total, large = medians(control_binary, target_binary, lines(2000000, 64))
assert_linear('split ascii lines', small_total, small, large_total, large)
PY
}

# --- f: split（日本語の行）が行数に比例すること ---
perf_split_multibyte() {
    python3 - "$workdir" "$hado_abs" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import assert_linear, build, medians

root = Path(sys.argv[1]) / 'f'
root.mkdir(parents=True, exist_ok=True)
control = ('def main =\n'
           '  IoDefault.read_stdin as bytes ->\n'
           '  Bytes.to_string bytes as text ->\n'
           '  String.length text |> Int.to_string |> IoDefault.log\n')
target = ('def main =\n'
          '  IoDefault.read_stdin as bytes ->\n'
          '  Bytes.to_string bytes |> String.split "、" as values ->\n'
          '  values |> List.length |> Int.to_string |> IoDefault.log\n')


def lines(count, width):
    return ('、'.join('あ' * width for _ in range(count))).encode()


control_binary = build(root, 'control', control)
target_binary = build(root, 'target', target)
_, small_total, small = medians(control_binary, target_binary, lines(400000, 32))
_, large_total, large = medians(control_binary, target_binary, lines(800000, 32))
assert_linear('split multibyte lines', small_total, small, large_total, large)
PY
}

# --- g / h: 区切りの長さを入力と一緒に増やす対照（候補位置ごとの照合を排除）---
perf_split_growing() {
    local subject="$1" separator_tail="$2" small="$3" large="$4" label="$5"
    python3 - "$workdir" "$hado_abs" "$subject" "$separator_tail" "$small" "$large" "$label" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import assert_linear, build, medians

subject, tail, small, large, label = sys.argv[3], sys.argv[4], int(sys.argv[5]), int(sys.argv[6]), sys.argv[7]
root = Path(sys.argv[1]) / label
root.mkdir(parents=True, exist_ok=True)
control = ('def main =\n'
           '  IoDefault.read_stdin as bytes ->\n'
           '  Bytes.to_string bytes as text ->\n'
           '  String.length text |> Int.to_string |> IoDefault.log\n')


def target_source(width):
    # 区切りは本文と同じ文字の連続のあとに tail を 1 つ。長さは入力長に比例する。
    # 入力の末尾がその区切り 1 回だけになるので、slice で取り出して split に渡す。
    return ('def main =\n'
            '  IoDefault.read_stdin as bytes ->\n'
            '  Bytes.to_string bytes as text ->\n'
            '  String.length text as total ->\n'
            f'  String.slice text (total - {width // 2}) total as separator ->\n'
            '  String.split text separator |> List.length |> Int.to_string |> IoDefault.log\n')


def input_bytes(n):
    separator = subject * (n // 2 - 1) + tail
    return (subject * n + separator).encode()


control_binary = build(root, 'control', control)
small_stdin = input_bytes(small)
large_stdin = input_bytes(large)
small_target = build(root, 'target-small', target_source(small))
large_target = build(root, 'target-large', target_source(large))
small_control, small_total, small = medians(control_binary, small_target, small_stdin)
large_control, large_total, large = medians(control_binary, large_target, large_stdin)
assert_linear(label, small_total, small, large_total, large)
PY
}

# --- i: join が要素数に比例すること ---
perf_join() {
    python3 - "$workdir" "$hado_abs" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import assert_linear, build, medians

root = Path(sys.argv[1]) / 'i'
root.mkdir(parents=True, exist_ok=True)
# 要素は 800 バイトの固定文字列 1 本を共有する。Int.to_string を要素にすると
# 要素の構築（整数の整形）が join のコピーより重くなり、引いた差が測定の
# 分解能に埋もれる。対照は同じリストを作って List.length を取るだけである。
element = 'x' * 800
build_prefix = f'def rec build n acc => if n == 0 => acc | build (n - 1) ["{element}", ...acc]\n'
control_source = (lambda count: build_prefix + 'def main =\n'
                  f'  build {count} [] as values ->\n'
                  '  values |> List.length |> Int.to_string |> IoDefault.log\n')
target_source = (lambda count: build_prefix + 'def main =\n'
                 f'  build {count} [] as values ->\n'
                 '  String.join values "," |> String.length |> Int.to_string |> IoDefault.log\n')
small_control_binary = build(root, 'control-small', control_source(300000))
large_control_binary = build(root, 'control-large', control_source(600000))
small_target_binary = build(root, 'target-small', target_source(300000))
large_target_binary = build(root, 'target-large', target_source(600000))
_, small_total, small = medians(small_control_binary, small_target_binary)
_, large_total, large = medians(large_control_binary, large_target_binary)
assert_linear('join', small_total, small, large_total, large)
PY
}

# --- j: 判定器そのものの較正（対照を引くことが比の判定に効く）---
control_probe() {
    python3 - "$workdir" <<'PY'
import sys

sys.path.insert(0, sys.argv[1])
from harness import RATIO, ratio_verdict

# issue の例: 線形の構築が 1 s と 2 s、join が 0.1 s と 0.4 s なら、全体の比は
# 2.18 で通り、引いた比は 4 で落ちる。対照を取らない門番はここで通ってしまう。
assert (2.4 / 1.1) <= RATIO
assert ratio_verdict(1.1, 1.0, 2.4, 2.0) is False
assert ratio_verdict(1.1, 1.0, 2.2, 2.0) is True
assert ratio_verdict(1.1, 1.0, 2.0, 1.0) is False
print('control: the control-subtracted ratio rejects the issue example that the total ratio accepts')
PY
}

# --- k: R1 の完了条件のうち Char / Float の長さ（char-and-float-conversions が引き取る）---
# R1 には Char module と正準形の Float.to_string が無く、この 2 つを測れなかった。
# 上の c は runtime の単体テストなので、長さそのものはここで測る。
conversion_lengths() {
    python3 - "$workdir" "$hado_abs" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import build, expect

root = Path(sys.argv[1]) / 'k'
root.mkdir(parents=True, exist_ok=True)
expect(build(root, 'float_length', 'def main = Int.to_string (String.length (Float.to_string 0.1))'), b'3\n')
expect(build(root, 'char_length', "def main = Int.to_string (String.length (Char.to_string 'あ'))"), b'1\n')
print('lengths: Float.to_string 0.1 is 3 scalars and Char.to_string あ is 1 scalar')
PY
}

check 'a semantics: split and join equations and EmptySeparator' semantics
check 'b length: String.length scans no bytes and does not slow with length' length_probe
check 'c layout: runtime unit tests after the HadoString header migration' runtime_tests
check 'd abi: every HadoString *-returning function classified and measured' abi_probe
check 'e perf: split on ASCII lines is linear' perf_split_ascii
check 'f perf: split on Japanese lines is linear' perf_split_multibyte
check 'g perf: split with a growing ASCII separator is linear' \
    perf_split_growing a b 20000000 40000000 'g'
check 'h perf: split with a growing Japanese separator is linear' \
    perf_split_growing あ い 7000000 14000000 'h'
check 'i perf: join is linear in the element count' perf_join
check 'j control: the subtracted ratio judge rejects the issue example' control_probe
check 'k lengths: R1 leftovers Float.to_string 0.1 and Char.to_string あ' conversion_lengths

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
