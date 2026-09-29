#!/usr/bin/env bash
# tier: full
# round: cranelift-opt-level
# Cranelift の `opt_level` の既定を、issue の完了条件の分岐（3 入力すべて
# 1.1 倍以上かつ比の中央値 2 倍以上のときだけ "speed"）で判定する門番。
#
# full にする理由: 3 入力 × 2 水準の実行ファイルを作り、65 MB の入力を生成して
# warm-up + 5 回 × 6 本を測る。判定が実測の比そのもので、quick / core では
# その費用に見合う変更が入らない（このラウンドの成果物は既定の選択だけ）。
#
# 入力は tests/perf/opt-level/ の追跡ソースをそのまま使う。期待値は判定器が
# ソースと入力から独立に計算し、コンパイラの出力からは採らない。3 本目の
# 入力は string-split-linear の完了条件（scripts/verify-string-split-linear.sh
# 検査 e）と同じ生成手順で再生成し、sha256 とバイト数の錠と照合する。
#
# 2 水準は HADO_OPT_LEVEL（none / speed）で選ぶ。既定（emit::BUILD_OPT_LEVEL・
# emit::RUN_OPT_LEVEL）が判定と一致することも見る。時間は子プロセスの CPU 時間
# （user + sys）の中央値で測る。負荷で待たされても CPU 時間は動かず、対照
# （同じ実行ファイルの他水準）と同条件で比べられる。
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

export OPT_ROOT="$root"
export OPT_HADO="$hado_abs"
export OPT_WORK="$workdir"

cat >"$workdir/harness.py" <<'PY'
"""cranelift-opt-level の判定器。ソースと入力から期待値を計算し、2 水準を測る。"""
import ast
import hashlib
import json
import os
import re
import resource
import statistics
import subprocess
import sys
from pathlib import Path

ROOT = Path(os.environ['OPT_ROOT'])
HADO = Path(os.environ['OPT_HADO'])
WORK = Path(os.environ['OPT_WORK'])
RUNS = 5
LOOPS = 10000000
LOWER = 1.1
MEDIAN = 2.0
SPLIT_FLOOR = 0.2
LEVELS = ('none', 'speed')
PROGRAMS = {
    'string_compare': ROOT / 'tests/perf/opt-level/string_compare/main.hd',
    'tail_recursion': ROOT / 'tests/perf/opt-level/tail_recursion/main.hd',
    'split': ROOT / 'tests/perf/opt-level/split/main.hd',
}
GENERATOR = ROOT / 'tests/perf/opt-level/split_input.py'
INPUT_LOCK = ROOT / 'tests/perf/opt-level/split_input.sha256'
INPUT = WORK / 'split_input.bin'


def state_path():
    return WORK / 'state.json'


def load_state():
    if state_path().is_file():
        return json.loads(state_path().read_text())
    return {}


def save_state(state):
    state_path().write_text(json.dumps(state, indent=2, sort_keys=True))


def decide(ratios):
    """issue の分岐。3 本すべて 1.1 倍以上かつ比の中央値が 2 倍以上のときだけ speed。"""
    values = list(ratios.values())
    if min(values) >= LOWER and statistics.median(values) >= MEDIAN:
        return 'speed'
    return 'none'


def self_call_arm(text, name):
    """自己呼び出しが腕の全体にある（末尾位置）ことを確かめ、その腕を返す。"""
    match = re.search(rf'(?m)^\s*\|\s*{re.escape(name)}\b.*$', text)
    assert match, f'{name}: the self call is not a whole branch arm (not tail recursive)'
    return match.group(0)


def loop_call(text, name):
    match = re.search(rf'{re.escape(name)}\s+(\d+)\s+(\d+)', text)
    assert match, f'{name}: the loop call with a literal count and base is missing'
    count, base = int(match.group(1)), int(match.group(2))
    assert count == LOOPS, f'{name}: iterations {count} is below {LOOPS}'
    return count, base


def string_compare_expected(text):
    names = re.findall(r'^def rec (\w+)', text, re.M)
    assert len(names) == 1, 'string_compare: expected exactly one def rec'
    name = names[0]
    self_call_arm(text, name)
    operands = re.findall(r'"([^"]*)"\s*==\s*"([^"]*)"', text)
    assert len(operands) == 1, 'string_compare: expected exactly one string == string'
    left, right = operands[0]
    count, base = loop_call(text, name)
    step = re.search(r'def bump \w+ \w+ => if \w+ => \w+ \+ (\d+)', text)
    assert step, 'string_compare: the flag is not accumulated by a literal step'
    step = int(step.group(1))
    # 被演算子を独立に == する。真なら基底 + 反復数 × 加算幅、偽なら基底。
    return base + count * step if left == right else base


