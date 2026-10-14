//! Standard Recursive Datalog Rules for Lena

/// Predefined Datalog rules for transitive graph traversal.
pub const STANDARD_RULES: &str = r"
[
  [(calls_rule ?caller ?callee)
   [?cs :entity/type :type/call-site]
   [?cs :call/caller ?caller]
   [?cs :call/callee ?callee]]

  [(calls_transitive ?caller ?callee)
   [?cs :entity/type :type/call-site]
   [?cs :call/caller ?caller]
   [?cs :call/callee ?callee]]

  [(calls_transitive ?caller ?callee)
   [?cs :entity/type :type/call-site]
   [?cs :call/caller ?caller]
   [?cs :call/callee ?intermediate]
   (calls_transitive ?intermediate ?callee)]

  [(parent_rule ?p ?c)
   [?c :parent/entity ?p]]

  [(parent_transitive ?p ?c)
   [?c :parent/entity ?p]]

  [(parent_transitive ?p ?c)
   [?intermediate :parent/entity ?p]
   (parent_transitive ?intermediate ?c)]
]
";
