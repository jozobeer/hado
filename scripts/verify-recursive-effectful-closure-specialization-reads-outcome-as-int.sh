#!/bin/bash
# tier: core
# round: recursive-effectful-closure-specialization-reads-outcome-as-int
# 再帰する関数が返す効果付き閉包を `$Outcome` のまま素の値として読まないことを見る。
# 本体を lower している間の再帰呼び出しは効果を落とした論理型で下りるので、その型を
# 目標に分岐の腕を揃えると、実際に返る `$Outcome` を包み直して誤った値を出す。
# 止まるのは許す。誤った値を出すのは許さない。相互再帰では、自分を呼ぶ控えが呼んだ先の
# 単相化の本体で積まれるので、控えを呼ばれた単相化の id で区別しないと見逃す。
# Run from the repository root.
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

ok=0
total=9
pass() { echo "OK: $1 $2"; ok=$((ok + 1)); }
fail() { echo "NG: $1 $2"; }

TMPD="$(mktemp -d)"
trap 'rm -rf "$TMPD"' EXIT

# 検体。`Catch` の重複を避けるため入力根を分ける。
for name in rec plain top mutual mut-ok nested-effect top-level-dropped; do
    mkdir -p "$TMPD/$name"
done
cat >"$TMPD/rec/main.hd" <<'EOF'
handler Error as Catch
abort raise _ => 0
return value => value
def rec pick n
| n < 1 => fn(x => 8 / x)
| n < 2 => fn(x => x + 1)
| pick (n - 2)
def main = Int.to_string (with Catch -> pick 3 as f -> f 4)
EOF
sed 's/pick 3/pick 1/' "$TMPD/rec/main.hd" >"$TMPD/plain/main.hd"
# 上位の `$Outcome` を返す再帰。実際に自分を呼ぶ（`count` が `count` を呼ぶ）。
cat >"$TMPD/top/main.hd" <<'EOF'
handler Error as Catch
abort raise _ => 0
return value => value
def rec count n => if n < 1 => 8 / 0 | count (n - 1)
def main = Int.to_string (with Catch -> count 3)
EOF
# 相互再帰。自分を呼ぶ控えが `pock` の本体で積まれる形。
cat >"$TMPD/mutual/main.hd" <<'EOF'
handler Error as Catch
abort raise _ => 0
return value => value
def rec pick n
| n < 1 => fn(x => 8 / x)
| n < 2 => fn(x => x + 1)
| pock (n - 2)
def rec pock n => pick n
def main = Int.to_string (with Catch -> pick 3 as f -> f 4)
EOF
# 相互再帰で、上位の `$Outcome` を返す正当な形。止めてはいけない。
cat >"$TMPD/mut-ok/main.hd" <<'EOF'
handler Error as Catch
abort raise _ => 0
return value => value
def rec f n => if n < 1 => 7 | g (n - 1)
def rec g n => f n
def main = Int.to_string (with Catch -> f 3)
EOF

# 関数自身が top-level の効果を持ち、内側の閉包にも効果の食い違いがある形。
# `$Outcome` の有無だけを見ると、外側の `$Outcome` が両側に付いて覆い隠される。
cat >"$TMPD/nested-effect/main.hd" <<'EOF'
handler Error as Catch
abort raise _ => 0
return value => value
def wrap n g => if n > 100 => fn(x => 8 / x) | g
def rec pick n
| n < 1 => fn(x => 8 / x)
| n < 2 => (8 / n) as k -> fn(x => k / x)
| pick (n - 2) as g -> wrap n g
def main = Int.to_string (with Catch -> pick 3 as f -> f 4)
EOF

# 本体が閉包を返すので `reconcile_outcome` が確定した署名から top-level の
# `$Outcome` を外す形。`logical_argument` は top-level を外さないので、控えにだけ
# 外側の `$Outcome` が付く。build は通り、できた実行ファイルが SIGILL で落ちていた。
cat >"$TMPD/top-level-dropped/main.hd" <<'EOF'
handler Error as Catch
abort raise _ => 0
return value => value
def rec pick n cb
| n < 1 => fn(x => cb x)
| n < 2 => fn(x => x + 1)
| fn x =>
    pick (n - 2) cb as g ->
    g x
  /fn
def main = Int.to_string (with Catch -> pick 3 (fn(x => 8 / x)) as f -> f 4)
EOF

# `build` を通してしまったら、できた実行ファイルが期待値ちょうどを返すこと。
# build が止まるのは許す。
builds_and_runs_exactly_or_stops() {
    local file="$1" expected="$2"
    local out code
    if ! cargo run --offline -q -- build "$file" >"$TMPD/build.log" 2>&1; then
        return 0
    fi
    out=$("${file%.hd}" 2>"$TMPD/exec.log")
    code=$?
    if [ "$code" -eq 0 ] && [ "$out" = "$expected" ]; then
        return 0
    fi
    echo "wrong-value exit=$code out=[$out] expected=[$expected]" >&2
    return 1
}

