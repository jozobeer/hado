# spec/ の入力の検証を詰める（spec-lock-input-hardening）
優先度: 中。

## 現象

`spec/` の正本を特定の形に壊すと、`scripts/spec-lock.sh` が壊れた `docs/spec.md` を作ったうえで一致を報告する。`docs/spec.md` の表が壊れると GFM として読めなくなる。

字下げしたフェンス行は、Markdown が閉じフェンスとして読むのに、門番は通す。

```
$ python3 -c "..."   # 字句群の本文に 3 文字下げたフェンス行を足す
$ bash scripts/spec-lock.sh >/dev/null 2>&1; echo exit=$?
exit=0
$ bash scripts/verify-spec-model.sh 2>&1 | tail -1
PASSED: 2/2
```

`|` は `anchor` だけが逃がされているので、段階に `|` を入れると表の列が増える。

```
$ python3 -c "..."   # E2xx の段階と spec/stages.toml の E2 を 名前|解決 にする
$ bash scripts/spec-lock.sh >/dev/null 2>&1; bash scripts/spec-lock.sh --check; echo exit=$?
spec-lock --check: 一致（70 エラーコード、10 文法群）
exit=0
$ grep -n '名前|解決' docs/spec.md | head -1
2037:| E201 | implicit-recursion | §9 | 名前|解決 | 自分を参照する名前 |
```

段階の対応表は、使われない族を持っていても通る。

```
$ python3 -c "..."   # spec/stages.toml に E8 を足す
$ bash scripts/verify-spec-model.sh 2>&1 | tail -2
OK: the model is well formed and its sections exist
PASSED: 2/2
```

## 原因

- `scripts/spec-lock.sh` の `grammar_groups` は本文のフェンス行を `line.startswith` で見ており、字下げを考えていない。
- `scripts/spec-lock.sh` の `error_table` は `anchor` だけを `replace` で逃がし、他のセルは素通しである。
- `scripts/verify-spec-model.sh` の `model_case` は、誤りが族の対応表を引けることだけを見て、対応表の族が誤りの族を覆っていることを見ていない。

## 案

推奨は (a)。

- (a) 3 件とも入力の検証で弾く。字下げは 3 文字までを正規化してフェンス行と見なし、表のセルはすべて `|` を逃がし（または `|` を拒み）、対応表の族の集合が誤りの族の集合と等しいことを見る。
- (b) `error_table` のセルをすべて逃がすだけにし、残りは `AGENTS.md` の「正本は手で壊さない」に委ねる。壊れた表が残るので弱い。

## 完了条件

3 つの変異のいずれでも `scripts/spec-lock.sh --check` か `scripts/verify-spec-model.sh` が赤になる。門番は `scripts/verify-spec-model.sh` が持つ。

## 由来

`docs/wants.md` になし。PR #34 の審査 6 回目。同 PR の往復が予算（3 回）を超えたため、`AGENTS.md` の「往復に予算を置く」に従って切り出した。`docs/issues/gate-optimize-flag.md` と同じ PR から出ている。
