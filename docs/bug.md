# 既知の不具合

修正していない不具合を 1 件 1 節で、検出順に記録する。直したら節を消し、経緯は git 履歴に残す。仕様の側に穴があるものは「仕様の状態」に書く。

## 未使用の効果に E513 が出ない（R32、裁定 #12）

§40・§38 は入口検査で未使用の効果を E513 とするが、次は診断なしで終了0になる。

リポジトリルートで実行する。

```bash
probe=$(mktemp -d)
trap 'rm -rf "$probe"' EXIT
cat >"$probe/main.hd" <<'HD'
def main = 0
effect Unused
reply unused: Int
HD
status=0
RUSTC_WRAPPER= CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-target} cargo run --offline -q -- check "$probe/main.hd" >"$probe/out" 2>"$probe/err" || status=$?
printf 'status=%s\nstdout_bytes=%s\nstderr_bytes=%s\n' "$status" "$(wc -c <"$probe/out")" "$(wc -c <"$probe/err")"
cat "$probe/out" "$probe/err"
```

実測した出力（診断は無い）:

```text
status=0
stdout_bytes=0
stderr_bytes=0
```

## 効果行付きの関数値を返す member の build が失敗する（R33、裁定 #9）

次は `check` を通るが、`build` は単相化で失敗する。

リポジトリルートで実行する。

```bash
probe=$(mktemp -d)
trap 'rm -rf "$probe"' EXIT
cat >"$probe/main.hd" <<'HD'
def apply f x => f x
def main = apply (M.make "x") "7"
module M
def make(s: String): Fn(String => String {Error}) Pure
implement M as M
def make s => fn(t => String.concat s (Int.to_string (Int.parse t)))
HD
RUSTC_WRAPPER= CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-target} cargo run --offline -q -- check "$probe/main.hd"
status=0
RUSTC_WRAPPER= CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-target} cargo run --offline -q -- build "$probe/main.hd" >"$probe/out" 2>"$probe/err" || status=$?
printf 'status=%s\nstdout_bytes=%s\n' "$status" "$(wc -c <"$probe/out")"
cat "$probe/out" "$probe/err"
```

実測した出力:

```text
status=1
stdout_bytes=0
specialization failed: Mismatch
```

## 効果行を省いた再帰生成器が E306 になる（R35、裁定 #11）

`%` を使う次の再帰生成器は E306 になるが、戻り型を `Int List {Error}` と明記すれば build できて `5` を返す。

リポジトリルートで実行する（診断の行・列は生成した空行・空白から計算した位置だけを `<anchor>` に置き換え、型変数の番号は `<type>` に揃える）。

```bash
probe=$(mktemp -d)
trap 'rm -rf "$probe"' EXIT
blank=$((1 + RANDOM % 7))
indent=$((1 + RANDOM % 9))
{
    for ((i=0; i<blank; i++)); do printf '\n'; done
    printf '%*s' "$indent" ''
    cat <<'HD'
def rec generate (n: Int): Int List =>
  if n == 0 => []
  | [n % 7, ...generate (n - 1)]
def main = List.length (generate 5)
HD
} >"$probe/main.hd"
status=0
RUSTC_WRAPPER= CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-target} cargo run --offline -q -- check "$probe/main.hd" >"$probe/out" 2>"$probe/err" || status=$?
printf 'status=%s\nstdout_bytes=%s\n' "$status" "$(wc -c <"$probe/out")"
cat "$probe/out"
sed -E "s/^main[.]hd:$((blank+1)):$((indent+1)):/main.hd:<anchor>:/; s/ t[0-9]+ / <type> /" "$probe/err"
```

実測した出力（正規化後）:

```text
status=1
stdout_bytes=0
main.hd:<anchor>: error[E306]: cannot unify <type> with Fn(Int => Int List {Error})
```
