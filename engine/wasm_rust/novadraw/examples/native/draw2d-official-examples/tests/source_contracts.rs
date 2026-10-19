use draw2d_official_examples::catalog;
use std::{collections::BTreeSet, fs, path::PathBuf};

const INVENTORY: &str = include_str!("../source-contracts.tsv");
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
