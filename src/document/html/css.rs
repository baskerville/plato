use fxhash::FxHashSet;

#[derive(Debug, Clone)]
pub enum Selector {
    Simple(SimpleSelector),
    ParentChild(SimpleSelector, SimpleSelector),
    Siblings(SimpleSelector, SimpleSelector),
}

#[derive(Debug, Clone)]
pub struct SimpleSelector {
    pub tag_name: Option<String>,
    pub classes: FxHashSet<String>,
    pub id: Option<String>,
}


impl Default for SimpleSelector {
    fn default() -> SimpleSelector {
        SimpleSelector { tag_name: None, id: None, classes: FxHashSet::default() }
    }
}

pub type Specificity = [usize; 3];

impl SimpleSelector {
    // http://www.w3.org/TR/selectors/#specificity
    pub fn specificity(&self) -> Specificity {
        let a = self.id.iter().count();
        let b = self.classes.len();
        let c = self.tag_name.iter().count();
        [a, b, c]
    }
}

impl Selector {
    pub fn specificity(&self) -> Specificity {
        match self {
            Selector::Simple(sel) => sel.specificity(),
            Selector::ParentChild(sel1, sel2) |
            Selector::Siblings(sel1, sel2) => {
                let s1 = sel1.specificity();
                let s2 = sel2.specificity();
                [s1[0] + s2[0], s1[1] + s2[1], s1[2] + s2[2]]
            },
        }
    }
}

#[derive(Debug)]
pub struct Declaration {
    pub name: String,
    pub value: String,
}

impl Default for Declaration {
    fn default() -> Declaration {
        Declaration {
            name: String::default(),
            value: String::default(),
        }
    }
}

#[derive(Debug)]
pub struct Rule {
    pub kind: RuleKind,
    pub selectors: Vec<Selector>,
    pub declarations: Vec<Declaration>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuleKind {
    Viewer = 0,
    User = 1,
    Document = 2,
}

#[derive(Debug)]
pub struct CssParser<'a> {
    input: &'a str,
    offset: usize,
}

impl<'a> CssParser<'a> {
    pub fn new(input: &str) -> CssParser {
        CssParser {
            input,
            offset: 0,
        }
    }

    fn eof(&self) -> bool {
        self.offset >= self.input.len()
    }

    fn next(&self) -> Option<char> {
        self.input[self.offset..].chars().next()
    }

    fn starts_with(&self, s: &str) -> bool {
        self.input[self.offset..].starts_with(s)
    }

    fn advance(&mut self, n: usize) {
        for c in self.input[self.offset..].chars().take(n) {
            self.offset += c.len_utf8();
        }
    }

    fn advance_while<F>(&mut self, test: F) where F: FnMut(&char) -> bool {
        for c in self.input[self.offset..].chars().take_while(test) {
            self.offset += c.len_utf8();
        }
    }

    fn advance_until(&mut self, target: &str) {
        if let Some(first) = target.chars().next() {
            while !self.eof() {
                self.advance(1);
                self.advance_while(|&c| c != first);
                if self.starts_with(target) {
                    break;
                }
            }
            self.advance(target.chars().count());
        }
    }

