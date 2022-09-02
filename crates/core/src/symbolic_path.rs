pub const PATH_SEPARATOR: char = '.';

pub trait SymbolicPath<'a> {
    type Owned;
    fn parent(&self) -> Option<&Self>;
    fn ancestors(&'a self) -> Ancestors<'a>;
    fn is_child_of(&self, other: &Self) -> bool;
    fn is_descendant_of(&self, other: &Self) -> bool;
    fn first_component(&self) -> &Self;
    fn last_component(&self) -> &Self;
    fn join(&self, other: &Self) -> Self::Owned;
    fn depth(&self) -> usize;
}

pub struct Ancestors<'a> {
    next: Option<&'a str>,
}

impl<'a> Iterator for Ancestors<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<&'a str> {
        let next = self.next;
        self.next = self.next.and_then(SymbolicPath::parent);
        next
    }
}

impl<'a> SymbolicPath<'a> for str {
    type Owned = String;

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

    #[inline]
    fn join(&self, other: &str) -> String {
        format!("{}{}{}", self, PATH_SEPARATOR, other)
    }

    #[inline]
    fn is_descendant_of(&self, other: &str) -> bool {
        self.starts_with(other) && self[other.len()..].starts_with(PATH_SEPARATOR)
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

#[cfg(test)]
mod tests {
    use super::SymbolicPath;

    #[test]
    fn relationships() {
        assert!("a.b".is_child_of("a"));
        assert!(!"a.bb".is_child_of("a.b"));
        assert!(!"a.b.c".is_child_of("a"));
        assert!(!"a".is_descendant_of("a"));
        assert!(!"a.bb".is_descendant_of("a.b"));
        assert!("a.b.c".is_descendant_of("a"));
        assert!(!"a.b.c".is_descendant_of("b"));
    }

    #[test]
    fn components() {
        assert_eq!("a.b.c".last_component(), "c");
        assert_eq!("a.b.c".first_component(), "a");
        assert_eq!("a".depth(), 0);
        assert_eq!("a.b.c".depth(), 2);
    }
}
