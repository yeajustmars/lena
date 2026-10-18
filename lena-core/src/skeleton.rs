//! Call Graph Skeleton Generator

use crate::db::LenaDb;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SkeletonError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Failed to parse entity data: {0}")]
    Parse(String),
}

#[derive(Debug, Clone, Copy)]
pub enum SkeletonFormat {
    Indented,
    Mermaid,
    Dot,
}

#[derive(Debug, Clone)]
pub struct Edge {
    pub caller: String,
    pub callee: String,
}

/// Iteratively queries the database to build a bounded subgraph, preventing infinite loops on cycles.
pub async fn generate_skeleton(
    db: &Arc<LenaDb>,
    root_fqsn: &str,
    max_depth: usize,
    format: SkeletonFormat,
    invert: bool, // NEW: if true, traverse upwards (callers)
) -> Result<String, SkeletonError> {
    let mut edges: Vec<Edge> = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();

    queue.push_back((root_fqsn.to_string(), 0));

    while let Some((current_fqsn, depth)) = queue.pop_front() {
        if depth >= max_depth || visited.contains(&current_fqsn) {
            continue;
        }
        visited.insert(current_fqsn.clone());

        // Select the traversal direction
        let query = if invert {
            // Find who calls the current symbol
            format!(
                "[:find ?caller_fqsn :where [?callee :symbol/fqsn \"{current_fqsn}\"] (calls_rule ?caller ?callee) [?caller :symbol/fqsn ?caller_fqsn]]"
            )
        } else {
            // Find who the current symbol calls
            format!(
                "[:find ?callee_fqsn :where [?caller :symbol/fqsn \"{current_fqsn}\"] (calls_rule ?caller ?callee) [?callee :symbol/fqsn ?callee_fqsn]]"
            )
        };

        if let Ok(json_result) = db.query_with_rules(&query).await
            && let Ok(rows) = serde_json::from_str::<Vec<serde_json::Value>>(&json_result)
        {
            for row in rows {
                let adjacent = if let Some(arr) = row.as_array() {
                    arr.first().and_then(|v| v.as_str())
                } else if let Some(obj) = row.as_object() {
                    obj.values().next().and_then(|v| v.as_str())
                } else {
                    None
                };

                if let Some(adjacent_str) = adjacent {
                    if invert {
                        // The discovered node is the Caller, current is Callee
                        edges.push(Edge {
                            caller: adjacent_str.to_string(),
                            callee: current_fqsn.clone(),
                        });
                    } else {
                        // Current is Caller, discovered node is Callee
                        edges.push(Edge {
                            caller: current_fqsn.clone(),
                            callee: adjacent_str.to_string(),
                        });
                    }
                    queue.push_back((adjacent_str.to_string(), depth + 1));
                }
            }
        }
    }

    Ok(format_skeleton(root_fqsn, &edges, format, invert))
}

fn format_skeleton(root: &str, edges: &[Edge], format: SkeletonFormat, invert: bool) -> String {
    match format {
        SkeletonFormat::Mermaid => {
            let mut out = String::from("```mermaid\ngraph TD\n");
            for edge in edges {
                let caller_id = sanitize_node_id(&edge.caller);
                let callee_id = sanitize_node_id(&edge.callee);
                out.push_str(&format!(
                    "    {caller_id}[\"{}\"] --> {callee_id}[\"{}\"]\n",
                    edge.caller, edge.callee
                ));
            }
            if edges.is_empty() {
                let root_id = sanitize_node_id(root);
                out.push_str(&format!("    {root_id}[\"{root}\"]\n"));
            }
            out.push_str("```");
            out
        }
        SkeletonFormat::Dot => {
            let mut out = String::from("digraph G {\n");
            for edge in edges {
                out.push_str(&format!(
                    "    \"{}\" -> \"{}\";\n",
                    edge.caller, edge.callee
                ));
            }
            if edges.is_empty() {
                out.push_str(&format!("    \"{}\";\n", root));
            }
            out.push('}');
            out
        }
        SkeletonFormat::Indented => {
            let mut out = String::new();
            let mut path_visited = HashSet::new();
            build_indented(root, root, edges, 0, &mut path_visited, &mut out, invert);
            out.trim_end().to_string()
        }
    }
}

