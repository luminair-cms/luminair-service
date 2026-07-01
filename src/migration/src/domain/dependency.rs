use std::collections::{HashMap, HashSet, VecDeque};

use crate::domain::tables::Table;

#[derive(Debug, thiserror::Error)]
pub enum DependencyError {
    #[error("Circular dependency detected involving tables: {0:?}")]
    CircularDependency(Vec<String>),
}

pub struct DependencyGraph<'a> {
    /// Map of table names to their corresponding Table structures
    pub table_map: HashMap<&'a str, &'a Table>,
    /// Adjacency list mapping a table name to all other tables that depend on it (i.e. reference it)
    pub dependents: HashMap<&'a str, Vec<&'a str>>,
    /// Map tracking the in-degree count (i.e. how many tables a given table directly depends on/references)
    pub in_degree: HashMap<&'a str, usize>,
}

pub fn build_dependency_graph<'a>(
    tables: &'a [Table],
) -> DependencyGraph<'a> {
    // Map table name -> Table reference
    let table_map: HashMap<&str, &Table> = tables
        .iter()
        .map(|t| (t.name.as_str(), t))
        .collect();

    // Build adjacency list: dependency -> dependents (who depends on it)
    // and in-degree count (how many tables this table depends on)
    let mut dependents: HashMap<&str, Vec<&str>> = tables
        .iter()
        .map(|t| (t.name.as_str(), vec![]))
        .collect();

    let mut in_degree: HashMap<&str, usize> = tables
        .iter()
        .map(|t| (t.name.as_str(), 0))
        .collect();

    for table in tables {
        let deps: HashSet<&str> = table
            .foreign_keys
            .iter()
            .map(|fk| fk.referenced_table_name.as_str())
            // Skip self-references
            .filter(|&dep| dep != table.name.as_str())
            // Skip references to tables not in our set
            .filter(|dep| table_map.contains_key(dep))
            .collect();

        for dep in deps {
            dependents.get_mut(dep).unwrap().push(table.name.as_str());
            *in_degree.get_mut(table.name.as_str()).unwrap() += 1;
        }
    }

    DependencyGraph {
        table_map,
        dependents,
        in_degree,
    }
}

pub fn resolve_table_order(tables: &[Table]) -> Result<Vec<&Table>, DependencyError> {
    let DependencyGraph { table_map, dependents, mut in_degree } = build_dependency_graph(tables);

    // Kahn's algorithm: start with tables that have no dependencies
    let mut queue: VecDeque<&str> = in_degree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(&name, _)| name)
        .collect();

    let mut ordered = Vec::with_capacity(tables.len());

    while let Some(name) = queue.pop_front() {
        ordered.push(table_map[name]);

        for &dependent in &dependents[name] {
            let degree = in_degree.get_mut(dependent).unwrap();
            *degree -= 1;
            if *degree == 0 {
                queue.push_back(dependent);
            }
        }
    }

    // If not all tables were processed, there's a cycle
    if ordered.len() != tables.len() {
        let cycle_tables: Vec<String> = in_degree
            .iter()
            .filter(|(_, degree)| **degree > 0)
            .map(|(&name, _)| name.to_owned())
            .collect();

        return Err(DependencyError::CircularDependency(cycle_tables));
    }

    Ok(ordered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tables::ForeignKeyConstraint;

    fn make_table(name: &str, deps: Vec<&str>) -> Table {
        let fks = deps
            .into_iter()
            .map(|dep| ForeignKeyConstraint::new(name, "fk_col", dep, "id"))
            .collect();
        Table::new(name.to_string(), vec![], fks, vec![])
    }

    #[test]
    fn test_no_dependencies() {
        let t1 = make_table("t1", vec![]);
        let t2 = make_table("t2", vec![]);
        let tables = vec![t1, t2];
        let ordered = resolve_table_order(&tables).unwrap();
        assert_eq!(ordered.len(), 2);
    }

    #[test]
    fn test_linear_dependencies() {
        // t2 depends on t1
        let t1 = make_table("t1", vec![]);
        let t2 = make_table("t2", vec!["t1"]);
        let tables = vec![t2.clone(), t1.clone()]; // out of order
        let ordered = resolve_table_order(&tables).unwrap();
        assert_eq!(ordered[0].name, "t1");
        assert_eq!(ordered[1].name, "t2");
    }

    #[test]
    fn test_circular_dependency() {
        // t1 depends on t2, and t2 depends on t1
        let t1 = make_table("t1", vec!["t2"]);
        let t2 = make_table("t2", vec!["t1"]);
        let tables = vec![t1, t2];
        let err = resolve_table_order(&tables).unwrap_err();
        assert!(matches!(err, DependencyError::CircularDependency(_)));
    }
}