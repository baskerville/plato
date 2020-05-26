use fxhash::FxHashMap;
use super::dom::Node;
use super::css::{CssParser, Rule, Selector, SimpleSelector, Specificity};

pub type PropertyMap = FxHashMap<String, String>;
pub type Stylesheet = [Rule];
type MatchedRule<'a> = (Specificity, &'a Rule);

#[cfg(test)]
mod tests {
    use super::specified_values;
    use super::super::css::{CssParser, RuleKind};
    use super::super::xml::XmlParser;

    #[test]
    fn simple_style() {
        let xml1 = XmlParser::new("<a class='c x y' style='c: 7'/>").parse();
        let xml2 = XmlParser::new("<a id='e' class='x y'/>").parse();
        let (mut css1, _) = CssParser::new("a { b: 23 }").parse(RuleKind::Viewer);
        let (mut css2, _) = CssParser::new(".c.x.y { b: 6 }").parse(RuleKind::Document);
        let (mut css3, _) = CssParser::new(".y { b: 2 }").parse(RuleKind::Document);
        css1.append(&mut css3);
        css1.append(&mut css2);
        assert_eq!(specified_values(&xml1, None, None, &css1), [("b".to_string(), "6".to_string()),
                                                    ("c".to_string(), "7".to_string())].iter().cloned().collect());
        assert_eq!(specified_values(&xml2, None, None, &css1), [("b".to_string(), "2".to_string())].iter().cloned().collect());
    }
}

pub fn specified_values(node: &Node, parent: Option<&Node>, sibling: Option<&Node>, stylesheet: &Stylesheet) -> PropertyMap {
    let mut props = FxHashMap::default();
    let mut rules = matching_rules(node, parent, sibling, stylesheet);

    rules.sort_by(|&(sa, ra), &(sb, rb)| {
        if ra.kind == rb.kind {
            sa.cmp(&sb)
        } else {
            ra.kind.cmp(&rb.kind)
        }
    });

    for (_, rule) in rules {
        for declaration in &rule.declarations {
            expand_and_insert(&declaration.name, &declaration.value, &mut props);
        }
    }

    let local_declarations = node.attr("style").map(|text| {
        CssParser::new(text).parse_declarations()
    }).unwrap_or_default();

    for declaration in &local_declarations {
        expand_and_insert(&declaration.name, &declaration.value, &mut props);
    }

    props
}

fn matching_rules<'a>(node: &Node, parent: Option<&Node>, sibling: Option<&Node>, stylesheet: &'a Stylesheet) -> Vec<MatchedRule<'a>> {
    stylesheet.iter().filter_map(|rule| match_rule(node, parent, sibling, rule)).collect()
}

fn match_rule<'a>(node: &Node, parent: Option<&Node>, sibling: Option<&Node>, rule: &'a Rule) -> Option<MatchedRule<'a>> {
    rule.selectors.iter().find(|selector| matches(node, parent, sibling, *selector))
        .map(|selector| (selector.specificity(), rule))
}

fn matches(node: &Node, parent: Option<&Node>, sibling: Option<&Node>, selector: &Selector) -> bool {
    match selector {
        Selector::Simple(sel) => matches_simple_selector(node, sel),
        Selector::ParentChild(sel1, sel2) => {
            if let Some(p) = parent {
                matches_simple_selector(p, sel1) && matches_simple_selector(node, sel2)
            } else {
                false
            }
        },
        Selector::Siblings(sel1, sel2) => {
            if let Some(s) = sibling {
                matches_simple_selector(s, sel1) && matches_simple_selector(node, sel2)
            } else {
                false
            }
        },
    }
}

fn matches_simple_selector(node: &Node, selector: &SimpleSelector) -> bool {
    if selector.tag_name.iter().any(|name| node.tag_name() != Some(name)) {
        return false;
    }

    let node_classes = node.classes().unwrap_or_default();
    if selector.classes.iter().any(|class| !node_classes.contains(&**class)) {
        return false;
    }

    if selector.id.iter().any(|id| node.id() != Some(id)) {
        return false;
    }

    true
}

fn expand_and_insert(name: &str, value: &str, props: &mut PropertyMap) {
    match name {
        "margin" | "padding" => {
            let values = value.split_whitespace().collect::<Vec<&str>>();
            match values.len() {
                1 => {
                    props.insert(format!("{}-top", name), value.to_string());
                    props.insert(format!("{}-right", name), value.to_string());
                    props.insert(format!("{}-bottom", name), value.to_string());
                    props.insert(format!("{}-left", name), value.to_string());
                },
                2 => {
                    let vertical = values[0];
                    let horizontal = values[1];
                    props.insert(format!("{}-top", name), vertical.to_string());
                    props.insert(format!("{}-right", name), horizontal.to_string());
                    props.insert(format!("{}-bottom", name), vertical.to_string());
                    props.insert(format!("{}-left", name), horizontal.to_string());
                },
                3 => {
                    let top = values[0];
                    let horizontal = values[1];
                    let bottom = values[2];
                    props.insert(format!("{}-top", name), top.to_string());
                    props.insert(format!("{}-right", name), horizontal.to_string());
                    props.insert(format!("{}-bottom", name), bottom.to_string());
                    props.insert(format!("{}-left", name), horizontal.to_string());
                },
                4 => {
                    let top = values[0];
                    let right = values[1];
                    let bottom = values[2];
                    let left = values[3];
                    props.insert(format!("{}-top", name), top.to_string());
                    props.insert(format!("{}-right", name), right.to_string());
                    props.insert(format!("{}-bottom", name), bottom.to_string());
                    props.insert(format!("{}-left", name), left.to_string());
                },
                _ => (),
            }
        },
        // TODO: border -> border-{top,right,bottom,left}-{width,style,color}
        // border-left -> border-left-{width,style,color}
        // border-style -> border-{top,right,bottom,left}-style
        _ => {
            props.insert(name.to_string(), value.to_string());
        }
    }
}
