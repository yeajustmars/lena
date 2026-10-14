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

        // Single-hop query for direct callees
        let query = format!(
            "[:find ?callee_fqsn :where [?caller :symbol/fqsn \"{current_fqsn}\"] (calls_rule ?caller ?callee) [?callee :symbol/fqsn ?callee_fqsn]]"
        );

        if let Ok(json_result) = db.query_with_rules(&query).await
            && let Ok(rows) = serde_json::from_str::<Vec<serde_json::Value>>(&json_result)
        {
            for row in rows {
                let callee = if let Some(arr) = row.as_array() {
                    arr.first().and_then(|v| v.as_str())
                } else if let Some(obj) = row.as_object() {
                    obj.values().next().and_then(|v| v.as_str())
                } else {
                    None
                };

                if let Some(callee_str) = callee {
                    edges.push(Edge {
                        caller: current_fqsn.clone(),
                        callee: callee_str.to_string(),
                    });
                    queue.push_back((callee_str.to_string(), depth + 1));
                }
            }
        }
    }

    Ok(format_skeleton(root_fqsn, &edges, format))
}

fn format_skeleton(root: &str, edges: &[Edge], format: SkeletonFormat) -> String {
    match format {
        SkeletonFormat::Mermaid => {
            let mut out = String::from("```mermaid\ngraph TD\n");
            for edge in edges {
                out.push_str(&format!(
                    "    \"{}\" --> \"{}\"\n",
                    edge.caller, edge.callee
                ));
            }
            if edges.is_empty() {
                out.push_str(&format!("    \"{}\"\n", root));
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
            build_indented(root, edges, 0, &mut path_visited, &mut out);
            out.trim_end().to_string()
        }
    }
}

fn build_indented(
    node: &str,
    edges: &[Edge],
    depth: usize,
    path_visited: &mut HashSet<String>,
    out: &mut String,
) {
    let indent = "  ".repeat(depth);
    if path_visited.contains(node) {
        out.push_str(&format!("{indent}└── {node} (recursive cycle)\n"));
        return;
    }

    out.push_str(&format!("{indent}└── {node}\n"));
    path_visited.insert(node.to_string());

    for edge in edges.iter().filter(|e| e.caller == node) {
        build_indented(&edge.callee, edges, depth + 1, path_visited, out);
    }

    path_visited.remove(node);
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
        let mermaid = format_skeleton("A", &edges, SkeletonFormat::Mermaid);
        assert!(mermaid.contains("graph TD"));
        assert!(mermaid.contains("\"A\" --> \"B\""));
        assert!(mermaid.contains("\"A\" --> \"C\""));
    }

    #[test]
    fn test_format_dot() {
        let edges = vec![Edge {
            caller: "A".into(),
            callee: "B".into(),
        }];
        let dot = format_skeleton("A", &edges, SkeletonFormat::Dot);
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
        let indented = format_skeleton("A", &edges, SkeletonFormat::Indented);
        assert!(indented.contains("└── A"));
        assert!(indented.contains("  └── B"));
        assert!(indented.contains("    └── A (recursive cycle)"));
    }

    #[test]
    fn test_format_indented_empty() {
        let indented = format_skeleton("RootOnly", &[], SkeletonFormat::Indented);
        assert_eq!(indented, "└── RootOnly");
    }

    #[tokio::test]
    async fn test_generate_skeleton_depth_and_query() {
        let dir = tempdir().unwrap();
        let db = Arc::new(LenaDb::open(dir.path().join("skeleton.db")).unwrap());

        // Use the native EDN ingestion pipeline to guarantee identical internal representations
        let edn = b"\
            [[:db/add 10 :symbol/fqsn \"root\"]]\n\
            [[:db/add 11 :symbol/fqsn \"child1\"]]\n\
            [[:db/add 12 :symbol/fqsn \"child2\"]]\n\
            [[:db/add 20 :entity/type :type/call-site] [:db/add 20 :call/caller 10] [:db/add 20 :call/callee 11]]\n\
            [[:db/add 21 :entity/type :type/call-site] [:db/add 21 :call/caller 11] [:db/add 21 :call/callee 12]]\n\
        ";

        crate::ingest::ingest_stream(&db, &edn[..]).await.unwrap();

        // Depth 1: Should only traverse from root to child1.
        let skel_d1 = generate_skeleton(&db, "root", 1, SkeletonFormat::Indented)
            .await
            .unwrap();
        assert!(skel_d1.contains("└── root"));
        assert!(skel_d1.contains("  └── child1"));
        assert!(
            !skel_d1.contains("child2"),
            "Depth 1 should not reveal child2"
        );

        // Depth 2: Should traverse all the way to child2.
        let skel_d2 = generate_skeleton(&db, "root", 2, SkeletonFormat::Indented)
            .await
            .unwrap();
        assert!(
            skel_d2.contains("    └── child2"),
            "Depth 2 should reveal child2"
        );
    }
}
