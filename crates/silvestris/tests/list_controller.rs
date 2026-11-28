//! `ListController` reload and selection invariants: ids, not indices, are
//! the contract, so a reload prunes ids that vanished while surviving ids keep
//! their order and the anchor is repaired.

use silvestris::binding::{ItemId, ListController};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Row {
    id: u32,
    name: &'static str,
}

fn row(id: u32, name: &'static str) -> Row {
    Row { id, name }
}

fn controller(items: &[u32]) -> ListController<Row> {
    let mut controller = ListController::new(|row: &Row| row.id.to_string().into());
    controller.set_items(items.iter().map(|id| row(*id, "row")).collect());
    controller
}

fn selected(controller: &ListController<Row>) -> Vec<String> {
    controller
        .selected_ids()
        .iter()
        .map(ToString::to_string)
        .collect()
}

fn selected_item_ids(controller: &ListController<Row>) -> Vec<u32> {
    controller
        .selected_items()
        .iter()
        .map(|row| row.id)
        .collect()
}

#[test]
fn a_reload_prunes_ids_that_vanished_and_clears_a_dead_anchor() {
    let mut controller = controller(&[1, 2, 3]);
    controller.set_selection(vec!["1".into(), "2".into(), "3".into()]);
    assert_eq!(
        controller.anchor().map(ToString::to_string),
        Some("3".into())
    );

    controller.set_items(vec![row(4, "four"), row(5, "five")]);
    assert!(controller.selected_ids().is_empty());
    assert_eq!(controller.selection(), None);
    assert_eq!(controller.anchor(), None);
    assert!(!controller.is_selected(&"1".into()));
    assert_eq!(controller.selected().map(|row| row.id), None);
}

#[test]
fn a_partial_reload_keeps_surviving_ids_in_selection_order_and_repairs_the_anchor() {
    let mut controller = controller(&[1, 2, 3]);
    controller.select("1");
    controller.toggle("2");
    controller.toggle("3");
    assert_eq!(selected(&controller), ["1", "2", "3"]);
    assert_eq!(
        controller.anchor().map(ToString::to_string),
        Some("3".into())
    );

    controller.set_items(vec![row(1, "one"), row(2, "two"), row(9, "nine")]);
    assert_eq!(
        selected(&controller),
        ["1", "2"],
        "surviving ids stay selected in the original order"
    );
    assert_eq!(
        controller.anchor().map(ToString::to_string),
        Some("2".into()),
        "the vanished anchor is repaired to the lead"
    );
    assert!(!controller.is_selected(&"3".into()));
    assert_eq!(selected_item_ids(&controller), vec![1, 2]);
}

#[test]
fn shift_extension_uses_the_anchor_and_keeps_order() {
    let mut controller = controller(&[1, 2, 3, 4]);
    controller.select("2");
    controller.extend_to("4");
    assert_eq!(selected(&controller), ["2", "3", "4"]);
    assert_eq!(
        controller.anchor().map(ToString::to_string),
        Some("2".into())
    );

    // Extending backwards from the same anchor keeps the anchor-first order
    // and makes the target the lead.
    controller.extend_to("1");
    assert_eq!(selected(&controller), ["2", "1"]);
    assert_eq!(
        controller.selection().map(ToString::to_string),
        Some("1".into())
    );
    let anchor = controller.anchor().cloned().unwrap();
    controller.toggle(&anchor);
    assert_eq!(selected(&controller), ["1"], "toggle keeps the anchor");
    assert_eq!(controller.anchor(), Some(&anchor));
}

#[test]
fn select_all_and_a_large_selection_set_stay_consistent() {
    let ids: Vec<u32> = (0..10_000).collect();
    let mut controller = controller(&ids);
    controller.set_sort(Some(|a: &Row, b: &Row| b.id.cmp(&a.id)));
    controller.select_all();
    assert_eq!(controller.selected_ids().len(), 10_000);
    assert!(controller.is_selected(&"0".into()));
    assert!(controller.is_selected(&"9999".into()));
    assert_eq!(
        controller.visible_index_of(&"9999".into()),
        Some(0),
        "visible lookup is a map read, not a scan"
    );
    assert_eq!(controller.selected_items().len(), 10_000);
    assert_eq!(controller.selected().map(|row| row.id), Some(0));

    // A reload that drops every other id keeps the survivors and their order.
    controller.set_items(
        ids.iter()
            .filter(|id| *id % 2 == 0)
            .map(|id| row(*id, "row"))
            .collect(),
    );
    assert_eq!(controller.selected_ids().len(), 5_000);
    assert_eq!(controller.selected().map(|row| row.id), Some(0));
    assert_eq!(
        controller.anchor().map(ToString::to_string),
        Some("0".into())
    );
}

#[test]
fn in_place_metadata_updates_preserve_selection_and_reapply_sort() {
    let mut controller = controller(&[1, 2, 3]);
    controller.select("2");
    controller.set_sort(Some(|a: &Row, b: &Row| a.id.cmp(&b.id)));
    controller.update_items(|row| row.name = "updated");
    assert_eq!(controller.len(), 3);
    assert_eq!(selected(&controller), ["2"]);
    controller.retain_items(|row| row.id != 3);
    assert_eq!(controller.len(), 2);
    assert_eq!(selected(&controller), ["2"]);
    assert!(controller.visible_index_of(&"3".into()).is_none());
}

// A tiny helper so assertions read by id rather than by position.

#[test]
fn item_ids_are_stable_across_a_reload_that_reorders_items() {
    let mut controller = controller(&[3, 1, 2]);
    controller.set_selection(vec!["3".into(), "1".into()]);
    controller.set_sort(Some(|a: &Row, b: &Row| a.id.cmp(&b.id)));
    assert_eq!(
        controller
            .visible_ids()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["1", "2", "3"]
    );
    controller.set_items(vec![row(1, "one"), row(2, "two"), row(3, "three")]);
    assert_eq!(selected(&controller), ["3", "1"]);
    assert_eq!(
        controller.selection().map(ToString::to_string),
        Some("1".into())
    );
    let ids: Vec<ItemId> = controller.selected_ids().to_vec();
    assert_eq!(ids.len(), 2);
}
