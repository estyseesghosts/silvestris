//! Mixed menu state (#16; HIG pp. 170-171): a dash for an attribute that
//! applies to part of the selection, exclusive with the checkmark.

use silvestris_core::command::{CommandDescriptor, CommandSet};

fn set() -> CommandSet {
    CommandSet::new([CommandDescriptor::toggle("format.bold", "Bold", true)])
}

#[test]
fn mixed_clears_checked_and_checked_clears_mixed() {
    let mut set = set();
    set.set_mixed("format.bold", true);
    let bold = set.get(&"format.bold".into()).unwrap();
    assert!(bold.mixed && !bold.checked);

    set.set_checked("format.bold", true);
    let bold = set.get(&"format.bold".into()).unwrap();
    assert!(bold.checked && !bold.mixed);
}

#[test]
fn mixed_changes_bump_the_version_once() {
    let mut set = set();
    let start = set.version;
    set.set_mixed("format.bold", true);
    let after = set.version;
    assert!(after > start);
    set.set_mixed("format.bold", true);
    assert_eq!(set.version, after, "a repeated update is not a change");
    set.set_mixed("missing", true);
    assert_eq!(set.version, after);
}

#[test]
fn builder_mixed_is_exclusive_with_checked() {
    let descriptor = CommandDescriptor::toggle("format.italic", "Italic", true).mixed(true);
    assert!(descriptor.mixed && !descriptor.checked);
}
