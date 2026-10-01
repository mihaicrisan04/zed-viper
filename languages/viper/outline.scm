(method_definition "method" @context name: (identifier) @name) @item
(function_definition "function" @context name: (identifier) @name) @item
(predicate_definition "predicate" @context name: (identifier) @name) @item
(macro_definition "define" @context name: (identifier) @name) @item
(field_declaration "field" @context (typed_name name: (identifier) @name)) @item

(domain_definition "domain" @context name: (identifier) @name) @item
(domain_function "function" @context name: (identifier) @name) @item
(axiom "axiom" @context name: (identifier) @name) @item

(adt_definition "adt" @context name: (identifier) @name) @item
(adt_constructor name: (identifier) @name) @item
