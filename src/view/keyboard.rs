use fnv::FnvHashMap;
use geom::LinearDir;

#[derive(Debug, Serialize, Deserialize)]
pub struct Layout<'a> {
    name: &'a str,
    outputs: [Keys<char>; 4],
    thumbs: Keys<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Keys<T> {
    row1: [T; 10],
    row2: [T; 9],
    row3: [T; 7],
}

const DEFAULT_LAYOUT: Layout = Layout {
    name: "US_en",
    outputs: [
        Keys {
            row1: ['q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p'],
            row2:   ['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l'],
            row3:        ['z', 'x', 'c', 'v', 'b', 'n', 'm'],
        },
        Keys {
            row1: ['Q', 'W', 'E', 'R', 'T', 'Y', 'U', 'I', 'O', 'P'],
            row2:   ['A', 'S', 'D', 'F', 'G', 'H', 'J', 'K', 'L'],
            row3:        ['Z', 'X', 'C', 'V', 'B', 'N', 'M'],
        },
        Keys {
            row1: ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'],
            row2:  ['\\', '@', ',', '`', '"', '\'', '.', '*', '/'],
            row3:        ['!', '-', '(',  ':', ')', '+', '?'],
        },
        Keys {
            row1: ['·', '“', '~', '#', '×', '…', '$', '=', '”', '°'],
            row2:   ['‘', '%', '[', '_', '^', '|', ']', '&', '’'],
            row3:        ['–', '<', '{', ';', '}', '>', '—'],
        },
    ],
    // Which thumb presses each key
    thumbs: Keys {
        row1: [0, 0, 0, 0, 0, 1, 1, 1, 1, 1],
        row2:  [0, 0, 0, 0, 1, 1, 1, 1, 1],
        row3:     [0, 0, 0, 0, 1, 1, 1],
    },
};

