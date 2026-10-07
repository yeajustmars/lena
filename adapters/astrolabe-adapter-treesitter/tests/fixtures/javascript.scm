;; Standard functions
(function_declaration name: (identifier) @symbol.function)

;; Class methods (e.g., async handle() { ... })
(method_definition name: (property_identifier) @symbol.function)

;; Arrow functions assigned to constants (const foo = () => {})
(variable_declarator 
    name: (identifier) @symbol.function 
    value: (arrow_function))

;; Call sites
(call_expression function: (identifier) @call.callee)
(call_expression function: (member_expression property: (property_identifier) @call.callee))
