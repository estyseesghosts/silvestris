//! One panel sort policy: folders first, case-insensitive name order from a
//! precomputed key, and explicit direction for every other key.

use std::cmp::Ordering;

use silvestris::controls::SortDirection;
use silvestris::file_panel::{FileEntry, FileSort, FileSortKey};

fn entry(name: &str) -> FileEntry {
    FileEntry::document(name.to_lowercase(), name)
}

fn names(entries: &[FileEntry]) -> Vec<String> {
    entries.iter().map(|entry| entry.name.to_string()).collect()
}

#[test]
fn name_sort_is_case_insensitive_and_folders_stay_first_in_both_directions() {
    let mut entries = vec![
        entry("beta.txt"),
        FileEntry::folder("docs", "Documents"),
        entry("Alpha.txt"),
        entry("charlie.txt"),
    ];
    let ascending = FileSort::new(FileSortKey::Name, SortDirection::Ascending);
    entries.sort_by(|a, b| ascending.compare(a, b));
    assert_eq!(
        names(&entries),
        ["Documents", "Alpha.txt", "beta.txt", "charlie.txt"]
    );

    let descending = FileSort::new(FileSortKey::Name, SortDirection::Descending);
    entries.sort_by(|a, b| descending.compare(a, b));
    assert_eq!(
        names(&entries),
        ["Documents", "charlie.txt", "beta.txt", "Alpha.txt"],
        "only the key order reverses, never the folders-first rule"
    );
}

#[test]
fn name_keys_lowercase_display_text_without_changing_it() {
    let unicode = FileEntry::document("u", "\u{c4}pfel.txt");
    assert_eq!(unicode.name.as_ref(), "\u{c4}pfel.txt");
    assert_eq!(unicode.name_key().as_ref(), "\u{e4}pfel.txt");
    assert_eq!(
        unicode.name_key(),
        &unicode.name.to_lowercase(),
        "simple Unicode lowercasing, not full case folding"
    );
    let mut renamed = unicode.clone();
    renamed.set_name("Zebra.txt");
    assert_eq!(renamed.name_key().as_ref(), "zebra.txt");
    assert_eq!(renamed.name.as_ref(), "Zebra.txt");
}

#[test]
fn modified_and_size_use_the_requested_direction() {
    let older = FileEntry::document("a", "older.txt").size(10).modified(100);
    let newer = FileEntry::document("b", "newer.txt").size(20).modified(200);

    let newest_first = FileSort::new(FileSortKey::Modified, SortDirection::Descending);
    assert_eq!(newest_first.compare(&newer, &older), Ordering::Less);
    assert_eq!(newest_first.compare(&older, &newer), Ordering::Greater);
    let oldest_first = newest_first.toggled();
    assert_eq!(oldest_first.compare(&older, &newer), Ordering::Less);

    let largest_first = FileSort::new(FileSortKey::Size, SortDirection::Descending);
    assert_eq!(largest_first.compare(&newer, &older), Ordering::Less);
    let smallest_first = largest_first.toggled();
    assert_eq!(smallest_first.compare(&older, &newer), Ordering::Less);
}

#[test]
fn keys_map_to_the_table_columns_and_natural_first_directions() {
    for key in [
        FileSortKey::Name,
        FileSortKey::Kind,
        FileSortKey::Modified,
        FileSortKey::Size,
    ] {
        assert_eq!(FileSortKey::from_column_id(key.column_id()), Some(key));
    }
    assert_eq!(
        FileSortKey::Name.default_direction(),
        SortDirection::Ascending
    );
    assert_eq!(
        FileSortKey::Kind.default_direction(),
        SortDirection::Ascending
    );
    assert_eq!(
        FileSortKey::Modified.default_direction(),
        SortDirection::Descending
    );
    assert_eq!(
        FileSortKey::Size.default_direction(),
        SortDirection::Descending
    );
    assert_eq!(
        FileSortKey::from_column_id("unknown"),
        None,
        "no second translation table"
    );
}
