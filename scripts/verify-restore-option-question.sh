#!/usr/bin/env bash
# restore-option-question: 人の認めた `as p? ->` と E508 が戻っていることを測る。
# 中核は「E508 の本文の案内に従うと、また別のエラーになる」欠陥である。
#   a. 本文が勧める綴り（`as p? ->`、束縛を使わない形は `as _? ->`）を名指しし、
#      `e ->` を勧めておらず、本文が出す正準形と勧められた綴りが実際に受理される。
#   b. 本文を古い `->` の案内に戻す変異で赤になる。
# 期待値は本文から読み、ファイル名や行を書き込まない。
#
# tier: core
# round: restore-option-question
set -uo pipefail
cd "$(dirname "$0")/.."

pass=0
total=2
ok() { echo "OK: $1"; pass=$((pass + 1)); }
ng() { echo "NG: $1 ($2)"; }

work=$(mktemp -d)
source_file=crates/hado/src/check.rs
backup=$(mktemp)
cp "$source_file" "$backup"
cleanup() {
    cp "$backup" "$source_file"
    rm -f "$backup"
    rm -rf "$work"
}
trap cleanup EXIT

cargo build --offline -q --bin hado || exit 1
target_dir=$(cd "${CARGO_TARGET_DIR:-target}" && pwd)
hado_bin="$target_dir/debug/hado"

# --- 本文の案内どおりに書けるか（受理されるか）を測る ---
advice() {
    python3 - "$hado_bin" "$work" <<'PY'
import re
import subprocess
import sys
from pathlib import Path

hado, root = sys.argv[1], Path(sys.argv[2])
head = 'def find (id: Int): Int Option Pure => if id > 0 => Some(id) | None\n'


def run(name, source):
    directory = root / name
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / 'main.hd'
    path.write_text(source)
    return subprocess.run([hado, 'check', str(path)], capture_output=True, text=True)


def e508_message(result, label):
    match = re.search(r'error\[E508\]: (.*)', result.stderr)
    if result.returncode != 1 or result.stdout or not match:
        sys.exit(f'{label}: expected one E508 and exit 1, got exit={result.returncode} stdout={result.stdout!r} stderr={result.stderr!r}')
    return match.group(1)


def advice_only(message):
    # 正準形の欄より前が「どう書くか」の案内である。
    return message.split('; canonical form:')[0]


# 1. 束縛を使う形: 本文の案内と、本文が出す正準形の両方が受理される。
used = head + 'def g (id: Int): Int Option Pure =>\n  if find id | Some(user) => Some(user + 1) | None => None\ndef main = g 1\n'
message = e508_message(run('used', used), 'used')
advice = advice_only(message)
if '`as p? ->`' not in advice or '`as _? ->`' not in advice:
    sys.exit(f'used: the advice does not name both spellings: {message!r}')
if '`->`' in advice:
    sys.exit(f'used: the advice recommends the bare discard `->`: {message!r}')
canonical = message.split('canonical form: ')[-1].strip()
if canonical == message or not canonical:
    sys.exit(f'used: no canonical form: {message!r}')
rewritten = head + f'def g (id: Int): Int Option Pure => {canonical}\ndef main = g 1\n'
back = run('used_canonical', rewritten)
if back.returncode != 0 or back.stdout or back.stderr:
    sys.exit(f'used: the canonical form is not accepted: {canonical!r} -> {back.stderr!r}')

# 2. 束縛を使わない形: 本文は `_` の綴りを案内し、その綴りが受理される。
bindless = head + 'def g (id: Int): Int Option Pure =>\n  if find id | Some(_) => Some(1) | None => None\ndef main = g 1\n'
message = e508_message(run('bindless', bindless), 'bindless')
advice = advice_only(message)
if '`as _? ->`' not in advice or '`->`' in advice:
    sys.exit(f'bindless: the advice is wrong: {message!r}')
discard = head + 'def g (id: Int): Int Option Pure => find id as _? -> Some(1)\ndef main = g 1\n'
back = run('bindless_discard', discard)
if back.returncode != 0 or back.stdout or back.stderr:
    sys.exit(f'bindless: the advised spelling is not accepted: {back.stderr!r}')
print('advice: the spellings the E508 message names are accepted')
PY
}

# --- a: 本文の案内どおりに書ける ---
if out=$(advice 2>&1); then
    ok "a the E508 message names \`as p? ->\` / \`as _? ->\` and both are accepted"
else
    printf '%s\n' "$out" | tail -5
    ng "a the E508 message names \`as p? ->\` / \`as _? ->\` and both are accepted" "$(printf '%s\n' "$out" | tail -1)"
fi

# --- b: 本文を古い `->` の案内に戻す変異で赤になる ---
if MUT_OLD='use `as p? ->`, or `as _? ->` when the arm binds nothing' MUT_NEW='use `as p? ->` or `->`' \
    python3 - "$source_file" <<'PY'
import os
import sys

path = sys.argv[1]
old, new = os.environ['MUT_OLD'], os.environ['MUT_NEW']
text = open(path, encoding='utf-8').read()
if text.count(old) != 1:
    sys.exit('anchor is not unique')
open(path, 'w', encoding='utf-8').write(text.replace(old, new))
PY
then
    if cargo build --offline -q --bin hado && ! advice >/dev/null 2>&1; then
        ok "b the mutation that makes the message recommend \`->\` is caught"
    else
        ng "b the mutation that makes the message recommend \`->\` is caught" "the mutation stayed green"
    fi
else
    ng "b the mutation that makes the message recommend \`->\` is caught" "anchor error"
fi

# 木を元へ戻し、正しい本文でビルドし直す。
cp "$backup" "$source_file"
cargo build --offline -q --bin hado

echo "PASSED: $pass/$total"
[ "$pass" -eq "$total" ]