def tail_recursion_expected(text):
    names = re.findall(r'^def rec (\w+)', text, re.M)
    assert len(names) == 1, 'tail_recursion: expected exactly one def rec'
    name = names[0]
    arm = self_call_arm(text, name)
    count, base = loop_call(text, name)
    step = re.search(r'acc \+ (\d+)', arm)
    assert step, 'tail_recursion: the arm does not add a literal to acc'
    step = int(step.group(1))
    return base + count * step


def split_separator(text):
    assert 'IoDefault.read_stdin' in text, 'split: the source does not read stdin'
    match = re.search(r'String\.split\s+"((?:[^"\\]|\\.)*)"', text)
    assert match, 'split: no String.split with a literal separator'
    separator = ast.literal_eval('"' + match.group(1) + '"').encode('utf-8')
    assert separator, 'split: the separator is empty'
    return separator


def sources():
    state = load_state()
    expected = {}
    for name, path in PROGRAMS.items():
        assert path.is_file(), f'the tracked source is missing: {path}'
        text = path.read_text()
        if name == 'string_compare':
            expected[name] = str(string_compare_expected(text))
        elif name == 'tail_recursion':
            expected[name] = str(tail_recursion_expected(text))
        else:
            state['separator_hex'] = split_separator(text).hex()
    state['expected'] = expected
    save_state(state)
    print('sources: two loops at 10000000 iterations, string == operands and the '
          'tail arm read from the tracked sources')
    print(f'sources: expected string_compare={expected["string_compare"]} '
          f'tail_recursion={expected["tail_recursion"]}')


def check_lock(data):
    """生成した入力が錠（sha256 とバイト数）と一致することを確かめる。"""
    digest = hashlib.sha256(data).hexdigest()
    lines = [line.strip() for line in INPUT_LOCK.read_text().splitlines()
             if line.strip() and not line.startswith('#')]
    assert len(lines) == 2, 'split_input.sha256: expected a digest and a byte count'
    assert digest == lines[0], f'the regenerated input is not byte-identical ({digest})'
    assert len(data) == int(lines[1]), f'the regenerated input is {len(data)} bytes'


def input_probe():
    state = load_state()
    subprocess.run([sys.executable, str(GENERATOR), str(INPUT)], check=True)
    data = INPUT.read_bytes()
    check_lock(data)
    separator = bytes.fromhex(state['separator_hex'])
    expected = data.count(separator) + 1
    state['expected']['split'] = str(expected)
    state['input_bytes'] = len(data)
    save_state(state)
    print(f'input: regenerated {len(data)} bytes '
          f'(sha256 {hashlib.sha256(data).hexdigest()[:12]}...), '
          f'{expected} lines for the split separator')


def build(program, level, output, env):
    done = subprocess.run([str(HADO), 'build', '-o', str(output), str(PROGRAMS[program])],
                          capture_output=True, env=env)
    assert done.returncode == 0, f'{program}/{level}: build failed: {done.stderr.decode()[:400]}'


def observe():
    state = load_state()
    expected = state['expected']
    data = INPUT.read_bytes()
    binaries = {}
    for program in PROGRAMS:
        for level in LEVELS:
            output = WORK / f'{program}-{level}'
            env = dict(os.environ, HADO_OPT_LEVEL=level)
            build(program, level, output, env)
            stdin = data if program == 'split' else b''
            done = subprocess.run([str(output)], input=stdin, capture_output=True, timeout=120)
            assert done.returncode == 0, f'{program}/{level}: exit {done.returncode}'
            got = done.stdout.decode().strip()
            assert got == expected[program], \
                f'{program}/{level}: got {got!r}, want {expected[program]!r}'
            binaries[f'{program}-{level}'] = str(output)
    digests = {
        program: {
            level: hashlib.sha256(Path(binaries[f'{program}-{level}']).read_bytes()).hexdigest()
            for level in LEVELS
        }
        for program in PROGRAMS
    }
    different = sorted(program for program in PROGRAMS
                       if digests[program]['none'] != digests[program]['speed'])
    assert different, 'HADO_OPT_LEVEL had no effect: both levels produced the same executable'
    state['binaries'] = binaries
    save_state(state)
    print(f'observe: {len(PROGRAMS) * len(LEVELS)} executables match the independent '
          f'expectations at both levels')
    print(f'observe: the two levels produce different machine code for {", ".join(different)}')


def cpu_seconds():
    usage = resource.getrusage(resource.RUSAGE_CHILDREN)
    return usage.ru_utime + usage.ru_stime


