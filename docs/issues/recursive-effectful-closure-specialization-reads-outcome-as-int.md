# 再帰する関数が返す効果付き閉包を、$Outcome のまま Int として読む（recursive-effectful-closure-specialization-reads-outcome-as-int）

優先度: 高。

## 現象

効果行を持つ閉包を返す再帰関数を、`as f ->` で受けて適用すると、**型検査と単相化が通り、実行時に黙って誤った値を返す**。期待値は `5` だが `128032991129504` を出力し、終了コードは 0 である。

```
$ cat > target/fatal/main.hd <<'EOF'
handler Error as Catch
abort raise _ => 0
return value => value
def rec pick n
| n < 1 => fn(x => 8 / x)
| n < 2 => fn(x => x + 1)
| pick (n - 2)
def main = Int.to_string (with Catch -> pick 3 as f -> f 4)
EOF
$ ./target/debug/hado run target/fatal/main.hd
128032991129504
$ echo $?
0
```

（値は走行ごとに変わる。下の表も同じ。）

再帰しない場合は正しい。`pick 1` は `5`、`pick 0` は `2` を返す。再帰する `pick 3` は誤った値を返す（値は走行ごとに変わる）。

```
$ mkdir -p target/f3 target/f1
$ cp target/f3/main.hd target/f1/main.hd && sed -i 's/pick 3/pick 1/' target/f1/main.hd
$ ./target/debug/hado run target/f1/main.hd
5
$ ./target/debug/hado run target/f3/main.hd
132853598982048
```

2 つの検体は同じ入力根に置かない（`Catch` が重複して E205 になる）。

この欠陥を持つ変更（PR #73、`e75a828`）を入れる前の main では、同じ入力が単相化の誤りで**停止する**。

```
$ ./target/debug/hado run target/fatal/main.hd
invalid branch types
$ echo $?
1
```

つまりこの変更は、コンパイルできない状態を、黙って誤った値を返す状態へ変える。

## 原因

`crates/hado/src/mono.rs` の `same_specialization_result`（105 行）が、効果を落とした論理型（`logical_argument`）だけで一致を判定する。`recursive_specialization`（5870 行）がこれを `active.result` の照合に使うため、本体を lower している途中の署名（効果行を落とした `Closure([Int], Int)`）が、実際に lower される結果型（`Closure([Int], $Outcome[Int,Int,Void])`）と同一と見なされる。

`dump-mono` で見ると、`file0$pick#Catch` の実際の結果型は `Closure([Int], $Outcome[Int,Int,Void])` であるのに、本体の中の再帰呼び出しの `CallDirect` には `result: Closure([Int], Int)` が付く。そのあと `as_outcome_closure` がこの呼び出しを純粋な閉包とみなして `outcome_N` で包むので、`$Outcome` へのポインタが `Int` として読まれる。

## 案

- **A: 再帰の照合では論理型の一致で通さない（推奨）。** `recursive_specialization` は `active.result` を厳密に比べ、食い違えば従来どおり `recursive specialization changes argument types` で止める。再帰の途中では lower した結果型がまだ確定していないので、ここで通すと必ず誤る。`cached_specialization` の側は、確定した署名どうしの比較なので `same_specialization_result` のままでよいかを別途確かめる。
- B: 再帰の結果型を先に確定させてから本体を lower する（2 段階の単相化）。効果は大きい。

## 完了条件

- 上の検体が `5` を返すか、止まる（誤った値を返さない）。門番（`# tier: core`、`# round: <slug>`）で測る。
- 検体を `run` して、終了コード 0 のときは期待値ちょうどであること。`invalid branch types` などの停止は許す。
- 変異（`same_specialization_result` を論理型の一致で通す形に戻す）で門番が赤になる。

## 由来

PR #73（`cursor/critical-bug-management-09e4`）のレビューで見つけた。`e75a828`。仕様は §36（分岐）、§26（パターン分岐）、§20（効果行）、§38（効果操作）。
