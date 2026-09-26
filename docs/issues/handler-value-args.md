# handler の値引数が腕で束縛されない（handler-value-args）
優先度: 中。

## 現象

§38 は handler が値の引数を取れるとし、`handler A Yield as Take(limit: Int)` を例に挙げる。腕の中でその引数を参照すると、名前解決が E204 で止める（2026-09-26）。

```
$ cat target/issue/value-arg/main.hd
// needs: prelude
handler Error as OrValue(fallback: Int)
abort raise _ => fallback
def main = with OrValue(0) -> Int.parse "x"
$ cargo run --offline -q -- check target/issue/value-arg/main.hd
main.hd:3:18: error[E204]: unbound name fallback
```

## 原因

`crates/hado/src/name_resolution.rs` の `handler` は、腕ごとに操作の引数（`bind_params`）と `return` の引数だけをスコープに入れ、`HandlerDecl` の `params` を入れない。型検査と単相化が値引数を腕へ渡すかは未確認である。

## 案

- 推奨: 名前解決・型検査・単相化の 3 段で、handler の値引数を全ての腕のスコープに入れる。`with` の引数の評価は `with` の位置で 1 回とする。
- 退ける: §38 から値引数を消す。状態を持つ handler の初期値と、既定値を持ち込む handler（`with OrValue(0) -> …`）が書けなくなる。

## 完了条件

- 上の `value-arg` が `0` を印字して終了コード 0 になる。
- §38 の `Take(limit: Int)` の形を受理コーパスが固定する。
- `cargo test --offline` が 0 failed。

## 由来

`docs/ledger.md` の R31 の記録に既知の制限として 1 行ある。2026-09-26 の実験（`do-option-absent`）で、`Option.unwrap_or` にあたる既定値を handler で書こうとして再び当たった。§38、E204。
