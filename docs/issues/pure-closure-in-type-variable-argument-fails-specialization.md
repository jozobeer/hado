# 型変数の引数に純粋な閉包を置くと単相化が落ちる（pure-closure-in-type-variable-argument-fails-specialization）

優先度: 高。

## 現象

`with` の下で、純粋な閉包を `Map.insert` の値や `Option.unwrap_or` の代替に置くと、main では正しく動くプログラムが `specialization failed: Mismatch` でビルドできなくなる（退行）。

```
$ cat > target/reg/main.hd <<'EOF'
handler Error as Catch
abort raise _ => 0
return value => value
def main =
  with Catch ->
  Map.insert Map.empty 1 fn(x => x) as m ->
  if Map.find m 1 | Some(f) => f 2 + 100 | 99
EOF
$ ./target/debug/hado run target/reg/main.hd
exit=1
specialization failed: Mismatch
```

main では同じ入力が `102` を返す。

```
$ ./target/debug/hado run target/reg/main.hd
102
$ echo $?
0
```

`Option.unwrap_or None fn(x => x) as f -> f 2 + 100` も同じである。main は `102`、この変更を入れると `specialization failed: Mismatch` になる。

## 原因

`crates/hado/src/mono.rs` の `actual_type` が、型検査器の推論した行をそのまま信じて、型引数の位置の閉包にも `$Outcome` を載せる。ところが閉包の本体は純粋なまま下ろされる（`dump-mono` で `lambda_1: [Int] -> Int`）ので、両者の型が合わなくなる。

担う形は 2 か所に分かれる。定義参照の側（5651 行付近）が `Map.empty` を落とし、構築子の側（3122 行付近）が `None` を落とす。片方だけを戻したバイナリで確かめると、定義参照の塊だけを戻せば `Map.insert` の形は `102`/`103` に戻り、`None` の形は構築子の塊が残るので `Mismatch` のままである。

起きる条件は、`with` の本体がほかで `Error` を出さないことである。このとき型検査器は main を `Int {Error}` と型付けし、閉包の行に `Error` が入る。本体のほかの場所で raise しうるときは正しく動く。黙って誤った値を返す形ではないが、仕様上正しく、いま動いているプログラムを壊す。

## 案

- **A: 型引数の閉包に `$Outcome` を載せない（推奨）。** 呼ぶ側（`Fn` と書いた引数）だけを callback と見なす。「呼ぶかどうか」は scheme の引数の型が `Fn` かどうかで決まり、型検査器が推論した行では決まらない。今回の変更が直そうとした欠陥（効果付き閉包を型変数の引数に置くと落ちる）は、別の形で解く。
- B: 型検査器の行を閉包の本体の lower の結果に合わせて落とす。型検査器の側に手を入れるので影響が広い。

## 完了条件

- 上の 2 つの検体が main と同じ `102` を返す。門番（`# tier: core`、`# round: <slug>`）で測る。
- 効果付き閉包を `Map.insert` の値や `Option.unwrap_or` の代替に置く形（この変更が直そうとしたもの）も引き続き正しい値になる。両方を同時に満たすこと。
- 変異（型引数の閉包に `$Outcome` を載せる形に戻す）で門番が赤になる。

## 由来

PR #75（`cursor/critical-bug-management-1032`）のレビューで見つけた。`434e1fd`。仕様は §20（効果行）、§38（効果）、§41（型の正準形）、§46・§47（契約）。
