use draw2d_official_examples::catalog;
use std::{collections::BTreeSet, fs, path::PathBuf};

const INVENTORY: &str = include_str!("../source-contracts.tsv");
const EVENTS: &str = include_str!("../source-events.tsv");
const COMPLETION: &str = include_str!("../completion-contracts.tsv");
const GRAPH_FIXTURES: &str =
    include_str!("../../../../verification/evidence/draw2d-official-examples/graph-fixtures.tsv");
const PLAN: &str = include_str!(
    "../../../../doc/verification/plans/draw2d-official-examples-remaining-contracts.md"
);

fn rows() -> Vec<Vec<&'static str>> {
    INVENTORY
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| line.split('\t').collect())
        .collect()
}

#[test]
fn inventory_covers_catalog_and_plan_exactly_with_owned_steps() {
    let rows = rows();
    let catalog = catalog();
    assert_eq!(rows.len(), catalog.len());
    let mut names = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for row in rows {
        assert_eq!(row.len(), 5);
        assert!(names.insert(row[0]));
        assert!(paths.insert(row[1]));
        let entry = catalog
            .iter()
            .find(|entry| entry.name == row[0])
            .expect("catalog entry");
        assert_eq!(
            entry.upstream_path,
            format!("org/eclipse/draw2d/examples/{}", row[1])
        );
        let plan_row = PLAN
            .lines()
            .find(|line| line.starts_with(&format!("| {} ·", row[0])))
            .expect("plan row");
        for step in row[4].split(',') {
            assert!(plan_row.contains(step), "{} missing {step}", row[0]);
            assert!(step.starts_with('K') || step.starts_with('H'));
        }
        assert!(!row[2].contains("zest"));
    }
}

#[test]
fn completion_scopes_cover_all_entries_and_preserve_every_dependency() {
    let inventory = rows();
    let contracts: Vec<_> = COMPLETION
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| line.split('\t').collect::<Vec<_>>())
        .collect();
    assert_eq!(contracts.len(), inventory.len());
    let mut names = BTreeSet::new();
    for contract in contracts {
        assert_eq!(contract.len(), 4);
        assert!(names.insert(contract[0]));
        assert!(
            [
                "base",
                "connection",
                "tree",
                "text",
                "graph",
                "view",
                "output"
            ]
            .contains(&contract[1])
        );
        let source = inventory.iter().find(|row| row[0] == contract[0]).unwrap();
        assert_eq!(contract[2], source[4]);
        assert!(contract[3].split(';').count() >= 3);
        assert!(catalog().iter().any(|entry| entry.name == contract[0]));
    }
}

#[test]
fn graph_baseline_accounts_for_every_menu_entry_and_both_directions() {
    let mut menus = std::collections::BTreeMap::<&str, Vec<&str>>::new();
    let mut outcomes = BTreeSet::new();
    let mut node_counts = std::collections::BTreeMap::new();
    let mut edge_counts = std::collections::BTreeMap::new();
    let mut expected_counts = std::collections::BTreeMap::new();
    for line in GRAPH_FIXTURES.lines() {
        let row: Vec<_> = line.split('\t').collect();
        if row[0] == "menu" {
            let menu = menus.entry(row[1]).or_default();
            assert_eq!(row[2].parse::<usize>().unwrap(), menu.len());
            assert!(!menu.contains(&row[3]));
            menu.push(row[3]);
            continue;
        }
        let key = (row[1], row[2], row[3]);
        assert!(["4", "16"].contains(&row[3]));
        match row[0] {
            "fixture" => {
                assert!(outcomes.insert(key));
                expected_counts.insert(
                    key,
                    (
                        row[4].parse::<usize>().unwrap(),
                        row[5].parse::<usize>().unwrap(),
                    ),
                );
            }
            "failure" => {
                assert!(outcomes.insert(key));
                assert!(!row[4].is_empty() && !row[5].is_empty());
            }
            "node" => *node_counts.entry(key).or_insert(0usize) += 1,
            "edge" => *edge_counts.entry(key).or_insert(0usize) += 1,
            kind => panic!("unknown graph evidence row {kind}"),
        }
    }
    assert_eq!(menus["directed"].len(), 16);
    assert_eq!(menus["compound"].len(), 10);
    assert_eq!(outcomes.len(), 52);
    for (kind, methods) in menus {
        for method in methods {
            for direction in ["4", "16"] {
                assert!(outcomes.contains(&(kind, method, direction)));
            }
        }
    }
    for (key, (nodes, edges)) in expected_counts {
        assert_eq!(node_counts.get(&key).copied().unwrap_or(0), nodes);
        assert_eq!(edge_counts.get(&key).copied().unwrap_or(0), edges);
    }
}

