use novadraw_editor::SelectionModel;

#[test]
fn ordered_selection_keeps_the_last_selected_item_primary() {
    let mut selection = SelectionModel::new();

    selection.replace(1_u64);
    selection.append(2);
    selection.append(1);

    assert_eq!(selection.items(), &[2, 1]);
    assert_eq!(selection.primary(), Some(1));
}

#[test]
fn toggle_and_clear_report_exact_selection_deltas() {
    let mut selection = SelectionModel::new();
    selection.replace(1_u64);
    selection.append(2);

    let removed = selection.toggle(1).unwrap();
    assert_eq!(removed.removed(), &[1]);
    assert!(removed.added().is_empty());
    assert_eq!(removed.primary(), Some(2));

    let cleared = selection.clear().unwrap();
    assert_eq!(cleared.removed(), &[2]);
    assert_eq!(cleared.primary(), None);
    assert!(selection.is_empty());
}

#[test]
fn focus_is_independent_but_reconciled_with_retired_parts() {
    let mut selection = SelectionModel::new();
    selection.replace(1_u64);
    selection.append(2);
    assert!(selection.set_focus(Some(1)));

    let delta = selection.reconcile(|part| part != 1).unwrap();

    assert_eq!(selection.items(), &[2]);
    assert_eq!(selection.primary(), Some(2));
    assert_eq!(selection.focus(), None);
    assert_eq!(delta.removed(), &[1]);
}