fn build_indented(
    root: &str,
    node: &str,
    edges: &[Edge],
    depth: usize,
    path_visited: &mut HashSet<String>,
    out: &mut String,
    invert: bool,
) {
    let indent = "  ".repeat(depth);
    let label = if node == root {
        format!("\x1b[1m{node}\x1b[0m")
    } else {
        node.to_string()
    };

    if path_visited.contains(node) {
        out.push_str(&format!("{indent}└── {label} (recursive cycle)\n"));
        return;
    }

    out.push_str(&format!("{indent}└── {label}\n"));
    path_visited.insert(node.to_string());

    if invert {
        // Walk UP the graph (who called this node)
        for edge in edges.iter().filter(|e| e.callee == node) {
            build_indented(
                root,
                &edge.caller,
                edges,
                depth + 1,
                path_visited,
                out,
                invert,
            );
        }
    } else {
        // Walk DOWN the graph (who this node calls)
        for edge in edges.iter().filter(|e| e.caller == node) {
            build_indented(
                root,
                &edge.callee,
                edges,
                depth + 1,
                path_visited,
                out,
                invert,
            );
        }
    }

    path_visited.remove(node);
}

fn sanitize_node_id(fqsn: &str) -> String {
    let mut safe = String::with_capacity(fqsn.len() + 3);
    safe.push_str("id_");
    for ch in fqsn.chars() {
        match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' => safe.push(ch),
            _ => safe.push('_'),
        }
    }
    safe
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_format_mermaid() {
        let edges = vec![
            Edge {
                caller: "A".into(),
                callee: "B".into(),
            },
            Edge {
                caller: "A".into(),
                callee: "C".into(),
            },
        ];
        let mermaid = format_skeleton("A", &edges, SkeletonFormat::Mermaid, false);
        assert!(mermaid.contains("graph TD"));
        assert!(mermaid.contains("id_A[\"A\"] --> id_B[\"B\"]"));
        assert!(mermaid.contains("id_A[\"A\"] --> id_C[\"C\"]"));
    }

    #[test]
    fn test_format_dot() {
        let edges = vec![Edge {
            caller: "A".into(),
            callee: "B".into(),
        }];
        let dot = format_skeleton("A", &edges, SkeletonFormat::Dot, false);
        assert!(dot.contains("digraph G {"));
        assert!(dot.contains("\"A\" -> \"B\";"));
    }

    #[test]
    fn test_format_indented_cycle() {
        let edges = vec![
            Edge {
                caller: "A".into(),
                callee: "B".into(),
            },
            Edge {
                caller: "B".into(),
                callee: "A".into(),
            },
        ];
        let indented = format_skeleton("A", &edges, SkeletonFormat::Indented, false);
        assert!(indented.contains("└── \x1b[1mA\x1b[0m"));
        assert!(indented.contains("  └── B"), "Callee B must not be bold");
        assert!(
            indented.contains("    └── \x1b[1mA\x1b[0m (recursive cycle)"),
            "Cycle of A must be bold"
        );
    }

    #[test]
    fn test_format_indented_empty() {
        let indented = format_skeleton("RootOnly", &[], SkeletonFormat::Indented, false);
        assert_eq!(indented, "└── \x1b[1mRootOnly\x1b[0m");
    }

    #[tokio::test]
    async fn test_generate_skeleton_depth_and_query() {
        let dir = tempdir().unwrap();
        let db = Arc::new(LenaDb::open(dir.path().join("skeleton.db")).unwrap());

        let edn = b"\
            [[:db/add 10 :symbol/fqsn \"root\"]]\n\
            [[:db/add 11 :symbol/fqsn \"child1\"]]\n\
            [[:db/add 12 :symbol/fqsn \"child2\"]]\n\
            [[:db/add 20 :entity/type :type/call-site] [:db/add 20 :call/caller 10] [:db/add 20 :call/callee 11]]\n\
            [[:db/add 21 :entity/type :type/call-site] [:db/add 21 :call/caller 11] [:db/add 21 :call/callee 12]]\n\
        ";

        crate::ingest::ingest_stream(&db, &edn[..]).await.unwrap();

        // Depth 1: Root is bold; child1 is normal text
        let skel_d1 = generate_skeleton(&db, "root", 1, SkeletonFormat::Indented, false)
            .await
            .unwrap();
        assert!(skel_d1.contains("└── \x1b[1mroot\x1b[0m"));
        assert!(skel_d1.contains("  └── child1"));
        assert!(
            !skel_d1.contains("child2"),
            "Depth 1 should not reveal child2"
        );

        // Depth 2: Should traverse all the way to child2
        let skel_d2 = generate_skeleton(&db, "root", 2, SkeletonFormat::Indented, false)
            .await
            .unwrap();
        assert!(
            skel_d2.contains("    └── child2"),
            "Depth 2 should reveal child2"
        );
    }
}
