use fxhash::FxHashMap;
use super::dom::{XmlTree, NodeId, Attributes};
use super::dom::{text, element, whitespace};

#[derive(Debug)]
pub struct XmlParser<'a> {
    pub input: &'a str,
    pub offset: usize,
}

impl<'a> XmlParser<'a> {
    pub fn new(input: &str) -> XmlParser {
        XmlParser {
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

    fn parse_attributes(&mut self) -> Attributes {
        let mut attrs = FxHashMap::default();
        while !self.eof() {
            self.advance_while(|&c| c.is_xml_whitespace());
            match self.next() {
                Some('>') | Some('/') | None => break,
                _ => {
                    let offset = self.offset;
                    self.advance_while(|&c| c != '=');
                    let key = self.input[offset..self.offset].to_string();
                    self.advance_while(|&c| c != '"' && c != '\'');
                    let quote = self.next().unwrap_or('"');
                    self.advance(1);
                    let offset = self.offset;
                    self.advance_while(|&c| c != quote);
                    let value = self.input[offset..self.offset].to_string();
                    attrs.insert(key, value);
                    self.advance(1);
                }
            }
        }
        attrs
    }

    fn parse_element(&mut self, tree: &mut XmlTree, parent_id: NodeId) {
        let offset = self.offset;
        self.advance_while(|&c| c != '>' && c != '/' && !c.is_xml_whitespace());
        let name = &self.input[offset..self.offset];
        let attributes = self.parse_attributes();

        match self.next() {
            Some('/') => {
                self.advance(2);
                tree.get_mut(parent_id)
                    .append(element(name, offset - 1, attributes));
            },
            Some('>') => {
                self.advance(1);
                let id = tree.get_mut(parent_id)
                             .append(element(name, offset - 1, attributes));
                self.parse_nodes(tree, id);
            },
            _ => (),
        }
    }

    fn parse_nodes(&mut self, tree: &mut XmlTree, parent_id: NodeId) {
        while !self.eof() {
            let offset = self.offset;
            self.advance_while(|&c| c.is_xml_whitespace());

            match self.next() {
                Some('<') => {
                    if self.offset > offset {
                        tree.get_mut(parent_id)
                            .append(whitespace(&self.input[offset..self.offset], offset));
                    }
                    if self.starts_with("</") {
                        self.advance(2);
                        self.advance_while(|&c| c != '>');
                        self.advance(1);
                        break;
                    }
                    self.advance(1);
                    match self.next() {
                        Some('?') => {
                            self.advance(1);
                            self.advance_until("?>");
                        },
                        Some('!') => {
                            self.advance(1);
                            match self.next() {
                                Some('-') => {
                                    self.advance(2);
                                    self.advance_until("-->");
                                },
                                Some('[') => {
                                    self.advance(1);
                                    self.advance_until("]]>");
                                },
                                _ => {
                                    self.advance_while(|&c| c != '>');
                                    self.advance(1);
                                }
                            }
                        },
                        _ => self.parse_element(tree, parent_id),
                    }
                },
                Some(..) => {
                    self.advance_while(|&c| c != '<');
                    tree.get_mut(parent_id)
                        .append(text(&self.input[offset..self.offset], offset));
                },
                None => break,
            }
        }
    }

    pub fn parse(&mut self) -> XmlTree {
        let mut tree = XmlTree::new();
        self.parse_nodes(&mut tree, NodeId::from_index(0));
        tree
    }
}

pub trait XmlExt {
    fn is_xml_whitespace(&self) -> bool;
}

impl XmlExt for char {
    fn is_xml_whitespace(&self) -> bool {
        matches!(self, ' ' | '\t' | '\n' | '\r')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_element() {
        let text = "<a/>";
        let xml = XmlParser::new(text).parse();
        let n = xml.root().first_child().unwrap();
        assert_eq!(n.offset(), 0);
        assert_eq!(n.tag_name(), Some("a"));
    }

    #[test]
    fn test_attributes() {
        let text = r#"<a b="c" d='e"'/>"#;
        let xml = XmlParser::new(text).parse();
        let n = xml.root().first_child().unwrap();
        assert_eq!(n.attribute("b"), Some("c"));
        assert_eq!(n.attribute("d"), Some("e\""));
    }

    #[test]
    fn test_text() {
        let text = "<a>bcd</a>";
        let xml = XmlParser::new(text).parse();
        let child = xml.root().first_child().unwrap().children().next();
        assert_eq!(child.map(|c| c.offset()), Some(3));
        assert_eq!(child.map(|c| c.text()), Some("bcd".to_string()));
    }

    #[test]
    fn test_inbetween_space() {
        let text = "<a><b>x</b> <c>y</c></a>";
        let xml = XmlParser::new(text).parse();
        let child = xml.root().first_child().unwrap()
                       .children().nth(1);
        assert_eq!(child.map(|c| c.text()), Some(" ".to_string()));
    }

    #[test]
    fn test_central_space() {
        let text = "<a><b> </b></a>";
        let xml = XmlParser::new(text).parse();
        assert_eq!(xml.root().text(), " ");
    }
}
