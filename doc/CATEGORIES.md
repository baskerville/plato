The categories of a book might come from:
- Its directory relative to the library path: `a/b/c` becomes `a.b.c`.
- The `dc:subject` tag for ePUBs.

They are stored in `.metadata.json` in the `categories` array of each entry.

Within the home view, the categories are displayed in the summary bar (the bar with a gray background).

In this bar will appear:
- The selected and negated categories.
- The ancestors of the selected and negated categories.
- The direct children of the selected categories.
- The first component of the categories appearing in the matched books (the first component of `a.b.c` is `a`).
