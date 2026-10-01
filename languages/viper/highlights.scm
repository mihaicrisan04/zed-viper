; later patterns win in Zed, so generic captures come first

(identifier) @variable

; --- keywords ---

[
  "import" "define" "field" "method" "function" "predicate" "domain" "axiom"
  "adt" "derives" "without" "returns" "var" "unique" "interpretation"
] @keyword

[ "requires" "ensures" "invariant" "decreases" ] @keyword

[ "if" "elseif" "else" "while" "goto" "label" ] @keyword

[
  "assert" "assume" "inhale" "exhale" "refute" "fold" "unfold"
  "apply" "package" "quasihavoc" "quasihavocall"
] @keyword

[
  "forall" "exists" "forperm" "let" "in" "unfolding" "applying" "asserting" "new"
  "union" "intersection" "setminus" "subset"
] @keyword

[ "acc" "perm" "old" "lhs" ] @function.builtin

(map_function [ "domain" "range" ] @function.builtin)
(collection_literal [ "Seq" "Set" "Multiset" ] @type.builtin)
(map_literal "Map" @type.builtin)

; --- literals ---

(integer) @number
(boolean) @boolean
(null) @constant.builtin
(permission_literal) @constant.builtin
(result) @variable.special
[ (string) (angle_path) ] @string
[ (line_comment) (block_comment) ] @comment

; --- types ---

(type (identifier) @type)
(builtin_type) @type.builtin
(type_parameters (identifier) @type)
(domain_definition name: (identifier) @type)
(adt_definition name: (identifier) @type)

; --- definitions and uses ---

(method_definition name: (identifier) @function)
(function_definition name: (identifier) @function)
(predicate_definition name: (identifier) @function)
(domain_function name: (identifier) @function)
(axiom name: (identifier) @label)
(macro_definition name: (identifier) @function)
(macro_parameters (identifier) @variable.parameter)
(adt_constructor name: (identifier) @constructor)

(call function: (identifier) @function)

(parameters (typed_name name: (identifier) @variable.parameter))
(field_declaration (typed_name name: (identifier) @property))
(field_access field: (identifier) @property)
(new_expression (identifier) @property)

(label_statement name: (identifier) @label)
(goto_statement label: (identifier) @label)
(old_expression (identifier) @label)

(annotation "@" @attribute name: (annotation_name) @attribute)

; --- operators and punctuation ---

[
  "<==>" "==>" "--*" "||" "&&" "==" "!=" "<" "<=" ">" ">="
  "+" "-" "++" "*" "/" "\\" "%" "!" ":=" "?" ".."
] @operator

(size_expression "|" @operator)

[ "(" ")" "[" "]" "{" "}" ] @punctuation.bracket

[ "," ";" ":" "::" "." ] @punctuation.delimiter
