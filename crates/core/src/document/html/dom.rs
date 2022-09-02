use std::num::NonZeroUsize;
use fxhash::{FxHashMap, FxHashSet};

pub type Attributes = FxHashMap<String, String>;
pub const WRAPPER_TAG_NAME: &str = "anonymous";

#[derive(Debug, Clone)]
pub enum NodeData {
    Root,
    Wrapper(usize),
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
}

impl ElementData {
    fn is_block(&self) -> bool {
        matches!(self.name.as_str(),
                 "address" | "article" | "aside" | "blockquote" | "body" | "head" |
                 "details" | "dialog" | "dd" | "div" | "dl" | "dt" | "fieldset" | "figcaption" |
                 "figure" | "footer" | "form" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "header" |
                 "hgroup" | "hr" | "html" | "li" | "main" | "nav" | "ol" | "p" | "pre" | "section" |
                 "table" | "thead" | "colgroup" | "tbody" | "tfoot" | "tr" | "caption" | "td" | "th" | "ul")
    }
}

impl NodeData {
    fn text(&self) -> Option<&str> {
        match *self {
            NodeData::Text(TextData { ref text, .. }) |
            NodeData::Whitespace(TextData { ref text, .. }) => Some(text),
            _ => None,
        }
    }

    fn offset(&self) -> usize {
        match *self {
            NodeData::Text(TextData { offset, .. }) |
            NodeData::Whitespace(TextData { offset, .. }) |
            NodeData::Element(ElementData { offset, .. }) => offset,
            NodeData::Wrapper(offset) => offset,
            NodeData::Root => 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TextData {
    pub offset: usize,
    pub text: String,
}

pub fn element(name: &str, offset: usize, attributes: Attributes) -> NodeData {
    let colon = name.find(':');
    NodeData::Element(ElementData {
        offset,
        name: name[colon.map(|index| index+1).unwrap_or(0)..].to_string(),
        qualified_name: colon.map(|_| name.to_string()),
        attributes,
    })
}

pub fn text(text: &str, offset: usize) -> NodeData {
    NodeData::Text(TextData {
        offset,
        text: text.to_string(),
    })
}

pub fn whitespace(text: &str, offset: usize) -> NodeData {
    NodeData::Whitespace(TextData {
        offset,
        text: text.to_string(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(NonZeroUsize);

impl NodeId {
    pub fn from_index(n: usize) -> Self {
        NodeId(unsafe { NonZeroUsize::new_unchecked(n + 1) })
    }

    pub fn to_index(self) -> usize {
        self.0.get() - 1
    }
}

#[derive(Debug, Clone)]
pub struct XmlTree {
    nodes: Vec<Node>,
}

#[derive(Debug, Clone)]
pub struct Node {
    data: NodeData,
    parent: Option<NodeId>,
    previous_sibling: Option<NodeId>,
    next_sibling: Option<NodeId>,
    first_child: Option<NodeId>,
    last_child: Option<NodeId>,
}

impl Default for Node {
    fn default() -> Self {
        Node {
            data: NodeData::Root,
            parent: None,
            previous_sibling: None,
            next_sibling: None,
            first_child: None,
            last_child: None,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct NodeRef<'a> {
    pub id: NodeId,
    pub node: &'a Node,
    pub tree: &'a XmlTree,
}

#[derive(Debug)]
pub struct NodeMut<'a> {
    pub id: NodeId,
    pub tree: &'a mut XmlTree,
}

impl XmlTree {
    pub fn new() -> Self {
        XmlTree {
            nodes: vec![Node::default()],
        }
    }

    fn node(&self, id: NodeId) -> &Node {
        unsafe { self.nodes.get_unchecked(id.to_index()) }
    }

    fn node_mut(&mut self, id: NodeId) -> &mut Node {
        unsafe { self.nodes.get_unchecked_mut(id.to_index()) }
    }

    pub fn get(&self, id: NodeId) -> NodeRef {
        NodeRef { id, node: self.node(id), tree: self }
    }

    pub fn get_mut(&mut self, id: NodeId) -> NodeMut {
        NodeMut { id, tree: self }
    }

    pub fn root(&self) -> NodeRef {
        self.get(NodeId::from_index(0))
    }

    pub fn root_mut(&mut self) -> NodeMut {
        self.get_mut(NodeId::from_index(0))
    }

    pub fn wrap_lost_inlines(&mut self) {
        let mut ids = Vec::new();
        let mut known_ids = FxHashSet::default();

        for n in self.root().descendants().filter(|n| n.is_inline()) {
            if known_ids.contains(&n.id) {
                continue;
            }

            let mut first_id = None;
            let mut last_id = None;

            for s in n.previous_siblings() {
                if s.is_block() {
                    first_id = Some(s.next_sibling().unwrap().id);
                    break;
                } else {
                    known_ids.insert(s.id);
                }
            }

            for s in n.next_siblings() {
                if s.is_block() {
                    last_id = Some(s.previous_sibling().unwrap().id);
                    break;
                } else {
                    known_ids.insert(s.id);
                }
            }

            if first_id.is_some() || last_id.is_some() {
                let parent = n.parent().unwrap();
                ids.push([parent.id,
                          first_id.unwrap_or_else(|| parent.node.first_child.unwrap()),
                          last_id.unwrap_or_else(|| parent.node.last_child.unwrap())]);
            }
        }

        for [parent_id, first_id, last_id] in ids {
            let offset = self.node(first_id).data.offset();
            let mut node = self.get_mut(parent_id);
            node.wrap_range(first_id, last_id, NodeData::Wrapper(offset));
        }
    }
}

impl<'a> NodeRef<'a> {
    pub fn parent(&self) -> Option<Self> {
        self.node.parent.map(|id|  self.tree.get(id))
    }

    pub fn parent_element(&self) -> Option<Self> {
        self.ancestors().find(|n| n.is_element() && !n.is_wrapper())
    }

    pub fn previous_sibling(&self) -> Option<Self> {
        self.node.previous_sibling.map(|id| self.tree.get(id))
    }

    pub fn previous_sibling_element(&self) -> Option<NodeRef<'a>> {
        self.previous_sibling_elements().next()
    }

    pub fn next_sibling_element(&self) -> Option<NodeRef<'a>> {
        self.next_sibling_elements().next()
    }

    pub fn next_sibling(&self) -> Option<Self> {
        self.node.next_sibling.map(|id| self.tree.get(id))
    }

    pub fn first_child(&self) -> Option<Self> {
        self.node.first_child.map(|id| self.tree.get(id))
    }

    pub fn last_child(&self) -> Option<Self> {
        self.node.last_child.map(|id| self.tree.get(id))
    }

    pub fn ancestors(&self) -> Ancestors<'a> {
        Ancestors {
            next: self.parent(),
        }
    }

    pub fn ancestor_elements(&self) -> impl Iterator<Item=NodeRef<'a>> {
        self.ancestors().filter(|n| n.is_element() && !n.is_wrapper())
    }

    pub fn previous_siblings(&self) -> PreviousSiblings<'a> {
        PreviousSiblings {
            next: self.previous_sibling(),
        }
    }

    pub fn next_siblings(&self) -> NextSiblings<'a> {
        NextSiblings {
            next: self.next_sibling(),
        }
    }

    pub fn previous_sibling_elements(&self) -> impl Iterator<Item=NodeRef<'a>> {
        self.previous_siblings().filter(|n| n.is_element())
    }

    pub fn next_sibling_elements(&self) -> impl Iterator<Item=NodeRef<'a>> {
        self.next_siblings().filter(|n| n.is_element())
    }

