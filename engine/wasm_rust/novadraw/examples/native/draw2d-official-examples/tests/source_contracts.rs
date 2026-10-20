use draw2d_official_examples::catalog;
use std::collections::BTreeSet;

const INVENTORY: &str = include_str!("../source-contracts.tsv");
const COMPLETION: &str = include_str!("../completion-contracts.tsv");

fn rows(text: &str) -> Vec<Vec<&str>> {
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| line.split('\t').collect())
        .collect()
}

#[test]
fn inventory_covers_every_official_canvas_entry() {
    let inventory = rows(INVENTORY);
    let entries = catalog();
    assert_eq!(inventory.len(), entries.len());
    let mut names = BTreeSet::new();
    for row in inventory {
        assert_eq!(row.len(), 5);
        assert!(names.insert(row[0]));
        let entry = entries.iter().find(|entry| entry.name == row[0]).unwrap();
        assert_eq!(
            entry.upstream_path,
            format!("org/eclipse/draw2d/examples/{}", row[1])
        );
        assert!(!row[2].contains("zest"));
    }
}

#[test]
fn completion_scopes_cover_all_entries_and_preserve_canvas_dependencies() {
    let inventory = rows(INVENTORY);
    let contracts = rows(COMPLETION);
    assert_eq!(contracts.len(), inventory.len());
    let mut names = BTreeSet::new();
    for contract in contracts {
        assert_eq!(contract.len(), 4);
        assert!(names.insert(contract[0]));
        let source = inventory.iter().find(|row| row[0] == contract[0]).unwrap();
        assert_eq!(contract[2], source[4]);
        assert!(!contract[2].contains("H03"));
        assert!(contract[3].split(';').count() >= 3);
    }
}
