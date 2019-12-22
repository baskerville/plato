# Home

## Summary

- Tap to select/de-select a category.
- Swipe north to negate/un-negate a category.
- Swipe south from the inside to the outside of the bar to grow it.
- Swipe north from the outside to the inside of the bar to shrink it.
- Swipe west/east to go to the next/previous page.

## Shelf

- Swipe west/east to go to the next/previous page.
- Tap on a book entry to open it.

## Bottom bar

Tap and hold the next/previous page icon to go the last/first page.

Tap the matches count label to bring up the library menu.

# Reader

## Viewer

The screen is divided into seven regions:

![Touch Regions](../artworks/touch_regions.svg)

Tap gestures by region:

- *LE* (Left Ear):
	- Normal Mode: previous page.
	- Search Mode: previous results page.
- *MB* (Middle Band): toggle the top and bottom bars.
- *RE* (Right Ear):
	- Normal Mode: next page.
	- Search Mode: next results page.
- *TL* (Top Left Corner): previous location.
- *TR* (Top Right Corner): toggle bookmark.
- *BL* (Bottom Left Corner): table of contents in normal mode, previous page in search mode.
- *BR* (Bottom Right Corner): go to page in normal mode, next page in search mode.

Swipe west/east to go to the next/previous page.

Swipe north/south to scroll the page stream when the zoom mode is fit-to-width.

Rotate to change the screen orientation (one finger is the center, the other describes the desired rotation with a circular motion around the center: the two fingers should land and take off simultaneously).

Spread (resp. pinch) horizontally to switch the zoom mode to fit-to-width (resp. fit-to-page).

The following swipe sequences are recognized:

![Swipe Sequences](../artworks/swipe_sequences.svg)

- Arrow west/east: go to the previous/next chapter in normal mode, the first/last results in search mode.
- Arrow north/south: start searching text backward/forward.
- Top left/right corner: go to the previous/next annotation, highlight or bookmark.
- Bottom left corner: guess the frontlight if there's more than two frontlight presets defined, toggle the frontlight otherwise.
- Bottom right corner: toggle the bitonal mode.

Simultaneously performing an east arrow with the left hand and a west arrow with the right hand will lead you back to the home screen.

### Text Selection

To select text, tap and hold the first or last word of the selection. Wait for the selection feedback. Move your finger on the other end of the selection and lift it. If you've made a mistake, select *Adjust Selection* and tap on the correct ends; tap and hold the selection when you're done.

## Bottom bar

Tap and hold the next/previous page icon to go the next/previous chapter.

## Top bar

Tap the title label to bring up the book menu.

# Home & Reader

Tap the bottom left and top right corners to do a full screen refresh.

Tap the top left and bottom right corners to take a screenshot.

## Menus

You can select a menu entry *without closing the menu* by tapping and holding it.

## Top bar

The frontlight can be toggled by holding the frontlight icon.

## Bottom bar

Tap the page indicator to go a specific page.

## Keyboard

The *ALT* and *SHIFT* keys can be locked by tapping them twice.