    pub fn children(&self) -> Children<'a> {
        Children {
            next: self.first_child(),
        }
    }

    pub fn descendants(&self) -> Descendants<'a> {
        Descendants {
            root_id: self.id,
            next: self.first_child(),
        }
    }

    pub fn has_children(&self) -> bool {
        self.node.first_child.is_some()
    }

    pub fn is_element(&self) -> bool {
        matches!(self.node.data, NodeData::Element { .. } |
                                 NodeData::Wrapper(..) |
                                 NodeData::Root)
    }

    pub fn is_inline(&self) -> bool {
        match &self.node.data {
            NodeData::Element(e) => !e.is_block(),
            NodeData::Text(..) => true,
            _ => false,
        }
    }

    pub fn is_block(&self) -> bool {
        match &self.node.data {
            NodeData::Element(e) => e.is_block(),
            NodeData::Wrapper(..) | NodeData::Root => true,
            _ => false,
        }
    }

    pub fn is_wrapper(&self) -> bool {
        matches!(self.node.data, NodeData::Wrapper(..))
    }

    pub fn data(&self) -> &'a NodeData {
        &self.node.data
    }

    pub fn offset(&self) -> usize {
        self.node.data.offset()
    }

    pub fn text(&self) -> String {
        self.node.data.text().map(String::from).unwrap_or_else(|| {
            self.descendants()
                .filter_map(|n| n.node.data.text())
                .fold(String::new(), |mut a, b| {
                    a.push_str(b);
                    a
                })
        })
    }

    pub fn tag_name(&self) -> Option<&'a str> {
        match self.node.data {
            NodeData::Element(ElementData { ref name, .. }) => Some(name),
            NodeData::Wrapper(..) => Some(WRAPPER_TAG_NAME),
            _ => None,
        }
    }

    pub fn tag_qualified_name(&self) -> Option<&'a str> {
        match self.node.data {
            NodeData::Element(ElementData { ref qualified_name, .. }) => qualified_name.as_deref(),
            _ => None,
        }
    }

    pub fn attributes(&self) -> Option<&'a Attributes> {
        match self.node.data {
            NodeData::Element(ElementData { ref attributes, .. }) => Some(attributes),
            _ => None,
        }
    }

    pub fn attribute(&self, name: &str) -> Option<&'a str> {
        self.attributes()
            .and_then(|a| a.get(name).map(String::as_str))
    }

    pub fn classes(&self) -> impl Iterator<Item=&'a str> {
        self.attribute("class")
            .unwrap_or("")
            .split_whitespace()
    }

    pub fn id(&self) -> Option<&str> {
        self.attribute("id")
    }

    pub fn find(&self, tag_name: &str) -> Option<Self> {
        self.descendants()
            .find(|n| n.tag_name() == Some(tag_name))
    }

    pub fn find_by_id(&self, id: &str) -> Option<Self> {
        self.descendants()
            .find(|n| n.id() == Some(id))
    }
}

