use fxhash::FxHashSet;
use super::style::StyleSheet;

#[derive(Debug, Clone)]
pub struct Selector {
    pub simple_selectors: Vec<SimpleSelector>,
    pub combinators: Vec<Combinator>,
}

#[derive(Debug, Clone)]
pub struct SimpleSelector {
    pub tag_name: Option<String>,
    pub classes: FxHashSet<String>,
    pub pseudo_classes: Vec<PseudoClass>,
    pub attributes: Vec<Attribute>,
    pub id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Attribute {
    pub name: String,
    pub operator: AttributeOperator,
}

#[derive(Debug, Clone)]
pub enum PseudoClass {
    FirstChild,
    LastChild,
}

#[derive(Debug, Clone)]
pub enum AttributeOperator {
    // `[attr]`
    Exists,
    // `[attr=value]`
    Matches(String),
    // `[attr~=value]`
    Contains(String),
    // `[attr|=value]`
    StartsWith(String),
}

#[derive(Debug, Copy, Clone)]
pub enum Combinator {
    None,
    // a > b
    Child,
    // a   b
    Descendant,
    // a + b
    NextSibling,
    // a ~ b
    SubsequentSibling,
}

impl Default for Selector {
    fn default() -> Self {
        Selector {
            simple_selectors: Vec::new(),
            combinators: Vec::new(),
        }
    }
}

impl Default for SimpleSelector {
    fn default() -> Self {
        SimpleSelector {
            tag_name: None,
            classes: FxHashSet::default(),
            pseudo_classes: Vec::new(),
            attributes: Vec::new(),
            id: None,
        }
    }
}

pub type Specificity = [usize; 3];

impl Selector {
    pub fn specificity(&self) -> Specificity {
        let mut spec = [0usize; 3];
        for sel in &self.simple_selectors {
            spec[0] = spec[0].saturating_add(sel.id.iter().count());
            spec[1] = spec[1].saturating_add(sel.classes.len());
            spec[1] = spec[1].saturating_add(sel.pseudo_classes.len());
            spec[1] = spec[1].saturating_add(sel.attributes.len());
            spec[2] = spec[2].saturating_add(sel.tag_name.iter().count());
        }
        spec
    }
}

#[derive(Debug, Clone)]
pub struct Declaration {
    pub name: String,
    pub value: String,
    pub important: bool,
}

impl Default for Declaration {
    fn default() -> Declaration {
        Declaration {
            name: String::default(),
            value: String::default(),
            important: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub selector: Selector,
    pub declarations: Vec<Declaration>,
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
        while !self.eof() && !self.starts_with(target) {
            self.advance(1);
        }
        self.advance(target.chars().count());
    }

    fn skip_spaces_and_comments(&mut self) {
        loop {
            let offset = self.offset;
            self.advance_while(|&c| c.is_whitespace());
            if self.starts_with("/*") {
                self.advance(2);
                self.advance_until("*/");
            }
            if offset == self.offset {
                break;
            }
        }
    }

    fn skip_ident(&mut self) {
        self.advance_while(|&c| c.is_ascii_alphanumeric() ||
                                c == '-' ||
                                c == '_');
    }

    fn skip_block(&mut self) {
        let mut balance = 0u8;

        while !self.eof() {
            match self.next() {
                Some('{') => {
                    self.advance(1);
                    balance = balance.saturating_add(1);
                },
                Some('}') => {
                    self.advance(1);
                    balance = balance.saturating_sub(1);
                },
                _ => break,
            }
            if balance == 0 {
                break;
            }
            self.advance_while(|&c| c != '{' && c != '}')
        }
    }

    fn skip_at_rule(&mut self) {
        self.advance_while(|&c| c != ';' && c != '{');

        match self.next() {
            Some(';') => self.advance(1),
            Some('{') => self.skip_block(),
            _ => (),
        }
    }

    fn attribute_value(&mut self) -> String {
        match self.next() {
            Some(delim @ '"' | delim @ '\'') => {
                self.advance(1);
                let start_offset = self.offset;
                self.advance_while(|&c| c != delim);
                let end_offset = self.offset;
                self.advance(1);
                self.input[start_offset..end_offset].to_string()
            },
            _ => {
                let offset = self.offset;
                self.skip_ident();
                self.input[offset..self.offset].to_string()
            },
        }
    }

    fn parse_selectors(&mut self) -> Vec<Selector> {
        let mut supported = true;
        let mut selectors = Vec::new();
        let mut s = Selector::default();
        let mut selec = SimpleSelector::default();
        let mut comb = Combinator::None;

        while !self.eof() {
            match self.next() {
                Some('#') => {
                    self.advance(1);
                    let offset = self.offset;
                    self.skip_ident();
                    selec.id = Some(self.input[offset..self.offset].to_string());
                },
                Some('.') => {
                    self.advance(1);
                    let offset = self.offset;
                    self.skip_ident();
                    selec.classes.insert(self.input[offset..self.offset].to_string());
                },
                Some('[') => {
                    self.advance(1);
                    self.skip_spaces_and_comments();
                    let offset = self.offset;
                    self.skip_ident();
                    let mut name = self.input[offset..self.offset].to_string();
                    if self.next() == Some('|') && !self.starts_with("|=") {
                        self.advance(1);
                        name += ":";
                        let offset = self.offset;
                        self.skip_ident();
                        name += &self.input[offset..self.offset];
                    }
                    self.skip_spaces_and_comments();
                    match self.next() {
                        Some(']') => {
                            self.advance(1);
                            selec.attributes.push(Attribute {
                                name,
                                operator: AttributeOperator::Exists,
                            });
                        },
                        Some('=') => {
                            self.advance(1);
                            self.skip_spaces_and_comments();
                            let value = self.attribute_value();
                            selec.attributes.push(Attribute {
                                name,
                                operator: AttributeOperator::Matches(value),
                            });
                            self.skip_spaces_and_comments();
                            self.advance(1);
                        },
                        Some('~') => {
                            self.advance(2);
                            self.skip_spaces_and_comments();
                            let value = self.attribute_value();
                            selec.attributes.push(Attribute {
                                name,
                                operator: AttributeOperator::Contains(value),
                            });
                            self.skip_spaces_and_comments();
                            self.advance(1);
                        },
                        Some('|') => {
                            self.advance(2);
                            self.skip_spaces_and_comments();
                            let value = self.attribute_value();
                            selec.attributes.push(Attribute {
                                name,
                                operator: AttributeOperator::StartsWith(value),
                            });
                            self.skip_spaces_and_comments();
                            self.advance(1);
                        },
                        _ => {
                            self.advance(2);
                            self.skip_spaces_and_comments();
                            self.advance(1);
                            supported = false;
                        },
                    }

                },
                Some(':') => {
                    self.advance(1);
                    if self.next() == Some(':') {
                        supported = false;
                        self.advance(1);
                    }
                    let offset = self.offset;
                    self.skip_ident();
                    match &self.input[offset..self.offset] {
                        "first-child" => {
                            selec.pseudo_classes.push(PseudoClass::FirstChild);
                        },
                        "last-child" => {
                            selec.pseudo_classes.push(PseudoClass::LastChild);
                        },
                        _ => {
                            supported = false;
                        },
                    }
                    if self.next() == Some('(') {
                        self.advance_while(|&c| c != ')');
                        self.advance(1);
                    }
                },
                Some('*') => {
                    self.advance(1);
                },
                _ => {
                    let offset = self.offset;
                    self.skip_ident();
                    if self.offset > offset {
                        selec.tag_name = Some(self.input[offset..self.offset].to_string());
                    } else {
                        let offset = self.offset;
                        self.skip_spaces_and_comments();

                        s.simple_selectors.push(selec);
                        s.combinators.push(comb);
                        selec = SimpleSelector::default();

                        match self.next() {
                            Some(',') => {
                                self.advance(1);
                                self.skip_spaces_and_comments();
                                if supported {
                                    selectors.push(s);
                                }
                                s = Selector::default();
                                comb = Combinator::None;
                                supported = true;
                            },
                            Some('{') => {
                                self.advance(1);
                                break;
                            },
                            Some('>') => {
                                self.advance(1);
                                self.skip_spaces_and_comments();
                                comb = Combinator::Child;
                            },
                            Some('+') => {
                                self.advance(1);
                                self.skip_spaces_and_comments();
                                comb = Combinator::NextSibling;
                            },
                            Some('~') => {
                                self.advance(1);
                                self.skip_spaces_and_comments();
                                comb = Combinator::SubsequentSibling;
                            },
                            _ => {
                                if self.offset > offset {
                                    comb = Combinator::Descendant;
                                } else {
                                    self.advance(1);
                                }
                            },
                        }
                    }
                }
            }
        }

        if supported {
            selectors.push(s);
        }

        selectors
    }

    pub fn parse_declarations(&mut self) -> Vec<Declaration> {
        let mut declarations = Vec::new();
        let mut d = Declaration::default();

        while !self.eof() {
            self.skip_spaces_and_comments();

            match self.next() {
                Some(':') => {
                    self.advance(1);
                    self.skip_spaces_and_comments();
                    let offset = self.offset;

                    while !self.eof() {
                        self.advance_while(|&c| c != '"' &&
                                                c != ';' &&
                                                c != '}' &&
                                                c != '!');
                        match self.next() {
                            Some('"') => {
                                self.advance(1);
                                self.advance_while(|&c| c != '"');
                                self.advance(1);
                            },
                            Some('!') => {
                                d.important = true;
                                break;
                            },
                            _ => break,
                        }
                    }

                    d.value = self.input[offset..self.offset].trim().to_string();
                    if d.important {
                        self.advance_while(|&c| c != ';' &&
                                                c != '}');
                    }
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
                _ => {
                    let offset = self.offset;
                    self.skip_ident();
                    if self.offset > offset {
                        d.name = self.input[offset..self.offset].trim().to_string();
                    } else {
                        self.advance(1);
                    }
                }
            }
        }

        if !d.name.is_empty() {
            declarations.push(d);
        }

        declarations
    }

    fn parse_rules(&mut self, rules: &mut Vec<Rule>) {
        let selectors = self.parse_selectors();
        let declarations = self.parse_declarations();
        for selector in selectors.into_iter() {
            rules.push(Rule {
                selector,
                declarations: declarations.clone(),
            });
        }
    }

    pub fn parse(&mut self) -> StyleSheet {
        let mut rules = Vec::new();

        while !self.eof() {
            self.skip_spaces_and_comments();

            match self.next() {
                None => break,
                Some('@') => self.skip_at_rule(),
                _ => self.parse_rules(&mut rules),
            }
        }

        StyleSheet { rules }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_css() {
        let text = "a, .b { b: c; d: e }";
        let css = CssParser::new(text).parse();
        println!("{:?}", css);
    }

    #[test]
    fn combinators_css() {
        let text = "a#i.j.k > b { b: c } a + .l { u: v } a { x: y }";
        let css = CssParser::new(text).parse();
        println!("{:?}", css);
    }
}