The *CMB* (combine) key can be used to enter special characters, e.g.: `CMB o e` produces `œ` (cf. [Combination Sequences](#combination-sequences)).

A tap and hold on the delete or motion keys will act on words instead of characters.

Tap and hold the space bar to bring up the keyboard layouts menu.

Keyboard layouts are described through a JSON object with the following keys:

- *name*: displayed in the keyboard layouts menu.
- *outputs*: list of output keys for each modifier combination (*none*, *shift*, *alt*, *shift+alt*).
- *keys*: description of each key on the keyboard. The following special key names (and abbreviations) are recognized: *Shift* (*Sft*), *Return* (*Ret*), *Alternate* (*Alt*), *Combine* (*Cmb*), *MoveFwd* (*MoveF*, *MF*), *MoveBwd* (*MoveB*, *MB*), *DelFwd* (*DelF*, *DF*), *DelBwd* (*DelB*, *DB*), *Space* (*Spc*). *▢* is used to indicate an output key.
- *widths*: width/height ratio for each key. The key gap's ratio is 0.06.

# Applications

Applications can be launched from the *Applications* submenu of the main menu.

You can go back to the previous view by tapping the top-left *back arrow*.

## Dictionary

*Dictionary* can be launched from the *Reader* view by tapping and holding a word or by making a text selection and tapping *Define* in the selection menu.

Dictionaries will be searched recursively in the `dictionaries` directory. The supported format is *dictd*: `.dict.dz` (or `.dict`) and `.index`. The dictionary definitions can be styled by creating a stylesheet at `css/dictionary-user.css`. The definitions that aren't formatted with XML are wrapped inside a *pre* tag. The font size and margin width can be changed in the `[dictionary]` section of `Settings.toml`.

You can select the search target by tapping the label in the bottom bar. You can set the input languages of a dictionary by tapping and holding the target's label. You can then provide a comma-separated list of IETF language tags (e.g.: *en, en-US, en-GB*).

You can toggle the fuzzy search mode by tapping the related entry in the search menu (brought up by tapping the search icon). If it's enabled, the headwords that differ only slightly ([Levenshtein distance](https://en.wikipedia.org/wiki/Levenshtein_distance) ≤ 1) from the current query will be considered matches.

# Input Fields

Tapping an input field will:
- Focus the input field if it isn't.
- Move the cursor under your finger if it is.

Tap and hold inside an input field to bring up the input history menu.

# Annex

## Combination Sequences

	o e   œ       a `   à       u "   ű       ] ]   ⟧
	O e   Œ       e `   è       o "   ő       + -   ±
	a e   æ       i `   ì       U "   Ű       - :   ÷
	A E   Æ       o `   ò       O "   Ő       < =   ≤
	c ,   ç       u `   ù       z .   ż       > =   ≥
	C ,   Ç       A `   À       Z .   Ż       = /   ≠
	a ;   ą       E `   È       t h   þ       - ,   ¬
	e ;   ę       I `   Ì       T h   Þ       ~ ~   ≈
	A ;   Ą       O `   Ò       a o   å       < <   «
	E ;   Ę       U `   Ù       A o   Å       > >   »
	a ~   ã       a ^   â       l /   ł       1 2   ½
	o ~   õ       e ^   ê       d /   đ       1 3   ⅓
	n ~   ñ       i ^   î       o /   ø       2 3   ⅔
	A ~   Ã       o ^   ô       L /   Ł       1 4   ¼
	O ~   Õ       u ^   û       D /   Đ       3 4   ¾
	N ~   Ñ       w ^   ŵ       O /   Ø       1 5   ⅕
	a '   á       y ^   ŷ       m u   µ       2 5   ⅖
	e '   é       A ^   Â       l -   £       3 5   ⅗
	i '   í       E ^   Ê       p p   ¶       4 5   ⅘
	o '   ó       I ^   Î       s o   §       1 6   ⅙
	u '   ú       O ^   Ô       | -   †       5 6   ⅚
	y '   ý       U ^   Û       | =   ‡       1 8   ⅛
	z '   ź       W ^   Ŵ       s s   ß       3 8   ⅜
	s '   ś       Y ^   Ŷ       S s   ẞ       5 8   ⅝
	c '   ć       a :   ä       o _   º       7 8   ⅞
	n '   ń       e :   ë       a _   ª       # f   ♭
	A '   Á       i :   ï       o o   °       # n   ♮
	E '   É       o :   ö       ! !   ¡       # s   ♯
	I '   Í       u :   ü       ? ?   ¿       % o   ‰
	O '   Ó       y :   ÿ       . -   ·       e =   €
	U '   Ú       A :   Ä       . =   •       o r   ®
	Y '   Ý       E :   Ë       . >   ›       o c   ©
	Z '   Ź       I :   Ï       . <   ‹       o p   ℗
	S '   Ś       O :   Ö       ' 1   ′       t m   ™
	C '   Ć       U :   Ü       ' 2   ″       
	N '   Ń       Y :   Ÿ       [ [   ⟦       
