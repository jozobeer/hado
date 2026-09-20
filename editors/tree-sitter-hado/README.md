# tree-sitter-hado

Hado（`.hd` / `.hdi`）の tree-sitter 文法と、Neovim のハイライト。

`grammar.js` は `docs/spec.md` の「付録C 文法の概形」と第7節の優先順位を写した
もの。`queries/hado/highlights.scm` は Neovim の highlighter が runtimepath から
`queries/<lang>/highlights.scm` を探す規約に合わせてこの置き場にある。

## ビルド

`tree-sitter` は版によって生成物が変わる。PATH 上の偶然の導入ではなく、
`scripts/treesitter-setup.sh` が返す 0.27.0 を使う。

```
cache=$(mktemp -d)
tools=$(bash scripts/treesitter-setup.sh --cache "$cache")
ts=$(printf '%s\n' "$tools" | sed -n 's/^tree-sitter //p')
cd editors/tree-sitter-hado
XDG_CACHE_HOME="$cache" "$ts" generate --js-runtime native
XDG_CACHE_HOME="$cache" "$ts" build -o "$cache/hado.so" .
```

`generate` が作る `src/`（`parser.c` など）と `*.so` は `.gitignore` にある。

## Neovim 0.12.5

`queries/hado/highlights.scm` は runtimepath から読ませる。parser はビルドした
`.so` を絶対パスで教える。

```lua
vim.opt.runtimepath:prepend('/abs/path/to/editors/tree-sitter-hado')
vim.filetype.add({ extension = { hd = 'hado', hdi = 'hado' } })
vim.treesitter.language.add('hado', { path = '/abs/path/to/hado.so' })
vim.api.nvim_create_autocmd('FileType', {
  pattern = 'hado',
  callback = function() vim.treesitter.start() end,
})
```

## 門番

`bash scripts/verify-treesitter.sh`（`tier: core`）が、受理コーパスと `stdlib/`
の parse、宣言形と第7節の優先順位の golden、Neovim の extmark と highlight group
を測る。golden は `test/golden/` の `.hd` と `.tree` の対で、`.tree` は
`scripts/treesitter_dump.py` が作る node 種別だけの木。
