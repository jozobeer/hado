# Cranelift 0.135 の呼び出し形

M0（`docs/plan.md` の M0）で確定させた、`emit` が依存する Cranelift API の実際の形。
**docs.rs の `latest` は 0.135 と食い違う**（`FunctionBuilder::finalize` が latest では引数を取らない）。
迷ったら `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/cranelift-*-0.135.1` の実物を読む。

下記は全項目を registry の実物と照合済み（2026-09-06）。

Cargo の `"0.135"` はいま 0.135.1 に解決する。Cranelift 側の `rust-version` は 1.95.0。ホスト 1.98.0 で足りる。

### `FunctionBuilder::finalize`

```rust
pub fn finalize(mut self, frontend_config: TargetFrontendConfig)
```

（`cranelift-frontend-0.135.1/src/frontend.rs:704`）

呼び出し: `bcx.seal_all_blocks(); bcx.finalize(module.target_config());`

`module.target_config()` は `Module::target_config` → `isa.frontend_config()`（`cranelift-module-0.135.1/src/module.rs:839-840`）。`TargetFrontendConfig` は `default_call_conv`、`pointer_width`、`page_size_align_log2`（`cranelift-codegen-0.135.1/src/isa/mod.rs:250-262`）。

`finalize()` 引数なしは 0.135 では型エラーになる。

### `ObjectModule` でオブジェクトを吐くまで

`use cranelift_module::Module;` が要る。メソッドはトレイト側。

1. ISA を作る（次節の flags）。`cranelift_native::builder() -> Result<isa::Builder, &'static str>`（`cranelift-native-0.135.1/src/lib.rs:16`）。`isa::Builder::finish(&self, shared_flags: settings::Flags) -> CodegenResult<OwnedTargetIsa>`（`cranelift-codegen-0.135.1/src/isa/mod.rs:225`）。`OwnedTargetIsa = Arc<dyn TargetIsa>`。
2. `ObjectBuilder::new(isa, name, default_libcall_names()) -> ModuleResult<Self>`（`cranelift-object-0.135.1/src/backend.rs:50-54`）。`default_libcall_names` は `cranelift_module`。
3. `ObjectModule::new(builder) -> Self`（同 `261`）。
4. `module.declare_function(name, linkage, &signature) -> ModuleResult<FuncId>`。C から呼ぶ `hado_entry` は `Linkage::Export`。内部は `Linkage::Local`。C ランタイムは `Linkage::Import`。
5. `Context::new()`。`ctx.func = Function::with_name_signature(UserFuncName::user(0, func_id.as_u32()), sig)`。
6. `FunctionBuilderContext::new()`。`FunctionBuilder::new(&mut ctx.func, &mut func_ctx)`（`cranelift-frontend-0.135.1/src/frontend.rs:323`）。
7. `create_block`、`switch_to_block`、入口なら `append_block_params_for_function_params`、`bcx.ins()...`、`seal_all_blocks`、`finalize(module.target_config())`。
8. `module.define_function(func_id, &mut ctx) -> ModuleResult<()>`（`cranelift-module-0.135.1/src/module.rs:958`）。
9. `let product = module.finish();` → `ObjectProduct`（`cranelift-object-0.135.1/src/backend.rs:683`）。消費する。
10. `product.emit() -> Result<Vec<u8>, object::write::Error>`（同 `1161`）。バイトを `.o` に書く。
11. ホスト `cc` で `.o` + runtime の `.a` + `-lgc` を実行ファイルにする。サブプロセスで走らせる。

参照実装: `cranelift-object-0.135.1/tests/basic.rs` の `aarch64_colocated_data_symbol_reloc`（267–314 行）と `eh_frame::define_leaf`（344–363 行）。

### `tail` と `return_call`

規約:

```rust
let mut sig = cranelift_codegen::ir::Signature::new(CallConv::Tail);
sig.params.push(AbiParam::new(types::I64));
sig.returns.push(AbiParam::new(types::I64));
```

`CallConv::Tail`（`cranelift-codegen-0.135.1/src/isa/call_conv.rs:31`）。`supports_tail_calls` が true なのは `Tail` だけ（同 `88-92`）。

発行:

```rust
let callee = module.declare_func_in_func(func_id, &mut bcx.func); // -> ir::FuncRef
bcx.ins().return_call(callee, &args); // -> Inst
```

`InstBuilder::return_call(self, FN: FuncRef, args: &[Value]) -> Inst`（docs.rs cranelift-codegen 0.135.1）。ブロック終端。

`hado_entry` は `CallConv::SystemV`。そこから `tail` 側へは `bcx.ins().call(func_ref, &args)`。`return_call` は使わない。verifier は callee の規約が caller と同じで、かつ tail を許すことを要求する（`verifier/mod.rs:1710-1734`）。lowering も caller が `Tail` でない `return_call` を拒否する（`machinst/abi.rs:1824-1829`）。

ランタイム extern（印字・`hado_alloc`）は `CallConv::SystemV` の `Linkage::Import`。末尾位置でも `call` + `return`。

x64 の整数引数レジスタは Tail でも SysV と同じ 6 本（`isa/x64/abi.rs:1021-1039`）。9 引数はスタックに乗る。

### ISA flags

```rust
use cranelift_codegen::settings::{self, Configurable};

let mut flag_builder = settings::builder(); // -> settings::Builder
flag_builder.set("is_pic", "true")?;
flag_builder.set("preserve_frame_pointers", "true")?;
flag_builder.set("opt_level", "none")?;
let flags = settings::Flags::new(flag_builder);
let isa = cranelift_native::builder()?.finish(flags)?;
```

`Configurable::set(&mut self, name: &str, value: &str) -> SetResult<()>`（`cranelift-codegen-0.135.1/src/settings.rs:38, 258`）。`opt_level` は列挙なので `set` が必要。`Configurable` トレイトを use しないと `set` は見えない。

既定（`settings.rs` テストが文字列比較で固定、491–510 行）: `opt_level = "none"`、`enable_verifier = true`、`is_pic = false`、`preserve_frame_pointers = false`。`enable_verifier` は触らない。`is_pic` と `preserve_frame_pointers` は true に上げる。`opt_level` は既定が `none` でも `set("opt_level", "none")` を書く。

歩く骨格と手組み CLIF の両方で同じ 3 flags。`preserve_frame_pointers=false` では `tail` + `return_call` の emit が panic する（`docs/plan.md:240`）。

### `iadd_imm` / `icmp_imm`

0.135 では即値版オペコードは無く、`InstBuilder` の `iadd_imm` / `icmp_imm` は `#[deprecated]` の薄い包み（`cranelift-codegen-meta-0.135.1/src/gen_inst.rs:1349-1361, 1460-1461`）。`iconst` + `iadd` / `icmp`、または `iadd_imm_s` / `icmp_imm_s` を使う。

```rust
fn iadd(self, x: Value, y: Value) -> Value
fn iconst<T1: Into<Imm64>>(self, NarrowInt: Type, N: T1) -> Value
fn icmp(self, Cond: IntCC, x: Value, y: Value) -> Value
```

Int は `i64`（`docs/plan.md:27`）。Cranelift 型は `types::I64`。