#[test]
fn event_source_evidence_covers_every_main_and_inventoried_helper() {
    let expected: BTreeSet<_> = rows()
        .iter()
        .flat_map(|row| row[1..3].iter().flat_map(|field| field.split(',')))
        .filter(|path| *path != "-")
        .collect();
    let mut sources = BTreeSet::new();
    for line in EVENTS.lines().filter(|line| !line.starts_with('#')) {
        let fields: Vec<_> = line.splitn(4, '\t').collect();
        assert_eq!(fields.len(), 4);
        assert!(expected.contains(fields[0]));
        assert!(!fields[3].contains("org.eclipse.zest"));
        match fields[2] {
            "sha256" => {
                assert_eq!(fields[1], "0");
                assert_eq!(fields[3].len(), 64);
                assert!(fields[3].bytes().all(|byte| byte.is_ascii_hexdigit()));
                assert!(sources.insert(fields[0]));
            }
            "registration" | "callback" | "platform" | "host" => {
                assert!(fields[1].parse::<usize>().unwrap() > 0);
                assert!(!fields[3].is_empty());
            }
            kind => panic!("unknown source evidence kind: {kind}"),
        }
    }
    assert_eq!(sources, expected);
}

/// Exact local upstream validation; ordinary workspace tests do not require the sibling checkout.
#[test]
#[ignore = "requires DRAW2D_REFERENCE_ROOT pointing to a fixed upstream source checkout"]
fn upstream_main_helpers_and_resources_match_the_inventory() {
    let root =
        PathBuf::from(std::env::var("DRAW2D_REFERENCE_ROOT").expect("DRAW2D_REFERENCE_ROOT"));
    let base = root.join("org.eclipse.draw2d.examples/src/org/eclipse/draw2d/examples");
    let mut discovered = BTreeSet::new();
    let mut pending = vec![base.clone()];
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|ext| ext == "java") {
                let text = fs::read_to_string(&path).unwrap();
                assert!(!text.contains("import org.eclipse.gef"));
                assert!(!text.contains("import org.eclipse.zest"));
                if text.contains("public static void main(") {
                    discovered.insert(
                        path.strip_prefix(&base)
                            .unwrap()
                            .to_string_lossy()
                            .to_string(),
                    );
                }
            }
        }
    }
    let rows = rows();
    let expected: BTreeSet<_> = rows.iter().map(|row| row[1].to_owned()).collect();
    assert_eq!(expected, discovered);
    for row in rows {
        for field in &row[1..4] {
            for relative in field.split(',').filter(|path| *path != "-") {
                assert!(
                    base.join(relative).is_file(),
                    "{} missing {relative}",
                    row[0]
                );
            }
        }
    }
    for line in EVENTS.lines().filter(|line| !line.starts_with('#')) {
        let fields: Vec<_> = line.splitn(4, '\t').collect();
        if fields[2] == "sha256" {
            continue;
        }
        let source = fs::read_to_string(base.join(fields[0])).unwrap();
        let number = fields[1].parse::<usize>().unwrap();
        assert_eq!(
            source
                .lines()
                .nth(number - 1)
                .unwrap()
                .trim()
                .replace('\t', " "),
            fields[3],
            "{}:{number}",
            fields[0],
        );
    }
    for (file, kind, expected) in [
        ("graph/GraphTests.java", "DirectedGraph", 16),
        ("graph/CompoundGraphTests.java", "CompoundDirectedGraph", 10),
    ] {
        let text = fs::read_to_string(base.join(file)).unwrap();
        let marker = format!("public static {kind} ");
        assert_eq!(
            text.lines()
                .filter(|line| line.contains(&marker) && line.contains("(int direction)"))
                .count(),
            expected
        );
    }
}
