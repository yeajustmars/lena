//! Standard Recursive Datalog Rules for Astrolabe

/// Predefined Datalog rules for transitive graph traversal.
pub const STANDARD_RULES: &str = r#"
[
  ;; Direct call rule
  [(calls? ?caller ?callee)
   [?cs :entity/type :type/call-site]
   [?cs :call/caller ?caller]
   [?cs :call/callee ?callee]]

  ;; Transitive call rule (Recursive)
  [(calls-transitive? ?caller ?callee)
   (calls? ?caller ?callee)]

  [(calls-transitive? ?caller ?callee)
   (calls? ?caller ?intermediate)
   (calls-transitive? ?intermediate ?callee)]

  ;; Direct parentage rule (modules -> functions)
  [(parent? ?p ?c)
   [?c :parent/entity ?p]]

  ;; Transitive parentage rule (e.g., nested modules/classes)
  [(parent-transitive? ?p ?c)
   (parent? ?p ?c)]

  [(parent-transitive? ?p ?c)
   (parent? ?p ?intermediate)
   (parent-transitive? ?intermediate ?c)]
]
"#;
