#!/bin/bash
# tier: core
# round: pure-closure-in-type-variable-argument-fails-specialization
# 型変数の引数に置いた閉包が、純粋なときも効果を運ぶときも正しい値を返すことを見る。
# 純粋な閉包の宣言側の行に効果が入る（`with` の本体がほかで効果を出さないため）ので、
# `actual_type` は型引数の位置に `$Outcome` を作る。本体が純粋なままだと単一化できず
# `specialization failed: Mismatch` で落ちる。逆に `$Outcome` を載せないと、効果を
# 運ぶ閉包の本体と型が合わない。両方を同時に満たすことを測る。Run from the repository root.
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

ok=0
total=6
pass() { echo "OK: $1 $2"; ok=$((ok + 1)); }
fail() { echo "NG: $1 $2"; }

TMPD="$(mktemp -d)"
trap 'rm -rf "$TMPD"' EXIT

# 検体。`Catch` の重複を避けるため入力根を分ける。
for name in pure-map eff-map pure-opt eff-opt; do
    mkdir -p "$TMPD/$name"
done
cat >"$TMPD/pure-map/main.hd" <<'EOF'
handler Error as Catch
abort raise _ => 0
return value => value
def main =
  with Catch ->
  Map.insert Map.empty 1 fn(x => x) as m ->
  if Map.find m 1 | Some(f) => f 2 + 100 | 99
EOF
cat >"$TMPD/eff-map/main.hd" <<'EOF'
handler Error as Catch
abort raise _ => 0
return value => value
def main =
  with Catch ->
  Map.insert Map.empty 1 fn(x => 8 / x) as m ->
  if Map.find m 1 | Some(f) => f 2 | 99
EOF
cat >"$TMPD/pure-opt/main.hd" <<'EOF'
handler Error as Catch
abort raise _ => 0
return value => value
def main =
  with Catch ->
  Option.unwrap_or None fn(x => x) as f ->
  f 2 + 100
EOF
cat >"$TMPD/eff-opt/main.hd" <<'EOF'
handler Error as Catch
abort raise _ => 0
return value => value
def main =
  with Catch ->
  Option.unwrap_or None fn(x => 8 / x) as f ->
  f 0
EOF

# 生の `Int` の main は値が終了コードになる。期待値ちょうどであること。
runs_exit_code() {
    local file="$1" expected="$2"
    local code
    cargo run --offline -q -- run "$file" >"$TMPD/stdout" 2>"$TMPD/stderr"
    code=$?
    if [ "$code" -eq "$expected" ]; then
        return 0
    fi
    echo "exit=$code expected=$expected stderr=[$(head -1 "$TMPD/stderr")]" >&2
    return 1
}

# --- check 1: cargo build ---
if cargo build >"$TMPD/build.log" 2>&1; then
    pass 1 "cargo-build"
else
    fail 1 "cargo-build exit!=0"
fi

# --- check 2: 純粋な閉包を型変数の引数に置く（Map.insert の V） ---
if runs_exit_code "$TMPD/pure-map/main.hd" 102; then
    pass 2 "pure-closure-in-map-value"
else
    fail 2 "pure-closure-in-map-value"
fi

# --- check 3: 純粋な閉包を型変数の引数に置く（Option.unwrap_or の A） ---
if runs_exit_code "$TMPD/pure-opt/main.hd" 102; then
    pass 3 "pure-closure-in-unwrap-or"
else
    fail 3 "pure-closure-in-unwrap-or"
fi

# --- check 4: 効果を運ぶ閉包を型変数の引数に置く（呼び出しが成功する腕） ---
if runs_exit_code "$TMPD/eff-map/main.hd" 4; then
    pass 4 "effectful-closure-in-map-value"
else
    fail 4 "effectful-closure-in-map-value"
fi

# --- check 5: 効果を運ぶ閉包を型変数の引数に置く（raise が handler に届く腕） ---
if runs_exit_code "$TMPD/eff-opt/main.hd" 0; then
    pass 5 "effectful-closure-in-unwrap-or"
else
    fail 5 "effectful-closure-in-unwrap-or"
fi

# --- check 6: 変異。宣言側の outcome に本体を揃える分岐を殺すと赤になる ---
cp crates/hado/src/mono.rs "$TMPD/mono.rs.bak"
restore() { cp "$TMPD/mono.rs.bak" crates/hado/src/mono.rs; }
trap 'restore; rm -rf "$TMPD"' EXIT
python3 - "$ROOT/crates/hado/src/mono.rs" <<'PY'
import pathlib, sys
path = pathlib.Path(sys.argv[1])
source = path.read_text()
old = """            if is_outcome(&declared_result) {
                body = self.done(body, &declared_result)?;
                declared_result
            } else {
                lowered
            }"""
new = """            let _ = is_outcome(&declared_result);
            lowered"""
if old not in source:
    raise SystemExit("mutation site not found")
path.write_text(source.replace(old, new, 1))
PY
mutate_status=$?
if [ "$mutate_status" -ne 0 ]; then
    fail 6 "mutation-site-missing"
elif ! cargo build >"$TMPD/mutate.log" 2>&1; then
    # 変異が壊れたコードだと、赤の理由を欠陥と区別できない。
    fail 6 "mutation-build-failed"
elif runs_exit_code "$TMPD/pure-map/main.hd" 102; then
    fail 6 "mutation-not-caught"
else
    pass 6 "mutation-turns-check-2-red"
fi
restore

echo "PASSED: $ok/$total"
[ "$ok" -eq "$total" ]