def median_run(binary, stdin):
    def once():
        start = cpu_seconds()
        subprocess.run([binary], input=stdin, capture_output=True, timeout=120, check=True)
        return cpu_seconds() - start

    once()
    return statistics.median(once() for _ in range(RUNS))


def measure():
    state = load_state()
    binaries = state['binaries']
    data = INPUT.read_bytes()
    medians = {}
    ratios = {}
    for program in PROGRAMS:
        stdin = data if program == 'split' else b''
        medians[program] = {level: median_run(binaries[f'{program}-{level}'], stdin)
                            for level in LEVELS}
        ratios[program] = (medians[program]['none'] / medians[program]['speed']
                           if medians[program]['speed'] > 0 else float('inf'))
    floor = min(medians['split'].values())
    assert floor >= SPLIT_FLOOR, \
        f'split: smaller median {floor:.3f}s is below the {SPLIT_FLOOR:.3f}s floor'
    verdict = decide(ratios)
    binding = min(ratios, key=ratios.get)
    for program in PROGRAMS:
        print(f'measure {program}: none={medians[program]["none"]:.4f}s '
              f'speed={medians[program]["speed"]:.4f}s ratio={ratios[program]:.3f}')
    print(f'judge: median={statistics.median(ratios.values()):.3f} '
          f'lower-bound input={binding} ratio={ratios[binding]:.3f} verdict={verdict}')
    state['ratios'] = ratios
    state['medians'] = medians
    state['verdict'] = verdict
    state['binding'] = binding
    save_state(state)


def default():
    state = load_state()
    verdict = state['verdict']
    emit = (ROOT / 'crates/hado/src/emit.rs').read_text()
    build_const = re.search(r'pub const BUILD_OPT_LEVEL: &str = "(\w+)";', emit)
    run_const = re.search(r'pub const RUN_OPT_LEVEL: &str = "(\w+)";', emit)
    assert build_const and run_const, 'emit.rs: the opt_level default constants are missing'
    build_const, run_const = build_const.group(1), run_const.group(1)
    clean = {key: value for key, value in os.environ.items() if key != 'HADO_OPT_LEVEL'}
    if verdict == 'none':
        assert build_const == 'none', f'verdict none but BUILD_OPT_LEVEL is {build_const}'
        assert run_const == 'none', f'verdict none but RUN_OPT_LEVEL is {run_const}'
        output = WORK / 'default'
        build('string_compare', 'default', output, clean)
        got = hashlib.sha256(output.read_bytes()).hexdigest()
        want = hashlib.sha256(Path(state['binaries']['string_compare-none']).read_bytes()).hexdigest()
        assert got == want, 'the default build is not the none build'
        print(f'default: verdict=none, BUILD_OPT_LEVEL={build_const}, RUN_OPT_LEVEL={run_const}, '
              'the default build equals the none build')
        return
    assert build_const == 'speed', f'verdict speed but BUILD_OPT_LEVEL is {build_const}'
    ratios = {}
    for program in PROGRAMS:
        output = WORK / f'default-{program}'
        build(program, 'default', output, clean)
        stdin = INPUT.read_bytes() if program == 'split' else b''
        medians = {level: median_run(state['binaries'][f'{program}-{level}'], stdin)
                   for level in LEVELS}
        ratios[program] = medians['none'] / medians['speed']
    assert decide(ratios) == 'speed', f'the default build does not reproduce speed: {ratios}'
    end_to_end = {}
    for program, path in PROGRAMS.items():
        stdin = INPUT.read_bytes() if program == 'split' else b''
        times = {}
        for level, env in (('none', dict(os.environ, HADO_OPT_LEVEL='none')), ('speed', clean)):
            def once():
                start = cpu_seconds()
                subprocess.run([str(HADO), 'run', str(path)], input=stdin,
                               capture_output=True, timeout=300, env=env, check=True)
                return cpu_seconds() - start
            once()
            times[level] = statistics.median(once() for _ in range(RUNS))
        end_to_end[program] = times['none'] / times['speed']
    run_ok = all(ratio >= LOWER for ratio in end_to_end.values())
    want_run = 'speed' if run_ok else 'none'
    assert run_const == want_run, \
        f'end-to-end says run={want_run} but RUN_OPT_LEVEL is {run_const} ({end_to_end})'
    print(f'default: verdict=speed, BUILD_OPT_LEVEL={build_const}, RUN_OPT_LEVEL={run_const}, '
          f'end-to-end ratios {end_to_end}')


