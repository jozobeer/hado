; Hado のハイライト（Neovim）。capture は 1 つずつ別の group にする。
;
; 型の `upper_name` と constructor の `upper_name` は別の capture にする。
; `queries/hado/highlights.scm` という置き場は Neovim の `query.lua` が
; runtimepath から `queries/<lang>/<name>.scm` を探すためで、文法リポジトリの
; 慣例（`queries/highlights.scm`）ではない。

(comment) @comment

(int) @number
(float) @number

(string) @string
(char) @character

(bool) @boolean

[
  "def"
  "fn"
  "if"
  "type"
  "module"
  "implement"
  "effect"
  "handler"
  "with"
  "return"
  "once"
  "abort"
  "again"
  "as"
  "shared"
  "not"
  "Fn"
  "Pure"
  "/if"
  "/fn"
  "/module"
  "/implement"
] @keyword

(rec_keyword) @keyword
(resume) @keyword
(library) @keyword

(dunder_name) @function.builtin

; 型としての `upper_name`
(type_ref (upper_name) @type)
(type_vars (upper_name) @type)
(contract_ref (upper_name) @type)
(impl_ref (upper_name) @type)
(module_section (upper_name) @type)
(module_item (upper_name) @type)
(implement_section (upper_name) @type)
(functor_section (upper_name) @type)
(apply_section (upper_name) @type)
(functor_param (upper_name) @type)
(effect_section (upper_name) @type)
(handler_section (upper_name) @type)
(type_definition (upper_name) @type)
(shared_type_realization (upper_name) @type)

; constructor としての `upper_name`
(constructor (upper_name) @constructor)
(constructor_expr (upper_name) @constructor)
(constructor_pattern (upper_name) @constructor)
; `type T = Payload { ... }` の `Payload`
(named_constructor (upper_name) @constructor)

; `name`（値・field・引数・名前空間）。キーワードは `word` で締め出してある。
(name) @variable

(path (name) @namespace)

[
  "+"
  "-"
  "*"
  "/"
  "%"
  "<"
  "<="
  ">"
  ">="
  "=="
  "!="
  "&&"
  "||"
  "|>"
] @operator