// Most of the combination sequences come from X.org.
// The chosen characters come from the layout described by
// Robert Bringhurst in *The Elements of Typographic Style*,
// version 3.1, p. 92.
lazy_static! {
    static ref DEFAULT_COMBINATIONS: FnvHashMap<&'static str, char> = {
        let mut m = FnvHashMap::default();
        m.insert("oe", 'œ');
        m.insert("Oe", 'Œ');
        m.insert("ae", 'æ');
        m.insert("AE", 'Æ');
        m.insert("c,", 'ç');
        m.insert("C,", 'Ç');
        m.insert("a;", 'ą');
        m.insert("e;", 'ę');
        m.insert("A;", 'Ą');
        m.insert("E;", 'Ę');
        m.insert("a~", 'ã');
        m.insert("o~", 'õ');
        m.insert("n~", 'ñ');
        m.insert("A~", 'Ã');
        m.insert("O~", 'Õ');
        m.insert("N~", 'Ñ');
        m.insert("a'", 'á');
        m.insert("e'", 'é');
        m.insert("i'", 'í');
        m.insert("o'", 'ó');
        m.insert("u'", 'ú');
        m.insert("y'", 'ý');
        m.insert("z'", 'ź');
        m.insert("s'", 'ś');
        m.insert("c'", 'ć');
        m.insert("n'", 'ń');
        m.insert("A'", 'Á');
        m.insert("E'", 'É');
        m.insert("I'", 'Í');
        m.insert("O'", 'Ó');
        m.insert("U'", 'Ú');
        m.insert("Y'", 'Ý');
        m.insert("Z'", 'Ź');
        m.insert("S'", 'Ś');
        m.insert("C'", 'Ć');
        m.insert("N'", 'Ń');
        m.insert("a`", 'à');
        m.insert("e`", 'è');
        m.insert("i`", 'ì');
        m.insert("o`", 'ò');
        m.insert("u`", 'ù');
        m.insert("A`", 'À');
        m.insert("E`", 'È');
        m.insert("I`", 'Ì');
        m.insert("O`", 'Ò');
        m.insert("U`", 'Ù');
        m.insert("a^", 'â');
        m.insert("e^", 'ê');
        m.insert("i^", 'î');
        m.insert("o^", 'ô');
        m.insert("u^", 'û');
        m.insert("w^", 'ŵ');
        m.insert("y^", 'ŷ');
        m.insert("A^", 'Â');
        m.insert("E^", 'Ê');
        m.insert("I^", 'Î');
        m.insert("O^", 'Ô');
        m.insert("U^", 'Û');
        m.insert("W^", 'Ŵ');
        m.insert("Y^", 'Ŷ');
        m.insert("a:", 'ä');
        m.insert("e:", 'ë');
        m.insert("i:", 'ï');
        m.insert("o:", 'ö');
        m.insert("u:", 'ü');
        m.insert("y:", 'ÿ');
        m.insert("A:", 'Ä');
        m.insert("E:", 'Ë');
        m.insert("I:", 'Ï');
        m.insert("O:", 'Ö');
        m.insert("U:", 'Ü');
        m.insert("Y:", 'Ÿ');
        m.insert("u\"", 'ű');
        m.insert("o\"", 'ő');
        m.insert("U\"", 'Ű');
        m.insert("O\"", 'Ő');
        m.insert("z.", 'ż');
        m.insert("Z.", 'Ż');
        m.insert("th", 'þ');
        m.insert("Th", 'Þ');
        m.insert("ao", 'å');
        m.insert("Ao", 'Å');
        m.insert("l/", 'ł');
        m.insert("d/", 'đ');
        m.insert("o/", 'ø');
        m.insert("L/", 'Ł');
        m.insert("D/", 'Đ');
        m.insert("O/", 'Ø');
        m.insert("mu", 'µ');
        m.insert("l-", '£');
        m.insert("pp", '¶');
        m.insert("so", '§');
        m.insert("|-", '†');
        m.insert("|=", '‡');
        m.insert("ss", 'ß');
        m.insert("Ss", 'ẞ');
        m.insert("o_", 'º');
        m.insert("a_", 'ª');
        m.insert("oo", '°');
        m.insert("!!", '¡');
        m.insert("??", '¿');
        m.insert(".-", '·');
        m.insert(".=", '•');
        m.insert(".>", '›');
        m.insert(".<", '‹');
        m.insert("'1", '′');
        m.insert("'2", '″');
        m.insert("[[", '⟦');
        m.insert("]]", '⟧');
        m.insert("+-", '±');
        m.insert("-:", '÷');
        m.insert("<=", '≤');
        m.insert(">=", '≥');
        m.insert("=/", '≠');
        m.insert("-,", '¬');
        m.insert("~~", '≈');
        m.insert("<<", '«');
        m.insert(">>", '»');
        m.insert("12", '½');
        m.insert("13", '⅓');
        m.insert("23", '⅔');
        m.insert("14", '¼');
        m.insert("34", '¾');
        m.insert("15", '⅕');
        m.insert("25", '⅖');
        m.insert("35", '⅗');
        m.insert("45", '⅘');
        m.insert("16", '⅙');
        m.insert("56", '⅚');
        m.insert("18", '⅛');
        m.insert("38", '⅜');
        m.insert("58", '⅝');
        m.insert("78", '⅞');
        m.insert("#f", '♭');
        m.insert("#n", '♮');
        m.insert("#s", '♯');
        m.insert("%o", '‰');
        m.insert("e=", '€');
        m.insert("or", '®');
        m.insert("oc", '©');
        m.insert("op", '℗');
        m.insert("tm", '™');
        m
    };
}

pub struct State {
    shift: bool,
    alternate: bool,
    combine: bool,
}

enum RelativePlace {
    Before,
    After,
}

enum KeyKind {
    Output(char),
    Delete(LinearDir),
    Move(LinearDir),
    Shift,
    Return,
    Combine,
    Alternate,
    Space,
}

type Row = u8;

pub struct BuiltInKey {
    kind: KeyKind,
    width: f32,
}

const BUILTIN_ROW_SPLIT: [usize; 3] = [2, 2, 5];

const BUILTIN_KEYS: [BuiltInKey; 9] = [
    BuiltInKey {
        kind: KeyKind::Delete(LinearDir::Backward),
        width: 1.0,
    },
    BuiltInKey {
        kind: KeyKind::Delete(LinearDir::Forward),
        width: 1.0,
    },
    BuiltInKey {
        kind: KeyKind::Shift,
        width: 2.0,
    },
    BuiltInKey {
        kind: KeyKind::Return,
        width: 2.0,
    },
    BuiltInKey {
        kind: KeyKind::Move(LinearDir::Backward),
        width: 1.5,
    },
    BuiltInKey {
        kind: KeyKind::Combine,
        width: 1.5,
    },
    BuiltInKey {
        kind: KeyKind::Space,
        width: 4.0,
    },
    BuiltInKey {
        kind: KeyKind::Alternate,
        width: 1.5,
    },
    BuiltInKey {
        kind: KeyKind::Move(LinearDir::Forward),
        width: 1.5,
    },
];

pub struct Keyboard<'a> {
    layout: Layout<'a>,
    state: State,
}