def control():
    bad = WORK / 'bad.hd'
    bad.write_text('def main = missing_name\n')
    probes = [PROGRAMS['tail_recursion'], bad]
    for probe in probes:
        for extra in (['--print-type=main'], []):
            seen = {}
            for level in LEVELS:
                env = dict(os.environ, HADO_OPT_LEVEL=level)
                done = subprocess.run([str(HADO), 'check', *extra, str(probe)],
                                      capture_output=True, env=env)
                seen[level] = (done.returncode, done.stdout, done.stderr)
            assert seen['none'] == seen['speed'], \
                f'check differs between levels for {probe.name} {extra}: {seen}'
    print('control: check diagnostics and --print-type are identical at both levels')


def judge():
    cases = [
        ({'a': 3.0, 'b': 3.0, 'c': 0.1}, 'none'),
        ({'a': 10.0, 'b': 1.1, 'c': 1.1}, 'none'),
        ({'a': 2.0, 'b': 2.0, 'c': 2.0}, 'speed'),
        ({'a': 1.05, 'b': 5.0, 'c': 5.0}, 'none'),
        ({'a': 1.09, 'b': 3.0, 'c': 3.0}, 'none'),
        ({'a': 1.1, 'b': 1.1, 'c': 2.0}, 'none'),
        ({'a': 1.1, 'b': 2.0, 'c': 3.0}, 'speed'),
    ]
    for ratios, want in cases:
        got = decide(ratios)
        assert got == want, f'decide({ratios}) = {got}, want {want}'
    print(f'judge: {len(cases)} ratio triples classified as the issue requires '
          '(median-only and max-only judges are rejected)')


def raises(call):
    try:
        call()
    except Exception:
        return True
    return False


def mutations():
    """issue が列挙する「通ってはいけない実装」を、判定器の部品が実際に落とすか。"""
    compare = PROGRAMS['string_compare'].read_text()
    tail = PROGRAMS['tail_recursion'].read_text()
    assert raises(lambda: string_compare_expected(compare.replace('10000000', '9999999'))), \
        'a loop below 10000000 iterations is accepted'
    assert raises(lambda: string_compare_expected(compare.replace('"hado" == "hado"', '1 + 1'))), \
        'a source without a string == is accepted'
    assert raises(lambda: string_compare_expected(
        compare.replace('| compare (n - 1)', '| total + compare (n - 1)'))), \
        'a non-tail self call is accepted for the string loop'
    assert raises(lambda: tail_recursion_expected(
        tail.replace('| count (n - 1) (acc + 1)', '| acc + count (n - 1) acc'))), \
        'a non-tail self call is accepted for the tail loop'
    short = (('a' * 64 + '\n') * 999 + 'a' * 64).encode()
    assert raises(lambda: check_lock(short)), 'a shorter input is accepted'
    # 中央値だけ・最大値だけの判定は issue の例で誤る。
    median_only = lambda r: 'speed' if statistics.median(r.values()) >= MEDIAN else 'none'
    max_only = lambda r: 'speed' if max(r.values()) >= MEDIAN else 'none'
    example_a = {'a': 3.0, 'b': 3.0, 'c': 0.1}
    example_b = {'a': 10.0, 'b': 1.1, 'c': 1.1}
    assert median_only(example_a) == 'speed' and decide(example_a) == 'none', \
        'the median-only judge is not distinguished'
    assert max_only(example_b) == 'speed' and decide(example_b) == 'none', \
        'the max-only judge is not distinguished'
    print('mutations: fewer iterations, no string ==, a non-tail call, a shorter input, '
          'and median-only / max-only judges are all rejected')


COMMANDS = {
    'sources': sources,
    'input': input_probe,
    'observe': observe,
    'measure': measure,
    'default': default,
    'control': control,
    'judge': judge,
    'mutations': mutations,
}

if __name__ == '__main__':
    COMMANDS[sys.argv[1]]()
PY

check 'a sources: tracked loops, iterations, == operands and the tail arm' \
    python3 "$workdir/harness.py" sources
check 'b input: regenerated bytes match the string-split-linear lock' \
    python3 "$workdir/harness.py" input
check 'c observe: both levels match the independent expectations' \
    python3 "$workdir/harness.py" observe
check 'd measure: two levels, warm-up + 5 runs, per-input floor and the verdict' \
    python3 "$workdir/harness.py" measure
check 'e default: emit defaults follow the verdict' \
    python3 "$workdir/harness.py" default
check 'f control: check diagnostics and --print-type do not depend on the level' \
    python3 "$workdir/harness.py" control
check 'g judge: the ratio decision is calibrated on the issue examples' \
    python3 "$workdir/harness.py" judge
check 'h mutations: the listed bad implementations are rejected' \
    python3 "$workdir/harness.py" mutations

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
