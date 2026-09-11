use std::collections::HashMap;

pub type Graph = HashMap<String, Vec<String>>;

/// Build dependency graph and its reverse.
/// graph[A] = [B, C] means A depends on B and C.
/// reverse[B] = [A] means A depends on B.
pub fn build_graph(projects: &[(String, Vec<String>)]) -> (Graph, Graph) {
    let mut graph: Graph = HashMap::new();
    let mut reverse: Graph = HashMap::new();

    for (name, deps) in projects {
        graph.insert(name.clone(), deps.clone());
        for dep in deps {
            reverse.entry(dep.clone()).or_default().push(name.clone());
        }
    }

    // Ensure all nodes exist in both maps
    for (name, _) in projects {
        graph.entry(name.clone()).or_default();
        reverse.entry(name.clone()).or_default();
    }

    (graph, reverse)
}

/// Detect cycles using tricolor DFS.
/// Returns the cycle path and true if a cycle is found.
pub fn detect_cycles(graph: &Graph) -> (Vec<String>, bool) {
    const WHITE: u8 = 0;
    const GRAY: u8 = 1;
    const BLACK: u8 = 2;

    let mut color: HashMap<String, u8> = HashMap::new();
    let mut parent: HashMap<String, String> = HashMap::new();

    fn dfs(
        node: &str,
        graph: &Graph,
        color: &mut HashMap<String, u8>,
        parent: &mut HashMap<String, String>,
    ) -> Option<Vec<String>> {
        color.insert(node.to_string(), GRAY);

        if let Some(deps) = graph.get(node) {
            for dep in deps {
                let dep_color = *color.get(dep.as_str()).unwrap_or(&WHITE);
                if dep_color == GRAY {
                    // Cycle detected, reconstruct path
                    let mut cycle = vec![dep.clone()];
                    let mut cur = node.to_string();
                    while cur != *dep {
                        cycle.push(cur.clone());
                        cur = parent.get(&cur).cloned().unwrap_or_default();
                    }
                    cycle.push(dep.clone());
                    cycle.reverse();
                    return Some(cycle);
                }
                if dep_color == WHITE {
                    parent.insert(dep.clone(), node.to_string());
                    if let Some(c) = dfs(dep, graph, color, parent) {
                        return Some(c);
                    }
                }
            }
        }

        color.insert(node.to_string(), BLACK);
        None
    }

    for node in graph.keys() {
        let c = *color.get(node.as_str()).unwrap_or(&WHITE);
        if c == WHITE {
            if let Some(cycle) = dfs(node, graph, &mut color, &mut parent) {
                return (cycle, true);
            }
        }
    }

    (vec![], false)
}

/// Topological sort using Kahn's algorithm.
/// Returns startup order for given targets including all transitive dependencies.
pub fn topological_sort(targets: &[String], graph: &Graph) -> Result<Vec<String>, String> {
    // Collect all needed nodes (targets + transitive deps)
    let mut needed: HashMap<String, bool> = HashMap::new();
    fn collect(name: &str, graph: &Graph, needed: &mut HashMap<String, bool>) {
        if needed.contains_key(name) {
            return;
        }
        needed.insert(name.to_string(), true);
        if let Some(deps) = graph.get(name) {
            for dep in deps {
                collect(dep, graph, needed);
            }
        }
    }
    for t in targets {
        collect(t, graph, &mut needed);
    }

    // Build in-degree for the subgraph
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    for node in needed.keys() {
        in_degree.entry(node.clone()).or_insert(0);
        if let Some(deps) = graph.get(node) {
            let count = deps.iter().filter(|d| needed.contains_key(d.as_str())).count();
            in_degree.insert(node.clone(), count);
        }
    }

    // Kahn's queue
    let mut queue: Vec<String> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(n, _)| n.clone())
        .collect();
    queue.sort(); // deterministic order

    let mut sorted = Vec::new();
    while let Some(node) = queue.first().cloned() {
        queue.remove(0);
        sorted.push(node.clone());

        for (other, _) in &needed {
            if let Some(deps) = graph.get(other) {
                if deps.contains(&node) {
                    if let Some(deg) = in_degree.get_mut(other) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push(other.clone());
                            queue.sort();
                        }
                    }
                }
            }
        }
    }

    if sorted.len() != needed.len() {
        return Err("cycle detected in dependencies".to_string());
    }

    Ok(sorted)
}

pub fn format_cycle(cycle: &[String]) -> String {
    cycle.join(" -> ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_graph() {
        let projects = vec![
            ("devbox".into(), vec![]),
            ("app1".into(), vec!["devbox".into()]),
            ("app2".into(), vec!["devbox".into(), "app1".into()]),
        ];
        let (graph, reverse) = build_graph(&projects);

        assert!(graph["devbox"].is_empty());
        assert_eq!(graph["app1"], vec!["devbox"]);
        assert_eq!(graph["app2"].len(), 2);
        assert_eq!(reverse["devbox"].len(), 2);
    }

    #[test]
    fn test_detect_cycles_no_cycle() {
        let mut graph = Graph::new();
        graph.insert("devbox".into(), vec![]);
        graph.insert("app1".into(), vec!["devbox".into()]);
        graph.insert("app2".into(), vec!["devbox".into(), "app1".into()]);

        let (_, has_cycle) = detect_cycles(&graph);
        assert!(!has_cycle);
    }

    #[test]
    fn test_detect_cycles_with_cycle() {
        let mut graph = Graph::new();
        graph.insert("a".into(), vec!["b".into()]);
        graph.insert("b".into(), vec!["c".into()]);
        graph.insert("c".into(), vec!["a".into()]);

        let (cycle, has_cycle) = detect_cycles(&graph);
        assert!(has_cycle);
        assert!(cycle.len() >= 3);
    }

    #[test]
    fn test_detect_cycles_self_loop() {
        let mut graph = Graph::new();
        graph.insert("a".into(), vec!["a".into()]);

        let (_, has_cycle) = detect_cycles(&graph);
        assert!(has_cycle);
    }

    #[test]
    fn test_topological_sort_simple() {
        let mut graph = Graph::new();
        graph.insert("devbox".into(), vec![]);
        graph.insert("app1".into(), vec!["devbox".into()]);
        graph.insert("app2".into(), vec!["devbox".into(), "app1".into()]);

        let sorted = topological_sort(&["app2".into()], &graph).unwrap();
        let idx = |n: &str| sorted.iter().position(|x| x == n).unwrap();

        assert!(idx("devbox") < idx("app1"));
        assert!(idx("app1") < idx("app2"));
        assert_eq!(sorted.len(), 3);
    }

    #[test]
    fn test_topological_sort_single_node() {
        let mut graph = Graph::new();
        graph.insert("devbox".into(), vec![]);
        graph.insert("app1".into(), vec!["devbox".into()]);

        let sorted = topological_sort(&["devbox".into()], &graph).unwrap();
        assert_eq!(sorted, vec!["devbox"]);
    }

    #[test]
    fn test_topological_sort_multiple_targets() {
        let mut graph = Graph::new();
        graph.insert("devbox".into(), vec![]);
        graph.insert("app1".into(), vec!["devbox".into()]);
        graph.insert("app2".into(), vec!["devbox".into()]);

        let sorted = topological_sort(&["app1".into(), "app2".into()], &graph).unwrap();
        assert_eq!(sorted.len(), 3);
        assert_eq!(sorted[0], "devbox");
    }
}
