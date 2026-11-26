# Action menu and command pop-down

`ActionMenu` and `CommandPopDown` are command menus (#46). Their entries are
`CommandRegistry` commands; choosing a row invokes the command through the
registry and responder chain. They are not `PopUpButton`: a value pop-up lists
mutually exclusive nouns and keeps its current choice.

```rust
ActionMenu::new(
    "toolbar-action",
    "Action",
    MenuModel {
        label: "Action".into(),
        entries: vec![
            MenuEntry::Command("file.new_folder".into()),
            MenuEntry::Separator,
            MenuEntry::Command("file.get_info".into()),
        ],
    },
    registry.clone(),
)
.chain(chain.clone())

CommandPopDown::new("more", "More", menu, registry.clone())
```

- `ActionMenu` is the toolbar affordance: the standard
  `TemplateImage::Action` artwork plus a small down arrow.
- `CommandPopDown` is the labelled form; `.image(TemplateImage::…)` adds a
  monochrome image before the label.
- Both open through `PopupMenu::open_pop_down` on the window's shared
  `TransientLayer`, so one menu excludes another and focus returns to the
  button. Pass `.layer(...)` to share the window layer.
- Command rows print no keyboard shortcuts (HIG Action-menu rule).
  `PopupMenu::set_show_shortcuts(false)` owns that rule; the main menu bar
  keeps its key column.
- `size`, `style` (textured or round), `disabled` and `chain` mirror the other
  command controls. Enabled/checked/title state always comes from the registry
  and responder chain at draw and invocation time.

The down arrow reuses the pop-up cap's triangle path and ink, inset 5 px from
the button's trailing/bottom edge. There is no separate Action-menu crop in the
corpus; the inset is listed in `docs/exceptions.md`.

Do not use an Action menu as a dumping ground for every command, and do not
put verbs in a normal value pop-up.