impl<'a> NodeMut<'a> {
    fn node(&mut self) -> &mut Node {
        self.tree.node_mut(self.id)
    }

    pub fn append(&mut self, data: NodeData) -> NodeId {
        let id = NodeId::from_index(self.tree.nodes.len());

        let node = Node {
            data,
            parent: Some(self.id),
            previous_sibling: self.node().last_child,
            next_sibling: None,
            first_child: None,
            last_child: None,
        };

        self.tree.nodes.push(node);

        if let Some(last_child) = self.node().last_child {
            self.tree.node_mut(last_child).next_sibling = Some(id);
        }

        self.node().last_child = Some(id);

        if self.node().first_child.is_none() {
            self.node().first_child = Some(id);
        }

        id
    }

    pub fn wrap_range(&mut self, first_id: NodeId, last_id: NodeId, data: NodeData) {
        let before = self.tree.node(first_id).previous_sibling;
        let after = self.tree.node(last_id).next_sibling;
        let id = NodeId::from_index(self.tree.nodes.len());

        let node = Node {
            data,
            parent: Some(self.id),
            previous_sibling: before,
            next_sibling: after,
            first_child: Some(first_id),
            last_child: Some(last_id),
        };

        self.tree.nodes.push(node);

        if let Some(before_id) = before {
            self.tree.node_mut(before_id).next_sibling = Some(id);
        }

        if let Some(after_id) = after {
            self.tree.node_mut(after_id).previous_sibling = Some(id);
        }

        if let Some(first_child_id) = self.node().first_child {
            if first_child_id == first_id {
                self.node().first_child = Some(id);
            }
        }

        if let Some(last_child_id) = self.node().last_child {
            if last_child_id == last_id {
                self.node().last_child = Some(id);
            }
        }

        self.tree.node_mut(first_id).previous_sibling = None;
        self.tree.node_mut(last_id).next_sibling = None;
        self.tree.node_mut(first_id).parent = Some(id);

        let mut node_id = first_id;
        while let Some(next_id) = self.tree.node(node_id).next_sibling {
            self.tree.node_mut(next_id).parent = Some(id);
            node_id = next_id;
        }
    }
}

pub struct Ancestors<'a> {
    next: Option<NodeRef<'a>>,
}

pub struct NextSiblings<'a> {
    next: Option<NodeRef<'a>>,
}

pub struct PreviousSiblings<'a> {
    next: Option<NodeRef<'a>>,
}

pub struct Children<'a> {
    next: Option<NodeRef<'a>>,
}

pub struct Descendants<'a> {
    root_id: NodeId,
    next: Option<NodeRef<'a>>,
}

impl<'a> Iterator for Ancestors<'a> {
    type Item = NodeRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.next.take();
        self.next = node.as_ref().and_then(|node| node.parent());
        node
    }
}

impl<'a> Iterator for PreviousSiblings<'a> {
    type Item = NodeRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.next.take();
        self.next = node.as_ref().and_then(|node| node.previous_sibling());
        node
    }
}

impl<'a> Iterator for NextSiblings<'a> {
    type Item = NodeRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.next.take();
        self.next = node.as_ref().and_then(|node| node.next_sibling());
        node
    }
}

impl<'a> Iterator for Children<'a> {
    type Item = NodeRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.next.take();
        self.next = node.as_ref().and_then(|node| node.next_sibling());
        node
    }
}

impl<'a> Iterator for Descendants<'a> {
    type Item = NodeRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.next.take();
        if let Some(node) = node {
            self.next = node.first_child()
                            .or_else(|| node.next_sibling())
                            .or_else(|| node.ancestors()
                                            .take_while(|n| n.id != self.root_id)
                                            .find(|n| n.node.next_sibling.is_some())
                                            .and_then(|n| n.next_sibling()));
        }
        node
    }
}
