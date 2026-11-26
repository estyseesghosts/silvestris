# Scope bar

`ScopeBar` is the small row of mutually exclusive scopes used to narrow a
search, list or other result set (#46). It is not navigation and not a second
toolbar; ordinary navigation keeps its source list or view segmented control.

```rust
ScopeBar::new(
    "search-scope",
    "Search scope",
    vec![
        Scope::new("all", "All"),
        Scope::new("folder", "Documents"),
        Scope::new("images", "Images"),
    ],
)
.caption("Search:")
.selected("all")
.on_select(|id, _, cx| set_scope(id.clone(), cx))
```

- `caption` is the optional visible leading text ("Search:"); `label` is the
  external accessibility name.
- The bar composes the measured `SegmentedControl` in its small textured form,
  so selection, keyboard movement, focus ring, inactive appearance and hit
  testing are the segmented control's. `size` exists for a consumer with a
  documented reason; the default is `ControlSize::Small`.
- `on_select` reports the chosen scope id. Filtering and result recomputation
  stay in the application.
- A disabled scope renders disabled but remains visible, so the available
  scopes do not jump when one is unavailable.

Do not add a scope bar to ordinary navigation merely to look Mac-like. The HIG
uses it specifically for narrowing search/results.
