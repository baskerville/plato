use fxhash::FxHashMap;
use super::dom::{Node, Attributes, text, element, whitespace};

#[derive(Debug)]
pub struct XmlParser<'a> {
    pub input: &'a str,
    pub offset: usize,
    html: bool
}

impl<'a> XmlParser<'a> {
    pub fn new(input: &str, html: bool) -> XmlParser {
        XmlParser {
            input,
            offset: 0,
            html
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

    fn parse_attributes(&mut self) -> Attributes {
        let mut attrs = FxHashMap::default();
        while !self.eof() {
            self.advance_while(|&c| c.is_whitespace());
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

    fn parse_element(&mut self, nodes: &mut Vec<Node>) {
        let offset = self.offset;
        self.advance_while(|&c| c != '>' && c != '/' && !c.is_whitespace());
        let name = &self.input[offset..self.offset];
        let attributes = self.parse_attributes();

        match self.next() {
            Some('/') => {
                self.advance(2);
                nodes.push(element(name, offset - 1, attributes, Vec::new()));
            },
            Some('>') => {
                if self.html {
                    match name {
                        "area"|"base"|"br"|"col"|"command"|"embed"|"hr"|"img"|"input"|"keygen"|"link"|"meta"|"param"|"source"|"track"|"wbr" => {
                            self.advance(1);
                            nodes.push(element(name, offset - 1, attributes, Vec::new()));
                        },
                        _ => {
                            self.advance(1);
                            let children = self.parse_nodes();
                            nodes.push(element(name, offset - 1, attributes, children));
                        }

                    }
                } else {
                    self.advance(1);
                    let children = self.parse_nodes();
                    nodes.push(element(name, offset - 1, attributes, children));
                }
            }
            _ => (),
        }
    }

    fn parse_nodes(&mut self) -> Vec<Node> {
        let mut nodes = Vec::new();

        while !self.eof() {
            let offset = self.offset;
            self.advance_while(|&c| c.is_whitespace());

            match self.next() {
                Some('<') => {
                    if self.offset > offset {
                        nodes.push(whitespace(&self.input[offset..self.offset], offset));
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
                        _ => self.parse_element(&mut nodes),
                    }
                },
                Some(..) => {
                    self.advance_while(|&c| c != '<');
                    nodes.push(text(&self.input[offset..self.offset], offset));
                },
                None => break,
            }
        }
        nodes
    }

    pub fn parse(&mut self) -> Node {
        let mut nodes = self.parse_nodes();
        if nodes.len() == 1 {
            nodes.remove(0)
        } else {
            element("root", 0, FxHashMap::default(), nodes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_element() {
        let text = "<a/>";
        let xml = XmlParser::new(text, false).parse();
        assert_eq!(xml.offset(), 0);
        assert_eq!(xml.tag_name(), Some("a"));
    }

    #[test]
    fn test_attributes() {
        let text = r#"<a b="c" d='e"'/>"#;
        let xml = XmlParser::new(text, false).parse();
        assert_eq!(xml.attr("b"), Some("c"));
        assert_eq!(xml.attr("d"), Some("e\""));
    }

    #[test]
    fn test_text() {
        let text = "<a>bcd</a>";
        let xml = XmlParser::new(text, false).parse();
        let child = xml.child(0);
        assert_eq!(child.map(|c| c.offset()), Some(3));
        assert_eq!(child.and_then(|c| c.text()), Some("bcd"));
    }

    #[test]
    fn test_inbetween_space() {
        let text = "<a><b>x</b> <c>y</c></a>";
        let xml = XmlParser::new(text, false).parse();
        let child = xml.child(1);
        assert_eq!(child.and_then(|c| c.text()), Some(" "));
    }

    #[test]
    fn test_central_space() {
        let text = "<a><b> </b></a>";
        let xml = XmlParser::new(text, false).parse();
        assert_eq!(xml.text(), Some(" "));
    }
}
