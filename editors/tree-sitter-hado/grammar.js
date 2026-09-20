// Hado の tree-sitter 文法。
//
// 節番号は docs/spec.md を指す。字句と構文の形は付録C を正とし、このファイルは
// その規則名をそのまま node 名にする（`type_expr` は `type_expr`、
// `pattern_arm` は `pattern_arm`）。付録C の `expression := pipe | prefix_form`
// のような 1 段の包みは node を作らず、golden に現れる構造だけを残す。
//
// 字句の優先順は最長一致に任せる。キーワードは `token(prec(1, ...))` で
// `name` / `upper_name` より先に読む（`true` は `name` にも一致するため）。

const PREC = {
  PIPE: 1,
  OR: 2,
  AND: 3,
  COMPARE: 4,
  ADDITIVE: 5,
  MULTIPLICATIVE: 6,
  UNARY: 7,
  APPLICATION: 8,
  // constructor の payload は並置より先に読む（付録C の注記）。式側は GLR の
  // 自己 conflict（`conflicts` の `$.constructor_expr`）で解く。パターン側は
  // 優先順位で強くする。`| Some(x) => ...` で式とパターンの両方が読めるため。
  CONSTRUCTOR_PATTERN: 10,
};

// キーワード。付録C のキーワード表（`docs/spec.md`）と同じ集合。
const keyword = (word) => token(prec(1, word));

const commaSep1 = (rule) => seq(rule, repeat(seq(',', rule)));
const semicolonSep1 = (rule) => seq(rule, repeat(seq(';', rule)));

