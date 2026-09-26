#!/usr/bin/env bash
# 後置束縛（postfix-binding）の試作を測る。仕様は変えない。既定の経路は触らない。
# 変換したコーパスを target/postfix-probe/ に置き、J1〜J6 を回す。
# tier: full
# round: postfix-binding-probe
set -uo pipefail
cd "$(dirname "$0")/.."

OUT=target/postfix-probe
BIN=target/debug/hado

checks=()
statuses=()

record() {
    checks+=("$1")
    statuses+=("$2")
    printf '%s: %s\n' "$1" "$2"
}

mkdir -p "$OUT"
rm -rf "$OUT"/*
# 変異の後始末のために、いまのプリンタを控える。
cp crates/hado/src/postfix_print.rs "$OUT/.printer.bak"

# ---------- 変換（print-postfix） ----------
printf '== 変換 ==\n'
for pair in "tests/accept:tests/accept" "tests/reject:tests/reject" "stdlib:stdlib" "tools:tools"; do
    src=${pair%%:*}
    dst=${pair##*:}
    if ! line=$(cargo run --offline -q -- print-postfix "$src" "$OUT/$dst" 2>"$OUT/.skip.$(basename "$src")"); then
        printf 'print-postfix %s: FAILED\n' "$src"
        record "print-postfix:$src" FAIL
    else
        printf '%s: %s\n' "$src" "$line"
    fi
done

if ! cargo build --offline -q; then
    printf 'cargo build: FAILED\n'
    record "build" FAIL
    printf 'PASSED: 0/%d\n' "${#checks[@]}"
    exit 1
fi

# ---------- J1/J2: 往復と冪等 ----------
j1_ok=0
j1_fail=0
j2_ok=0
j2_fail=0
for root in tests/accept tests/reject stdlib tools; do
    count=$(find "$root" -name '*.hd' -type f | wc -l)
    if [ "$count" -eq 0 ]; then
        printf 'J1/J2: %s に .hd が無い\n' "$root"
        record "J1/J2:$root" FAIL
        continue
    fi
    summary=$("$BIN" probe-postfix-roundtrip "$root" 2>&1 | tail -1)
    printf 'J1/J2 %s: %s\n' "$root" "$summary"
    j1_ok=$((j1_ok + $(sed -n 's/.*j1-ok=\([0-9]*\).*/\1/p' <<<"$summary")))
    j1_fail=$((j1_fail + $(sed -n 's/.*j1-fail=\([0-9]*\).*/\1/p' <<<"$summary")))
    j2_ok=$((j2_ok + $(sed -n 's/.*j2-ok=\([0-9]*\).*/\1/p' <<<"$summary")))
    j2_fail=$((j2_fail + $(sed -n 's/.*j2-fail=\([0-9]*\).*/\1/p' <<<"$summary")))
done
if [ "$j1_ok" -gt 0 ] && [ "$j1_fail" -eq 0 ]; then
    record "J1:往復(ok=$j1_ok)" PASS
else
    record "J1:往復(ok=$j1_ok,fail=$j1_fail)" ISSUES
fi
if [ "$j2_ok" -gt 0 ] && [ "$j2_fail" -eq 0 ]; then
    record "J2:冪等(ok=$j2_ok)" PASS
else
    record "J2:冪等(ok=$j2_ok,fail=$j2_fail)" ISSUES
fi

