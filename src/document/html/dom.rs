use fxhash::{FxHashMap, FxHashSet};

pub type Attributes = FxHashMap<String, String>;

#[derive(Debug, Clone)]
pub enum Node {
    Element(ElementData),
    Text(TextData),
    Whitespace(TextData),
}

#[derive(Debug, Clone)]
pub struct ElementData {
    pub offset: usize,
    pub name: String,
    pub qualified_name: Option<String>,
    pub attributes: Attributes,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone)]
pub struct TextData {
    pub offset: usize,
    pub text: String,
}

pub fn element(name: &str, offset: usize, attributes: Attributes, children: Vec<Node>) -> Node {
    let colon = name.find(':');
    Node::Element(ElementData {
        offset,
        name: name[colon.map(|index| index+1).unwrap_or(0)..].to_string(),
        qualified_name: colon.map(|_| name.to_string()),
        attributes,
        children
    })
}

pub fn text(text: &str, offset: usize) -> Node {
    Node::Text(TextData {
        offset,
        text: text.to_string(),
    })
}

pub fn whitespace(text: &str, offset: usize) -> Node {
    Node::Whitespace(TextData {
        offset,
        text: text.to_string(),
    })
}

impl Node {
    pub fn tag_name(&self) -> Option<&str> {
        match *self {
            Node::Element(ElementData { ref name, .. }) => Some(name),
            _ => None,
        }
    }

    pub fn tag_qualified_name(&self) -> Option<&str> {
        match *self {
            Node::Element(ElementData { ref qualified_name, .. }) => qualified_name.as_deref(),
            _ => None,
        }
    }

    pub fn children(&self) -> Option<&Vec<Node>> {
        match *self {
            Node::Element(ElementData { ref children, .. }) => Some(children),
            _ => None,
        }
    }

    pub fn child(&self, index: usize) -> Option<&Node> {
        self.children().and_then(|children| children.get(index))
    }

    pub fn attributes(&self) -> Option<&Attributes> {
        match *self {
            Node::Element(ElementData { ref attributes, .. }) => Some(attributes),
            _ => None,
        }
    }

    pub fn text(&self) -> Option<&str> {
        match *self {
            Node::Text(TextData { ref text, .. }) |
            Node::Whitespace(TextData { ref text, .. }) => Some(text),
            Node::Element(ElementData { ref children, .. }) if children.len() == 1 => {
                children.first().and_then(Self::text)
            },
            _ => None,
        }
    }

    pub fn is_block(&self) -> bool {
        match *self {
            Node::Element(ElementData { ref name, .. }) => {
                match name.as_str() {
                    "address" | "anonymous" | "article" | "aside" | "blockquote" | "body" | "head" |
                    "details" | "dialog" | "dd" | "div" | "dl" | "dt" | "fieldset" | "figcaption" |
                    "figure" | "footer" | "form" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "header" |
                    "hgroup" | "hr" | "html" | "li" | "main" | "nav" | "ol" | "p" | "pre" | "section" |
                    "table" | "thead" | "colgroup" | "tbody" | "tfoot" | "tr" | "caption" | "td" | "th" | "ul" => true,
                    _ => false,
                }
            },
            _ => false,
        }
    }

    pub fn is_whitespace(&self) -> bool {
        match *self {
            Node::Whitespace(..) => true,
            _ => false,
        }
    }


    pub fn is_element(&self) -> bool {
        match *self {
            Node::Element(..) => true,
            _ => false,
        }
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attributes().and_then(|a| a.get(name).map(String::as_str))
    }

    pub fn classes(&self) -> Option<FxHashSet<&str>> {
        self.attr("class").map(|t| t.split(' ').collect())
    }

    pub fn id(&self) -> Option<&str> {
        self.attr("id")
    }

    pub fn offset(&self) -> usize {
        match *self {
            Node::Text(TextData { offset, .. }) |
            Node::Whitespace(TextData { offset, .. }) |
            Node::Element(ElementData { offset, .. }) => offset,
        }
    }

    pub fn find(&self, tag_name: &str) -> Option<&Node> {
        match *self {
            Node::Element(ElementData { ref name, ref children, .. }) => {
                if tag_name == name {
                    Some(self)
                } else {
                    for child in children {
                        let result = child.find(tag_name);
                        if result.is_some() {
                            return result;
                        }
                    }
                    None
                }
            },
            _ => None,
        }
    }

    pub fn find_by_id(&self, value: &str) -> Option<&Node> {
        match *self {
            Node::Element(ElementData { ref attributes, ref children, .. }) => {
                if attributes.get("id").map(|v| v == value).unwrap_or(false) {
                    Some(self)
                } else {
                    for child in children {
                        let result = child.find_by_id(value);
                        if result.is_some() {
                            return result;
                        }
                    }
                    None
                }
            },
            _ => None,
        }
    }

    pub fn wrap_lost_inlines(&mut self) {
        if let Node::Element(ElementData { ref mut children, .. }) = self {
            if children.iter().any(Self::is_block) {
                let mut start_index = None;
                let mut end_index = None;
                let mut i = 0;
                while i < children.len() {
                    let is_block = children[i].is_block();
                    let is_whitespace = children[i].is_whitespace();
                    if !is_block {
                        if start_index.is_none() && !is_whitespace {
                            start_index = Some(i);
                        } else if start_index.is_some() {
                            end_index = Some(i);
                        }
                    } else {
                        children[i].wrap_lost_inlines();
                    }
                    if (is_block || i == children.len() - 1) && start_index.is_some() {
                        let j = start_index.unwrap();
                        let k = end_index.unwrap_or(j);
                        let n = k - j + 1;
                        let anon = [element("anonymous", children[j].offset(), FxHashMap::default(), Vec::new())];
                        let inlines = children.splice(j..=k, anon.iter().cloned()).collect();
                        if let Some(Node::Element(ElementData { ref mut children, .. })) = children.get_mut(j) {
                            *children = inlines;
                        }
                        start_index = None;
                        end_index = None;
                        i = i - n + 1;
                    }
                    i += 1;
                }
            }
        }
    }
}