# 終了コード 0 なら標準出力が期待値ちょうどであること。停止は許す。
# 誤った値を返したら赤。
runs_exactly_or_stops() {
    local file="$1" expected="$2"
    local out code
    out=$(cargo run --offline -q -- run "$file" 2>"$TMPD/stderr")
    code=$?
    if [ "$code" -eq 0 ]; then
        if [ "$out" = "$expected" ]; then
            return 0
        fi
        echo "wrong-value stdout=[$out] expected=[$expected]" >&2
        return 1
    fi
    return 0
}

# 期待値ちょうどであること（停止は許さない）。
runs_exactly() {
    local file="$1" expected="$2"
    local out
    out=$(cargo run --offline -q -- run "$file" 2>"$TMPD/stderr")
    if [ "$out" = "$expected" ]; then
        return 0
    fi
    echo "stdout=[$out] expected=[$expected] stderr=[$(head -1 "$TMPD/stderr")]" >&2
    return 1
}

# --- check 1: cargo build ---
if cargo build >"$TMPD/build.log" 2>&1; then
    pass 1 "cargo-build"
else
    fail 1 "cargo-build exit!=0"
fi

# --- check 2: 再帰する検体が誤った値を返さない ---
if runs_exactly_or_stops "$TMPD/rec/main.hd" "5"; then
    pass 2 "recursive-outcome-not-read-as-plain"
else
    fail 2 "recursive-outcome-not-read-as-plain"
fi

# --- check 3: 再帰しない検体も誤った値を返さない ---
if runs_exactly_or_stops "$TMPD/plain/main.hd" "5"; then
    pass 3 "non-recursive-outcome-not-read-as-plain"
else
    fail 3 "non-recursive-outcome-not-read-as-plain"
fi

# --- check 4: 上位の `$Outcome` を返す再帰は今までどおり動く ---
if runs_exactly "$TMPD/top/main.hd" "0"; then
    pass 4 "top-level-outcome-recursion-still-runs"
else
    fail 4 "top-level-outcome-recursion-still-runs"
fi

# --- check 5: 相互再帰でも誤った値を返さない ---
if runs_exactly_or_stops "$TMPD/mutual/main.hd" "5"; then
    pass 5 "mutual-recursion-outcome-not-read-as-plain"
else
    fail 5 "mutual-recursion-outcome-not-read-as-plain"
fi

# --- check 6: 上位の `$Outcome` を返す相互再帰は今までどおり動く ---
if runs_exactly "$TMPD/mut-ok/main.hd" "7"; then
    pass 6 "top-level-outcome-mutual-recursion-still-runs"
else
    fail 6 "top-level-outcome-mutual-recursion-still-runs"
fi

# --- check 7: top-level の効果と内側の閉包の効果が重なる形でも誤った値を返さない ---
if runs_exactly_or_stops "$TMPD/nested-effect/main.hd" "2"; then
    pass 7 "nested-outcome-under-top-level-outcome-not-read-as-plain"
else
    fail 7 "nested-outcome-under-top-level-outcome-not-read-as-plain"
fi

# --- check 8: 控えにだけ top-level の `$Outcome` が付く形でも誤った値を返さない ---
if builds_and_runs_exactly_or_stops "$TMPD/top-level-dropped/main.hd" "5"; then
    pass 8 "top-level-outcome-dropped-from-the-signature-not-read-as-plain"
else
    fail 8 "top-level-outcome-dropped-from-the-signature-not-read-as-plain"
fi

# --- check 9: 変異。型の食い違いを見る判定を殺すと赤になる ---
cp crates/hado/src/mono.rs "$TMPD/mono.rs.bak"
restore() { cp "$TMPD/mono.rs.bak" crates/hado/src/mono.rs; }
trap 'restore; rm -rf "$TMPD"' EXIT
python3 - "$ROOT/crates/hado/src/mono.rs" <<'PY'
import pathlib, sys
path = pathlib.Path(sys.argv[1])
source = path.read_text()
old = """            strip_outcome(logical_argument(s))
                == strip_outcome(logical_argument(&signature.result))
                && **s != signature.result"""
new = """            let _ = (
                strip_outcome(logical_argument(s)),
                strip_outcome(logical_argument(&signature.result)),
            );
            false"""
if old not in source:
    raise SystemExit("mutation site not found")
path.write_text(source.replace(old, new, 1))
PY
mutate_status=$?
if [ "$mutate_status" -ne 0 ]; then
    fail 9 "mutation-site-missing"
elif ! cargo build >"$TMPD/mutate.log" 2>&1; then
    # 変異が壊れたコードだと、赤の理由を欠陥と区別できない。
    fail 9 "mutation-build-failed"
elif runs_exactly_or_stops "$TMPD/rec/main.hd" "5"; then
    fail 9 "mutation-not-caught"
else
    pass 9 "mutation-turns-check-2-red"
fi
restore

echo "PASSED: $ok/$total"
[ "$ok" -eq "$total" ]