module.exports = grammar({
  name: 'hado',

  extras: ($) => [/\s/, $.comment],

  // 予約語を `name` から締め出す。付録C はキーワードを名前から除く。
  word: ($) => $.name,

  // 付録C は腕の列を緩く読む（`pattern_fallback` は最後に 1 つで、途中の規則は
  // 網羅性の段階で当てる）。`|` の後が式にもパターンにも読めるので、GLR の分岐を
  // 許して `=>` の有無で決めさせる。
  conflicts: ($) => [
    [$.constructor_expr],
    [$.primary, $.literal_pattern],
    [$.list_expr, $.list_pattern],
    [$.primary, $.pattern],
    [$.typed_param, $.lambda_params],
    [$.condition_arms],
    [$.pattern_arms],
    [$.expression, $.arm_body],
    [$.arm_body, $._prefix_form],
  ],

  rules: {
    // 付録C「プログラム」。`library` は繰り返す outer_item の 1 つなので、最初の
    // 見出しより前なら宣言の間にも置ける（`crates/hado/src/parser.rs:parse_items` の
    // `outer` も同じで、見出しの後は `library` を読まない）。見出しの後も宣言は
    // 読み続ける。`effect` と `handler` の本文は操作だけなので、その後の `def` は
    // 外の宣言に戻る。`implement` の本文は次の見出しまで続く。
    source_file: ($) =>
      seq(
        repeat(choice($.library, $._outer_item)),
        repeat(seq($._section, repeat($._outer_item))),
      ),

    // 付録C「プログラム」: 最初の見出しより前の outer_item
    library: () => keyword('library'),

    _outer_item: ($) => choice($.type_definition, $.definition),

    _section: ($) =>
      choice(
        $.module_section,
        $.implement_section,
        $.functor_section,
        $.apply_section,
        $.effect_section,
        $.handler_section,
      ),

    // 付録C「名前付き定義」
    definition: ($) =>
      seq(
        keyword('def'),
        choice(
          // 値定義。`rec` は付けられない（E102）
          seq($.name, '=', $.expression),
          seq(
            optional($.rec_keyword),
            $.name,
            choice(
              seq($.parameters, optional($.return_type), '=>', $.expression),
              seq($.parameters, optional($.return_type), $.condition_arms),
              // arity 1 のパターン分岐（第31節）。引数列は持たない
              $.pattern_arms,
            ),
          ),
        ),
      ),

    rec_keyword: () => keyword('rec'),

    parameters: ($) =>
      choice(
        repeat1($.param),
        seq('(', commaSep1($.typed_param), ')'),
      ),

    param: ($) => choice($.name, '_'),

    typed_param: ($) => seq($.param, ':', $.type_expr),

    return_type: ($) => seq(':', $.result),

    // 付録C「型定義」
    type_definition: ($) =>
      seq(
        keyword('type'),
        optional($.type_vars),
        $.upper_name,
        '=',
        choice(
          repeat1(seq('|', $.constructor)),
          $.record_type,
          $.named_constructor,
          // `implement` 本文の実体化（第47節）。`shared` 付きは別の node
          $.type_expr,
        ),
      ),

    // 付録C「型定義」の `"=" upper_name record_type`。名前付き field を持つ単一の
    // constructor で、宣言した型名と payload の名前を別の node に分ける。裸の
    // `upper_name` は `type_expr` とも読めるので、ここでは `record_type` を必須に
    // して `$.constructor` との衝突を避ける。
    named_constructor: ($) => seq($.upper_name, $.record_type),

    shared_type_realization: ($) =>
      seq(
        keyword('shared'),
        keyword('type'),
        optional($.type_vars),
        $.upper_name,
        '=',
        $.type_expr,
      ),

    type_vars: ($) =>
      choice($.upper_name, seq('(', commaSep1($.upper_name), ')')),

    constructor: ($) =>
      seq(
        $.upper_name,
        optional(
          choice(seq('(', commaSep1($.type_expr), ')'), $.record_type),
        ),
      ),

    record_type: ($) => seq('{', semicolonSep1($.field_type), '}'),

    field_type: ($) => seq($.name, ':', $.type_expr),

    // 付録C「型式」
    type_expr: ($) => seq($.type_atom, repeat($.type_ref)),

    type_atom: ($) =>
      choice(
        $.type_ref,
        seq('(', commaSep1($.type_expr), ')'),
        seq(keyword('Fn'), '(', commaSep1($.type_expr), '=>', $.result, ')'),
      ),

    result: ($) => seq($.type_expr, optional($.effect_row)),

    effect_row: ($) =>
      choice(
        seq('{', commaSep1($.type_expr), '}'),
        keyword('Pure'),
      ),

    type_ref: ($) => seq(optional($.path), $.upper_name, optional(seq('.', $.upper_name))),

    path: ($) => repeat1(seq($.name, '::')),

    // 付録C「module」
    module_section: ($) => prec.right(seq(keyword('module'), $.upper_name, repeat($.module_item))),

    module_item: ($) =>
      choice(
        seq(keyword('type'), optional($.type_vars), $.upper_name),
        seq(keyword('def'), optional($.type_vars), $.name, ':', $.result),
        seq(
          keyword('def'),
          optional($.type_vars),
          $.name,
          '(',
          commaSep1($.contract_param),
          ')',
          ':',
          $.result,
        ),
      ),

    contract_param: ($) => seq($.name, ':', $.type_expr),

    implement_section: ($) =>
      prec.right(
        seq(
          keyword('implement'),
          $.contract_ref,
          keyword('as'),
          $.upper_name,
          repeat($._implement_item),
        ),
      ),

    functor_section: ($) =>
      prec.right(
        seq(
          keyword('implement'),
          $.contract_ref,
          keyword('as'),
          $.upper_name,
          '(',
          commaSep1($.functor_param),
          ')',
          repeat($._implement_item),
        ),
      ),

    apply_section: ($) =>
      seq(
        keyword('implement'),
        $.contract_ref,
        keyword('as'),
        $.upper_name,
        '=',
        $.impl_ref,
        '(',
        commaSep1($.impl_ref),
        ')',
      ),

    _implement_item: ($) =>
      choice($.definition, $.type_definition, $.shared_type_realization),

    functor_param: ($) => seq($.upper_name, ':', $.contract_ref),

    contract_ref: ($) => seq(optional($.path), $.upper_name),

    impl_ref: ($) => seq(optional($.path), $.upper_name),

    // 付録C「module」の effect と handler
    effect_section: ($) =>
      seq(keyword('effect'), optional($.type_vars), $.upper_name, repeat($.effect_item)),

    effect_item: ($) =>
      seq(
        $.op_kind,
        $.name,
        optional(seq('(', commaSep1($.contract_param), ')')),
        optional(seq(':', $.result)),
      ),

    op_kind: () => choice(keyword('once'), keyword('abort'), keyword('again')),

    handler_section: ($) =>
      seq(
        keyword('handler'),
        optional($.type_vars),
        $.contract_ref,
        keyword('as'),
        $.upper_name,
        optional($.handler_params),
        repeat($.handler_item),
      ),

    handler_params: ($) => seq('(', commaSep1($.typed_param), ')'),

    handler_item: ($) =>
      choice(
        seq($.op_kind, $.name, repeat($.param), '=>', $.expression),
        seq(keyword('return'), $.param, '=>', $.expression),
      ),

    // 付録C「式（優先順位の低い方から）」
    expression: ($) => choice($.pipe, $._prefix_form),

    pipe: ($) => choice($.or, prec.left(PREC.PIPE, seq($.pipe, '|>', $.or))),

    or: ($) => choice($.and, prec.left(PREC.OR, seq($.or, '||', $.and))),

    and: ($) => choice($.compare, prec.left(PREC.AND, seq($.and, '&&', $.compare))),

    compare: ($) =>
      choice(
        $.additive,
        prec(PREC.COMPARE, seq($.additive, choice('<', '<=', '>', '>=', '==', '!='), $.additive)),
      ),

    additive: ($) =>
      choice(
        $.multiplicative,
        prec.left(PREC.ADDITIVE, seq($.additive, choice('+', '-'), $.multiplicative)),
      ),

    multiplicative: ($) =>
      choice(
        $.unary,
        prec.left(PREC.MULTIPLICATIVE, seq($.multiplicative, choice('*', '/', '%'), $.unary)),
      ),

    unary: ($) =>
      choice(
        prec(PREC.UNARY, seq(choice('-', keyword('not')), $.unary)),
        $.application,
        $.postfix,
      ),

    application: ($) => prec.left(PREC.APPLICATION, seq($.postfix, repeat1($.postfix))),

    postfix: ($) => seq($.primary, repeat(seq('.', $.name))),

    primary: ($) =>
      choice(
        $.int,
        $.float,
        $.char,
        $.string,
        $.bool,
        $.void,
        $.name,
        $.dunder_name,
        $.constructor_expr,
        $.record_expr,
        $.list_expr,
        $.lambda,
        $.resume,
        seq('(', $.expression, ')'),
      ),

    resume: () => keyword('resume'),

    constructor_expr: ($) =>
      seq(
        optional($.path),
        $.upper_name,
        optional(choice(seq('(', commaSep1($.expression), ')'), $.record_expr)),
      ),

    record_expr: ($) => seq('{', semicolonSep1(seq($.name, '=', $.expression)), '}'),

    list_expr: ($) =>
      choice(
        seq('[', ']'),
        seq('[', commaSep1($.expression), optional(seq(',', '...', $.expression)), ']'),
      ),

    // 付録C「継続と匿名関数」
    _prefix_form: ($) =>
      choice($.binding, $.rail_binding, $.rail_discard, $.with_form, $.branch),

    with_form: ($) =>
      seq(keyword('with'), $.impl_ref, optional($.with_args), '->', $.expression),

    with_args: ($) => seq('(', commaSep1($.expression), ')'),

    binding: ($) =>
      seq(keyword('let'), $.pattern, '=', $.expression, '->', $.expression),

    rail_binding: ($) =>
      seq(keyword('let'), $.pattern, '?', '=', $.expression, '->', $.expression),

    rail_discard: ($) => seq(keyword('do'), $.expression, '->', $.expression),

    lambda: ($) =>
      seq(
        keyword('fn'),
        '(',
        choice(
          seq($.lambda_params, optional($.return_type), '=>', $.expression),
          seq($.lambda_params, optional($.return_type), $.condition_arms),
          $.pattern_arms,
        ),
        ')',
      ),

    lambda_params: ($) =>
      choice(
        seq($.param, repeat(seq(',', $.param))),
        seq($.typed_param, repeat(seq(',', $.typed_param))),
      ),

    // 付録C「分岐」
    branch: ($) => choice($.condition_branch, $.boolean_branch, $.pattern_branch),

    boolean_branch: ($) =>
      seq(keyword('if'), $.expression, '=>', $.arm_body, '|', $.arm_body),

    condition_branch: ($) => seq(keyword('if'), $.condition_arms),

    condition_arms: ($) => repeat1(choice($.condition_arm, $.pattern_fallback)),

    condition_arm: ($) => seq('|', $.expression, '=>', $.arm_body),

    pattern_branch: ($) =>
      seq(keyword('if'), $.expression, repeat(seq(',', $.expression)), $.pattern_arms),

    pattern_arms: ($) => repeat1(choice($.pattern_arm, $.pattern_fallback)),

    pattern_arm: ($) =>
      seq(
        '|',
        $.pattern,
        repeat(seq(',', $.pattern)),
        '=>',
        choice($.arm_body, $.arm_conditions),
      ),

    arm_conditions: ($) => seq(keyword('if'), $.condition_arms),

    pattern_fallback: ($) => seq('|', $.arm_body),

    arm_body: ($) => choice($.pipe, $.binding, $.rail_binding, $.rail_discard),

    // 付録C「パターン」
    pattern: ($) =>
      choice(
        '_',
        $.name,
        $.constructor_pattern,
        $.record_pattern,
        $.list_pattern,
        $.literal_pattern,
      ),

    constructor_pattern: ($) =>
      prec.left(
        PREC.CONSTRUCTOR_PATTERN,
        seq(
          optional($.path),
          $.upper_name,
          optional(choice(seq('(', commaSep1($.pattern), ')'), $.record_pattern)),
        ),
      ),

    list_pattern: ($) =>
      choice(
        seq('[', ']'),
        seq('[', commaSep1($.pattern), optional(seq(',', '...', choice($.name, '_'))), ']'),
      ),

    record_pattern: ($) => seq('{', semicolonSep1($.field_pattern), '}'),

    field_pattern: ($) => choice($.name, seq($.name, '=', $.pattern)),

    literal_pattern: ($) =>
      choice(
        seq(optional('-'), $.int),
        seq(optional('-'), $.float),
        $.char,
        $.string,
        $.bool,
      ),

    // 付録C「字句」
    int: () => /[0-9]+/,

    float: () => /[0-9]+\.[0-9]+/,

    // 付録C「字句」。生の改行は文字列にも文字にも入れられない（`crates/hado/src/lexer.rs`
    // の `lex_string` と `lex_char` が拒む）。閉じ忘れを次の行へ飲み込ませない。
    char: () => token(seq("'", choice(seq('\\', /[nt\\']/), /[^'\\\n]/), "'")),

    string: () =>
      token(seq('"', repeat(choice(seq('\\', /[nt\\"]/), /[^"\\\n]/)), '"')),

    bool: () => choice(keyword('true'), keyword('false')),

    void: () => '()',

    name: () => /[a-z][a-zA-Z0-9_]*/,

    dunder_name: () => /__[a-z][a-zA-Z0-9_]*/,

    upper_name: () => /[A-Z][a-zA-Z0-9_]*/,

    comment: () => token(seq('//', /[^\n]*/)),
  },
});