# ---------- J3/J4: 変換した受理の実行と拒否のコード ----------
printf '== J3/J4 ==\n'
J3J4=$(BIN="$BIN" python3 - <<'PY'
import glob, os, re, subprocess, tempfile

bin = os.path.abspath(os.environ['BIN'])
# 受理のプログラムはファイルを書くものがあるので、木の外で走らせる。
work = tempfile.mkdtemp(prefix='postfix-probe-j3-')
out = 'target/postfix-probe'

def header(path, marker):
    for line in open(path, encoding='utf-8'):
        if line.startswith(marker):
            return line[len(marker):].strip()
    return None

fails = []
mains = sorted(glob.glob(out + '/tests/accept/*/main.hd'))
for main in mains:
    text = open(main, encoding='utf-8').read()
    is_error = any(line.startswith('// expect-error:') for line in text.splitlines())
    want = header(main, '// expect-error:' if is_error else '// expect:')
    result = subprocess.run([bin, 'run', '--syntax', 'postfix', os.path.abspath(main)],
                            capture_output=True, text=True, cwd=work)
    if is_error:
        ok = result.returncode != 0 and (want or '') in result.stderr
    else:
        expected = (want + '\n') if want else ''
        ok = result.stdout == expected and result.returncode == 0
    if not ok:
        fails.append((os.path.basename(os.path.dirname(main)), result.returncode,
                      result.stdout[-80:], result.stderr[-120:]))
print('j3 cases=%d fail=%d' % (len(mains), len(fails)))
for name, code, stdout, stderr in fails[:5]:
    print('  j3-fail: %s rc=%s stdout=%r stderr=%r' % (name, code, stdout, stderr))

fails4 = []
cases4 = 0
for main in sorted(glob.glob(out + '/tests/reject/*/main.hd')):
    want = header(main, '// error:')
    if not want:
        continue
    cases4 += 1
    result = subprocess.run([bin, 'check', '--syntax', 'postfix', main],
                            capture_output=True, text=True)
    match = re.search(r'error\[([A-Z0-9]+)\]', result.stderr)
    got = match.group(1) if match else None
    if got != want:
        fails4.append((os.path.basename(os.path.dirname(main)), want, got,
                       result.stderr.strip()[:120]))
print('j4 cases=%d mismatch=%d' % (cases4, len(fails4)))
for name, want, got, stderr in fails4[:5]:
    print('  j4-mismatch: %s want=%s got=%s %s' % (name, want, got, stderr))
PY
)
printf '%s\n' "$J3J4" | tail -12
j3_line=$(grep -m1 '^j3 ' <<<"$J3J4")
j4_line=$(grep -m1 '^j4 ' <<<"$J3J4")
j3_total=$(sed -n 's/.*cases=\([0-9]*\).*/\1/p' <<<"$j3_line")
j3_fail=$(sed -n 's/.*fail=\([0-9]*\).*/\1/p' <<<"$j3_line")
j4_total=$(sed -n 's/.*cases=\([0-9]*\).*/\1/p' <<<"$j4_line")
j4_fail=$(sed -n 's/.*mismatch=\([0-9]*\).*/\1/p' <<<"$j4_line")
if [ -n "$j3_total" ] && [ "$j3_total" -gt 0 ] && [ "$j3_fail" -eq 0 ]; then
    record "J3:受理の実行(cases=$j3_total)" PASS
else
    record "J3:受理の実行(cases=${j3_total:-0},fail=${j3_fail:-?})" ISSUES
fi
if [ -n "$j4_total" ] && [ "$j4_total" -gt 0 ] && [ "$j4_fail" -eq 0 ]; then
    record "J4:拒否のコード(cases=$j4_total)" PASS
else
    record "J4:拒否のコード(cases=${j4_total:-0},mismatch=${j4_fail:-?})" ISSUES
fi

# ---------- J5: 手書きの正例・負例 ----------
printf '== J5 ==\n'
j5_total=0
j5_fail=0
for old in tests/postfix-probe/pairs/*_old.hd; do
    [ -e "$old" ] || continue
    new="${old%_old.hd}_new.hd"
    name=$(basename "$old" _old.hd)
    j5_total=$((j5_total + 1))
    if ! "$BIN" probe-postfix-compare "$old" "$new" >/dev/null 2>&1; then
        printf 'J5 %s: compare mismatch\n' "$name"
        j5_fail=$((j5_fail + 1))
    elif ! "$BIN" probe-postfix "$new" >/dev/null 2>&1; then
        printf 'J5 %s: new has diagnostics\n' "$name"
        j5_fail=$((j5_fail + 1))
    fi
done
for directory in tests/postfix-probe/run/*/; do
    [ -d "$directory" ] || continue
    name=$(basename "$directory")
    j5_total=$((j5_total + 1))
    want=$(sed -n 's|^// run-expect: *||p' "$directory/case.hd" | paste -sd $'\n' -)
    got=$("$BIN" run --syntax postfix "$directory/case.hd" 2>/dev/null)
    if [ "$got" != "$want" ]; then
        printf 'J5 %s: run got=%q want=%q\n' "$name" "$got" "$want"
        j5_fail=$((j5_fail + 1))
    fi
