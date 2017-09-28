use std::collections::BTreeSet;

pub const PATH_SEPARATOR: char = '.';

pub trait SymbolicPath<'a> {
    fn parent(&self) -> Option<&Self>;
    fn ancestors(&'a self) -> Ancestors<'a>;
    fn is_child_of(&self, other: &Self) -> bool;
    fn is_descendant_of(&self, other: &Self) -> bool;
    fn first_component(&self) -> &Self;
    fn last_component(&self) -> &Self;
    fn depth(&self) -> usize;
}

pub struct Ancestors<'a> {
    next: Option<&'a str>,
}

impl<'a> Iterator for Ancestors<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<&'a str> {
        let next = self.next;
        self.next = self.next.and_then(|path| path.parent());
        next
    }
}

impl<'a> SymbolicPath<'a> for str {
    fn parent(&self) -> Option<&str> {
        self.rfind(PATH_SEPARATOR).map(|index| &self[..index])
    }

    fn ancestors(&'a self) -> Ancestors<'a> {
        Ancestors {
            next: self.parent(),
        }
    }

    fn is_child_of(&self, other: &str) -> bool {
        if let Some(p) = self.parent() {
            p == other
        } else {
            false
        }
    }

    fn is_descendant_of(&self, other: &str) -> bool {
        self.len() >= other.len() && self.split(PATH_SEPARATOR)
                                         .zip(other.split(PATH_SEPARATOR))
                                         .all(|(a, b)| a == b)
    }

    fn first_component(&self) -> &str {
        if let Some(index) = self.find(PATH_SEPARATOR) {
            &self[..index]
        } else {
            self
        }
    }

    fn last_component(&self) -> &str {
        if let Some(index) = self.rfind(PATH_SEPARATOR) {
            &self[index+1..]
        } else {
            self
        }
    }

    fn depth(&self) -> usize {
        self.matches(PATH_SEPARATOR).count()
    }
}
