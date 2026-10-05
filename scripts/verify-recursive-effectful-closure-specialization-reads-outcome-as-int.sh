#!/bin/bash
# tier: core
# round: recursive-effectful-closure-specialization-reads-outcome-as-int
# 再帰する関数が返す効果付き閉包を `$Outcome` のまま素の値として読まないことを見る。
# 本体を lower している間の再帰呼び出しは効果を落とした論理型で下りるので、その型を
# 目標に分岐の腕を揃えると、実際に返る `$Outcome` を包み直して誤った値を出す。
# 止まるのは許す。誤った値を出すのは許さない。Run from the repository root.
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

ok=0
total=5
pass() { echo "OK: $1 $2"; ok=$((ok + 1)); }
fail() { echo "NG: $1 $2"; }

TMPD="$(mktemp -d)"
trap 'rm -rf "$TMPD"' EXIT

# 検体。`Catch` の重複を避けるため入力根を分ける。
mkdir -p "$TMPD/rec" "$TMPD/plain" "$TMPD/top"
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
cat >"$TMPD/top/main.hd" <<'EOF'
handler Error as Catch
abort raise _ => 0
return value => value
def rec pick n => if n < 1 => 8 / 0 | n + 1
def main = Int.to_string (with Catch -> pick 3)
EOF

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
    fail 2 "recursive-outcome-not-read-as-plain:$?"
fi

# --- check 3: 再帰しない検体も誤った値を返さない ---
if runs_exactly_or_stops "$TMPD/plain/main.hd" "5"; then
    pass 3 "non-recursive-outcome-not-read-as-plain"
else
    fail 3 "non-recursive-outcome-not-read-as-plain:$?"
fi

# --- check 4: 上位の `$Outcome` を返す再帰は今までどおり動く ---
top_out=$(cargo run --offline -q -- run "$TMPD/top/main.hd" 2>"$TMPD/stderr")
if [ "$top_out" = "4" ]; then
    pass 4 "top-level-outcome-recursion-still-runs"
else
    fail 4 "top-level-outcome-recursion-still-runs stdout=[$top_out]"
fi

# --- check 5: 変異。入れ子の `$Outcome` の有無を見る判定を殺すと赤になる ---
cp crates/hado/src/mono.rs "$TMPD/mono.rs.bak"
restore() { cp "$TMPD/mono.rs.bak" crates/hado/src/mono.rs; }
trap 'restore; rm -rf "$TMPD"' EXIT
python3 - "$ROOT/crates/hado/src/mono.rs" <<'PY'
import pathlib, sys
path = pathlib.Path(sys.argv[1])
source = path.read_text()
old = """fn carries_outcome(ty: &Type) -> bool {
    strip_all_outcomes(ty) != *ty
}"""
new = """fn carries_outcome(ty: &Type) -> bool {
    let _ = strip_all_outcomes(ty);
    false
}"""
if old not in source:
    raise SystemExit("mutation site not found")
path.write_text(source.replace(old, new, 1))
PY
mutate_status=$?
if [ "$mutate_status" -ne 0 ]; then
    fail 5 "mutation-site-missing"
elif ! cargo build >"$TMPD/mutate.log" 2>&1; then
    # 変異が壊れたコードだと、検査 2 が赤くなった理由を欠陥と区別できない。
    fail 5 "mutation-build-failed"
elif runs_exactly_or_stops "$TMPD/rec/main.hd" "5"; then
    fail 5 "mutation-not-caught"
else
    pass 5 "mutation-turns-check-2-red"
fi
restore

echo "PASSED: $ok/$total"
[ "$ok" -eq "$total" ]