done
for directory in tests/postfix-probe/neg/*/; do
    [ -d "$directory" ] || continue
    name=$(basename "$directory")
    want=$(sed -n 's|^// error: *||p' "$directory/case.hd" | head -1)
    note=$(sed -n 's|^// note: *||p' "$directory/case.hd" | head -1)
    got=$("$BIN" check --syntax postfix "$directory/case.hd" 2>&1 \
        | grep -o 'error\[[A-Z0-9]*\]' | head -1 | sed 's/error\[//;s/\]//')
    if [ -n "$note" ]; then
        printf 'J5 %s: note want=%s got=%s (%s)\n' "$name" "$want" "$got" "$note"
        continue
    fi
    j5_total=$((j5_total + 1))
    if [ "$got" != "$want" ]; then
        printf 'J5 %s: want=%s got=%s\n' "$name" "$want" "$got"
        j5_fail=$((j5_fail + 1))
    fi
done
if [ "$j5_total" -gt 0 ] && [ "$j5_fail" -eq 0 ]; then
    record "J5:手書き(cases=$j5_total)" PASS
else
    record "J5:手書き(cases=$j5_total,fail=$j5_fail)" ISSUES
fi

# ---------- J6: プリンタの変異 ----------
printf '== J6 ==\n'
j6_total=0
j6_fail=0
mutate() {
    local label=$1
    local old=$2
    local new=$3
    cp "$OUT/.printer.bak" crates/hado/src/postfix_print.rs
    if ! python3 - "$old" "$new" <<'PY'
import sys
path = 'crates/hado/src/postfix_print.rs'
old, new = sys.argv[1], sys.argv[2]
text = open(path, encoding='utf-8').read()
if text.count(old) != 1:
    sys.exit('mutation anchor is not unique: ' + old)
open(path, 'w', encoding='utf-8').write(text.replace(old, new))
PY
    then
        printf 'J6 %s: anchor error\n' "$label"
        j6_fail=$((j6_fail + 1))
        return
    fi
    j6_total=$((j6_total + 1))
    if ! cargo build --offline -q 2>/dev/null; then
        printf 'J6 %s: mutation did not compile\n' "$label"
        j6_fail=$((j6_fail + 1))
        return
    fi
    caught=0
    for root in tests/accept stdlib tools tests/reject; do
        if ! "$BIN" probe-postfix-roundtrip "$root" >/dev/null 2>&1; then
            caught=1
            break
        fi
    done
    if [ "$caught" -eq 1 ]; then
        printf 'J6 %s: caught\n' "$label"
    else
        printf 'J6 %s: mutation stayed green\n' "$label"
        j6_fail=$((j6_fail + 1))
    fi
}
mutate "arrow-if-with-parens" 'format!("if {inner}\n{pad}/if")' 'format!("if({inner})")'
mutate "drop-whole-if-parens" '    } else {
        format!("if({inner})")
    }
}' '    } else {
        format!("if {inner}")
    }
}'
mutate "drop-option-question" 'format!(" as {pat}?")' 'format!(" as {pat}")'
cp "$OUT/.printer.bak" crates/hado/src/postfix_print.rs
cargo build --offline -q
if [ "$j6_total" -gt 0 ] && [ "$j6_fail" -eq 0 ]; then
    record "J6:変異(cases=$j6_total)" PASS
else
    record "J6:変異(cases=$j6_total,caught_fail=$j6_fail)" ISSUES
fi

# ---------- 集計 ----------
passed=0
for status in "${statuses[@]}"; do
    [ "$status" = PASS ] && passed=$((passed + 1))
done
printf 'PASSED: %d/%d\n' "$passed" "${#checks[@]}"
[ "$passed" -eq "${#checks[@]}" ]