    fn parse_selectors(&mut self) -> Vec<Selector> {
        let mut selectors = Vec::new();
        let mut simple_selectors = Vec::new();
        let mut sel = SimpleSelector::default();
        let mut combinator = None;

        self.advance_while(|&c| c.is_whitespace());

        while !self.eof() {
            match self.next() {
                Some('#') => {
                    self.advance(1);
                    let offset = self.offset;
                    self.advance_while(|&c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
                    sel.id = Some(self.input[offset..self.offset].to_string());

                },
                Some('.') => {
                    self.advance(1);
                    let offset = self.offset;
                    self.advance_while(|&c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
                    sel.classes.insert(self.input[offset..self.offset].to_string());
                },
                _ => {
                    let offset = self.offset;
                    self.advance_while(|&c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '@');
                    if self.offset > offset {
                        sel.tag_name = Some(self.input[offset..self.offset].to_string());
                    } else {
                        self.advance_while(|&c| c.is_whitespace());
                        simple_selectors.push(sel);
                        sel = SimpleSelector::default();
                        let next = self.next();
                        self.advance(1);
                        if next == Some('>') || next == Some('+') {
                            combinator = next;
                        } else if next == Some('{') || next == Some(',') {
                            match combinator {
                                Some('>') if simple_selectors.len() == 2 => {
                                    selectors.push(Selector::ParentChild(simple_selectors[0].clone(),
                                                                         simple_selectors[1].clone()));
                                },
                                Some('+') if simple_selectors.len() == 2 => {
                                    selectors.push(Selector::Siblings(simple_selectors[0].clone(),
                                                                      simple_selectors[1].clone()));
                                },
                                None if simple_selectors.len() == 1 => {
                                    selectors.push(Selector::Simple(simple_selectors[0].clone()));
                                },
                                _ => (),
                            }
                            simple_selectors.clear();
                            if next == Some('{') {
                                break;
                            }
                        }
                    }
                    self.advance_while(|&c| c.is_whitespace());
                }
            }
        }

        selectors.sort_by(|a, b| b.specificity().cmp(&a.specificity()));

        selectors
    }

    pub fn parse_declarations(&mut self) -> Vec<Declaration> {
        let mut declarations = Vec::new();
        let mut d = Declaration::default();

        while !self.eof() {
            self.advance_while(|&c| c.is_whitespace());

            match self.next() {
                Some(':') => {
                    self.advance(1);
                    let offset = self.offset;
                    while !self.eof() {
                        // TODO: Skip !important.
                        self.advance_while(|&c| c != '"' && c != ';' && c != '}');
                        if let Some('"') = self.next() {
                            self.advance(1);
                            self.advance_while(|&c| c != '"');
                            self.advance(1);
                        } else {
                            break;
                        }
                    }
                    d.value = self.input[offset..self.offset].trim().to_string();
                },
                Some(';') => {
                    self.advance(1);
                    declarations.push(d);
                    d = Declaration::default();
                },
                Some('}') => {
                    self.advance(1);
                    break;
                }
                Some('/') => {
                    if self.starts_with("/*") {
                        self.advance(2);
                        self.advance_until("*/");
                    } else {
                        self.advance(1);
                    }
                },
                _ => {
                    let offset = self.offset;
                    self.advance_while(|&c| c != ':');
                    d.name = self.input[offset..self.offset].trim().to_string();
                }
            }
        }

        if !d.name.is_empty() {
            declarations.push(d);
        }

        declarations
    }

    fn parse_rule(&mut self, kind: RuleKind) -> Rule {
        let selectors = self.parse_selectors();
        let declarations = self.parse_declarations();
        Rule { kind, selectors, declarations }
    }

    fn skip_nested_rules(&mut self) {
        self.advance_while(|&c| c != '{');
        self.advance(1);
        while !self.eof() {
            self.parse_rule(RuleKind::Viewer);
            self.advance_while(|&c| c.is_whitespace());
            if let Some('}') = self.next() {
                self.advance(1);
                break;
            }
        }
    }

    pub fn parse(&mut self, kind: RuleKind) -> (Vec<Rule>, Vec<Rule>) {
        let mut rules = Vec::new();
        let mut at_rules = Vec::new();
        while !self.eof() {
            self.advance_while(|&c| c.is_whitespace());
            match self.next() {
                None => break,
                Some('/') => {
                    if self.starts_with("/*") {
                        self.advance(2);
                        self.advance_until("*/");
                    } else {
                        self.advance(1);
                    }
                },
                Some('@') => {
                    if self.starts_with("@namespace") || self.starts_with("@charset") {
                        self.advance_while(|&c| c != ';');
                        self.advance(1);
                        continue;
                    } else if self.starts_with("@media") {
                        self.skip_nested_rules();
                        continue;
                    } else {
                        at_rules.push(self.parse_rule(kind));
                    }
                },
                _ => rules.push(self.parse_rule(kind)),
            }
        }
        (rules, at_rules)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_css() {
        let text = "a, .b { b: c; d: e }";
        let (css, _) = CssParser::new(text).parse(RuleKind::User);
        println!("{:?}", css);
    }

    #[test]
    fn combinators_css() {
        let text = "a#i.j.k > b { b: c } a + .l { u: v } a { x: y }";
        let (css, _) = CssParser::new(text).parse(RuleKind::User);
        println!("{:?}", css);
    }
}
