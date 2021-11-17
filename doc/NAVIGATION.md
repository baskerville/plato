## Naming pages

To name a page, hold the current page indicator and select the *Name* entry. A page name can be an uppercase ASCII letter, a lowercase roman numeral or an arabic numeral.

Once a page is named, you can jump to any page above it in the same category. For example if you've defined page 15 as *vi*, by entering *'ix*, in the *Go to page* input field, you'll jump to page 18.

You can also select a page name in the book's text and jump to it by tapping *Go To* in the selection menu. This can be particularly useful within a book's index.

## Overriding the TOC

You can override a book's TOC by adding a *toc* key to the corresponding entry in `.metadata.json`:

```
{
	⋮
	"toc": [
		["Chapter 1", 17],
		["Chapter 2", 46],
		["Chapter 3", 88],
		⋮
	],
	⋮
},
```

Page names can also be used instead of page numbers:

```
{
	⋮
	"toc": [
		["Preface", "'viii"],
		["Acknowledgments", "'xvii"],
		["Introduction", "'1"],
		["Section 1", "'16", [["Chapter 1", "'16"],
							  ["Chapter 2", "'47"],
							  ["Chapter 3", "'62"]]],
		⋮
		["Conclusion", "'141"],
		["Notes", "'145"],
		["Index", "'169"]
	],
	⋮
},
```

For the page names to be resolved, you'll need to name the first page of each category.

## Special Notations

`-` or `+` can be prepended to a page number to jump to a relative page.

Instead of the page number, you can specify one of the following characters:
- `(` and `)` to jump to the first and last page.
- `_` to jump to a random page.

If a number ending with `%` is given it will be interpreted as a percentage of the book's pages count.
