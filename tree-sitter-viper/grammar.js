/**
 * @file Tree-sitter grammar for the Viper verification language (.vpr)
 *
 * Follows Silver's FastParser (viper/silver/parser/FastParser.scala) but is
 * deliberately permissive: it is meant for highlighting, not for rejecting
 * invalid programs.
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

// Lowest to highest, mirroring FastParser's expression layers.
const PREC = {
  binder: 0,
  ternary: 1,
  iff: 2,
  implies: 3,
  wand: 4,
  or: 5,
  and: 6,
  equality: 7,
  compare: 8,
  sum: 9,
  term: 10,
  unary: 11,
  postfix: 12,
  call: 13,
};

const sepBy1 = (rule, sep = ',') => seq(rule, repeat(seq(sep, rule)));
const sepBy = (rule, sep = ',') => optional(sepBy1(rule, sep));
const parens = rule => seq('(', rule, ')');
const brackets = rule => seq('[', rule, ']');
const semi = rule => seq(rule, optional(';'));

module.exports = grammar({
  name: 'viper',

  word: $ => $.identifier,

  extras: $ => [/\s/, $.line_comment, $.block_comment, $.annotation],

  conflicts: $ => [
    [$.macro_parameters, $._expression],
    [$.call_statement, $._expression],
  ],

  rules: {
    source_file: $ => repeat($._member),

    _member: $ => choice(
      $.import,
      $.macro_definition,
      $.field_declaration,
      $.method_definition,
      $.function_definition,
      $.predicate_definition,
      $.domain_definition,
      $.adt_definition,
    ),

    // --- members ---

    import: $ => semi(seq('import', field('path', choice($.string, $.angle_path)))),
    angle_path: _ => token(seq('<', /[^>\n]+/, '>')),

    macro_definition: $ => seq(
      'define',
      field('name', $.identifier),
      optional(field('parameters', $.macro_parameters)),
      field('body', choice($.block, $._expression)),
    ),
    macro_parameters: $ => parens(sepBy($.identifier)),

    field_declaration: $ => semi(seq('field', sepBy1($.typed_name))),

    method_definition: $ => seq(
      'method',
      field('name', $.identifier),
      field('parameters', $.parameters),
      optional(seq('returns', field('returns', $.parameters))),
      repeat($._specification),
      optional(field('body', $.block)),
    ),

    function_definition: $ => seq(
      'function',
      field('name', $.identifier),
      field('parameters', $.parameters),
      ':',
      field('type', $.type),
      repeat($._specification),
      optional(field('body', $.expression_body)),
    ),

    predicate_definition: $ => seq(
      'predicate',
      field('name', $.identifier),
      field('parameters', $.parameters),
      optional(field('body', $.expression_body)),
    ),

    domain_definition: $ => seq(
      'domain',
      field('name', $.identifier),
      optional(field('type_parameters', $.type_parameters)),
      optional($.interpretation_list),
      '{',
      repeat(semi(choice($.domain_function, $.axiom))),
      '}',
    ),
    type_parameters: $ => brackets(sepBy1($.identifier)),
    interpretation_list: $ => seq('interpretation', parens(sepBy(seq($.identifier, ':', $.string)))),

    domain_function: $ => seq(
      optional('unique'),
      'function',
      field('name', $.identifier),
      '(', sepBy(choice($.typed_name, $.type)), ')',
      ':',
      field('type', $.type),
      optional(seq('interpretation', $.string)),
    ),

    axiom: $ => seq('axiom', optional(field('name', $.identifier)), field('body', $.expression_body)),

    adt_definition: $ => seq(
      'adt',
      field('name', $.identifier),
      optional(field('type_parameters', $.type_parameters)),
      '{',
      repeat(semi($.adt_constructor)),
      '}',
      optional(seq('derives', '{', repeat(semi(seq($.identifier, optional(seq('without', sepBy1($.identifier)))))), '}')),
    ),
    adt_constructor: $ => seq(field('name', $.identifier), field('parameters', $.parameters)),

    parameters: $ => parens(sepBy($.typed_name)),
    typed_name: $ => seq(field('name', $.identifier), ':', field('type', $.type)),

    // --- specifications ---

    _specification: $ => semi(choice($.requires, $.ensures, $.decreases)),
    requires: $ => seq('requires', $._expression),
    ensures: $ => seq('ensures', $._expression),
    invariant: $ => seq('invariant', $._expression),
    decreases: $ => prec.right(seq(
      'decreases',
      optional(choice('_', '*', sepBy1($._expression))),
      optional(seq('if', $._expression)),
    )),

    expression_body: $ => seq('{', $._expression, '}'),

    // --- statements ---

    block: $ => seq('{', repeat(semi($._statement)), '}'),

    _statement: $ => choice(
      $.block,
      $.variable_declaration,
      $.assignment,
      $.call_statement,
      $.if_statement,
      $.while_statement,
      $.verification_statement,
      $.quasihavocall_statement,
      $.package_statement,
      $.label_statement,
      $.goto_statement,
      $.macro_definition,
    ),

    variable_declaration: $ => prec.right(seq('var', sepBy1($.typed_name), optional(seq(':=', field('value', $._expression))))),

    assignment: $ => seq(sepBy1(field('target', $._assign_target)), ':=', field('value', $._expression)),
    _assign_target: $ => choice($.identifier, $.field_access, $.call),

    // method call without results, or a macro used as a statement
    call_statement: $ => choice($.call, $.identifier),

    if_statement: $ => seq(
      'if', parens(field('condition', $._expression)),
      field('consequence', $.block),
      repeat($.elseif_clause),
      optional($.else_clause),
    ),
    elseif_clause: $ => seq('elseif', parens(field('condition', $._expression)), field('consequence', $.block)),
    else_clause: $ => seq('else', field('alternative', $.block)),

    while_statement: $ => seq(
      'while', parens(field('condition', $._expression)),
      repeat(semi(choice($.invariant, $.decreases))),
      field('body', $.block),
    ),

    verification_statement: $ => seq(
      choice('assert', 'assume', 'inhale', 'exhale', 'refute', 'fold', 'unfold', 'apply', 'quasihavoc'),
      $._expression,
    ),
    quasihavocall_statement: $ => seq('quasihavocall', sepBy1($.typed_name), '::', $._expression),
    package_statement: $ => prec.right(seq('package', $._expression, optional($.block))),

    label_statement: $ => prec.right(seq('label', field('name', $.identifier), repeat($.invariant))),
    goto_statement: $ => seq('goto', field('label', $.identifier)),

    // --- expressions ---

    _expression: $ => choice(
      $.identifier,
      $.integer,
      $.boolean,
      $.null,
      $.result,
      $.permission_literal,
      $.parenthesized_expression,
      $.call,
      $.field_access,
      $.index_expression,
      $.slice_expression,
      $.update_expression,
      $.unary_expression,
      $.binary_expression,
      $.ternary_expression,
      $.old_expression,
      $.acc_expression,
      $.perm_expression,
      $.quantifier,
      $.forperm_expression,
      $.let_expression,
      $.unfolding_expression,
      $.applying_expression,
      $.asserting_expression,
      $.collection_literal,
      $.map_literal,
      $.map_function,
      $.size_expression,
      $.range_expression,
      $.inhale_exhale_expression,
      $.new_expression,
    ),

    // `(f(x): T)` gives a function call an explicit type
    parenthesized_expression: $ => parens(seq($._expression, optional(seq(':', $.type)))),

    call: $ => prec(PREC.call, seq(field('function', $.identifier), field('arguments', $.arguments))),
    arguments: $ => parens(sepBy($._expression)),

    field_access: $ => prec(PREC.postfix, seq(field('object', $._expression), '.', field('field', $.identifier))),
    index_expression: $ => prec(PREC.postfix, seq($._expression, brackets($._expression))),
    slice_expression: $ => prec(PREC.postfix, seq($._expression, '[', choice(
      seq($._expression, '..', optional($._expression)),
      seq('..', $._expression),
    ), ']')),
    update_expression: $ => prec(PREC.postfix, seq($._expression, brackets(seq($._expression, ':=', $._expression)))),

    unary_expression: $ => prec(PREC.unary, seq(field('operator', choice('!', '-')), $._expression)),

    binary_expression: $ => {
      const table = [
        [prec.right, PREC.iff, '<==>'],
        [prec.right, PREC.implies, '==>'],
        [prec.right, PREC.wand, '--*'],
        [prec.right, PREC.or, '||'],
        [prec.right, PREC.and, '&&'],
        [prec.left, PREC.equality, choice('==', '!=')],
        [prec.left, PREC.compare, choice('<', '<=', '>', '>=', 'in')],
        [prec.left, PREC.sum, choice('+', '-', '++', 'union', 'intersection', 'setminus', 'subset')],
        [prec.left, PREC.term, choice('*', '/', '\\', '%')],
      ];
      return choice(...table.map(([assoc, precedence, operator]) => assoc(precedence, seq(
        field('left', $._expression),
        field('operator', operator),
        field('right', $._expression),
      ))));
    },

    ternary_expression: $ => prec.right(PREC.ternary, seq(
      field('condition', $._expression), '?',
      field('consequence', $._expression), ':',
      field('alternative', $._expression),
    )),

    old_expression: $ => seq('old', optional(brackets(choice('lhs', $.identifier))), parens($._expression)),
    acc_expression: $ => seq('acc', parens(seq($._expression, optional(seq(',', $._expression))))),
    perm_expression: $ => seq('perm', parens($._expression)),

    quantifier: $ => prec.right(PREC.binder, seq(
      choice('forall', 'exists'),
      sepBy1($.typed_name),
      '::',
      repeat($.trigger),
      field('body', $._expression),
    )),
    trigger: $ => seq('{', sepBy1($._expression), '}'),
    forperm_expression: $ => prec.right(PREC.binder, seq(
      'forperm', sepBy1($.typed_name), brackets($._expression), '::', field('body', $._expression),
    )),

    let_expression: $ => prec.right(PREC.binder, seq(
      'let', field('name', $.identifier), '==', parens($._expression), 'in', field('body', $._expression),
    )),
    unfolding_expression: $ => prec.right(PREC.binder, seq(
      'unfolding', choice($.acc_expression, $.call), 'in', field('body', $._expression),
    )),
    applying_expression: $ => prec.right(PREC.binder, seq('applying', parens($._expression), 'in', field('body', $._expression))),
    asserting_expression: $ => prec.right(PREC.binder, seq('asserting', parens($._expression), 'in', field('body', $._expression))),

    collection_literal: $ => seq(
      choice('Seq', 'Set', 'Multiset'),
      optional(brackets($.type)),
      parens(sepBy($._expression)),
    ),
    map_literal: $ => seq(
      'Map',
      optional(brackets(seq($.type, ',', $.type))),
      parens(sepBy(seq($._expression, ':=', $._expression))),
    ),
    map_function: $ => seq(choice('domain', 'range'), parens($._expression)),
    size_expression: $ => seq('|', $._expression, '|'),
    range_expression: $ => seq('[', $._expression, '..', $._expression, ')'),
    inhale_exhale_expression: $ => seq('[', $._expression, ',', $._expression, ']'),
    new_expression: $ => seq('new', parens(choice('*', sepBy($.identifier)))),

    // --- types ---

    type: $ => prec.right(choice(
      $.builtin_type,
      seq(alias(choice('Seq', 'Set', 'Multiset'), $.builtin_type), brackets($.type)),
      seq(alias('Map', $.builtin_type), brackets(seq($.type, ',', $.type))),
      seq($.identifier, optional(brackets(sepBy1($.type)))),
    )),
    builtin_type: _ => choice('Int', 'Bool', 'Perm', 'Ref', 'Rational'),

    // --- leaves ---

    identifier: _ => /[A-Za-z_$][A-Za-z0-9_$]*/,
    integer: _ => /\d+/,
    boolean: _ => choice('true', 'false'),
    null: _ => 'null',
    result: _ => 'result',
    permission_literal: _ => choice('write', 'none', 'wildcard', 'epsilon'),
    string: _ => token(seq('"', /[^"\n]*/, '"')),
    annotation: $ => seq('@', field('name', $.annotation_name), parens(sepBy($.string))),
    annotation_name: _ => token.immediate(/[A-Za-z_$][A-Za-z0-9_$.]*/),
    line_comment: _ => token(seq('//', /.*/)),
    block_comment: _ => token(seq('/*', /[^*]*\*+([^/*][^*]*\*+)*/, '/')),
  },
});
