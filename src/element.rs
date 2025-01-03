// RustyXML
// Copyright 2013-2016 RustyXML developers
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::element_builder::{BuilderError, ElementBuilder};
use crate::parser::Parser;
use crate::{escape, AttrMap, Xml};

use std::collections::HashMap;
use std::fmt;
use std::iter::IntoIterator;
use std::slice;
use std::str::FromStr;

#[derive(Clone, PartialEq, Debug)]
/// A struct representing an XML element
pub struct Element {
    /// The element's name
    pub name: String,
    /// The element's namespace
    pub ns: Option<String>,
    /// The element's attributes
    pub attributes: AttrMap<(String, Option<String>), String>,
    /// The element's child `Xml` nodes
    pub children: Vec<Xml>,
    /// The prefixes set for known namespaces
    pub(crate) prefixes: HashMap<String, String>,
    /// The element's default namespace
    pub(crate) default_ns: Option<String>,
}

fn fmt_elem(
    elem: &Element,
    parent: Option<&Element>,
    all_prefixes: &HashMap<String, String>,
    f: &mut fmt::Formatter,
) -> fmt::Result {
    let mut all_prefixes = all_prefixes.clone();
    all_prefixes.extend(elem.prefixes.clone().into_iter());

    // Do we need a prefix?
    if elem.ns != elem.default_ns {
        let prefix = all_prefixes
            .get(elem.ns.as_ref().map_or("", |x| &x[..]))
            .expect("No namespace prefix bound");
        write!(f, "<{}:{}", *prefix, elem.name)?;
    } else {
        write!(f, "<{}", elem.name)?;
    }

    // Do we need to set the default namespace ?
    if !elem
        .attributes
        .iter()
        .any(|(&(ref name, _), _)| name == "xmlns")
    {
        match (parent, &elem.default_ns) {
            // No parent, namespace is not empty
            (None, &Some(ref ns)) => write!(f, " xmlns='{}'", *ns)?,
            // Parent and child namespace differ
            (Some(parent), ns) if parent.default_ns != *ns => {
                write!(f, " xmlns='{}'", ns.as_ref().map_or("", |x| &x[..]))?
            }
            _ => (),
        }
    }

    for (&(ref name, ref ns), value) in &elem.attributes {
        match *ns {
            Some(ref ns) => {
                let prefix = all_prefixes.get(ns).expect("No namespace prefix bound");
                write!(f, " {}:{}='{}'", *prefix, name, escape(value))?
            }
            None => write!(f, " {}='{}'", name, escape(value))?,
        }
    }

    if elem.children.is_empty() {
        write!(f, "/>")?;
    } else {
        write!(f, ">")?;
        for child in &elem.children {
            match *child {
                Xml::ElementNode(ref child) => fmt_elem(child, Some(elem), &all_prefixes, f)?,
                ref o => fmt::Display::fmt(o, f)?,
            }
        }
        if elem.ns != elem.default_ns {
            let prefix = all_prefixes
                .get(elem.ns.as_ref().unwrap())
                .expect("No namespace prefix bound");
            write!(f, "</{}:{}>", *prefix, elem.name)?;
        } else {
            write!(f, "</{}>", elem.name)?;
        }
    }

    Ok(())
}

impl fmt::Display for Element {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt_elem(self, None, &HashMap::new(), f)
    }
}

/// An iterator returning filtered child `Element`s of another `Element`
pub struct ChildElements<'a, 'b> {
    elems: slice::Iter<'a, Xml>,
    name: &'b str,
    ns: Option<&'b str>,
}

impl<'a, 'b> Iterator for ChildElements<'a, 'b> {
    type Item = &'a Element;

    fn next(&mut self) -> Option<&'a Element> {
        let (name, ns) = (self.name, self.ns);
        self.elems.by_ref().find_map(|child| {
            if let Xml::ElementNode(ref elem) = *child {
                if name == elem.name && ns == elem.ns.as_ref().map(|x| &x[..]) {
                    return Some(elem);
                }
            }
            None
        })
    }
}

impl Element {
    /// Create a new `Element`, with specified name and namespace.
    /// Attributes are specified as a `Vec` of `(name, namespace, value)` tuples.
    pub fn new<A>(name: String, ns: Option<String>, attrs: A) -> Element
    where
        A: IntoIterator<Item = (String, Option<String>, String)>,
    {
        let mut prefixes = HashMap::with_capacity(2);
        prefixes.insert(
            "http://www.w3.org/XML/1998/namespace".to_owned(),
            "xml".to_owned(),
        );
        prefixes.insert(
            "http://www.w3.org/2000/xmlns/".to_owned(),
            "xmlns".to_owned(),
        );

        let attributes: AttrMap<_, _> = attrs
            .into_iter()
            .map(|(name, ns, value)| ((name, ns), value))
            .collect();

        Element {
            name,
            ns: ns.clone(),
            default_ns: ns,
            prefixes,
            attributes,
            children: Vec::new(),
        }
    }

    /// Returns the character and CDATA contained in the element.
    pub fn content_str(&self) -> String {
        let mut res = String::new();
        for child in &self.children {
            match *child {
                Xml::ElementNode(ref elem) => res.push_str(&elem.content_str()),
                Xml::CharacterNode(ref data) | Xml::CDATANode(ref data) => res.push_str(data),
                _ => (),
            }
        }
        res
    }

    /// Gets an attribute with the specified name and namespace. When an attribute with the
    /// specified name does not exist `None` is returned.
    pub fn get_attribute<'a>(&'a self, name: &str, ns: Option<&str>) -> Option<&'a str> {
        self.attributes
            .get(&(name.to_owned(), ns.map(|x| x.to_owned())))
            .map(|x| &x[..])
    }

    /// Sets the attribute with the specified name and namespace.
    /// Returns the original value.
    pub fn set_attribute(
        &mut self,
        name: String,
        ns: Option<String>,
        value: String,
    ) -> Option<String> {
        self.attributes.insert((name, ns), value)
    }

    /// Remove the attribute with the specified name and namespace.
    /// Returns the original value.
    pub fn remove_attribute(&mut self, name: &str, ns: Option<&str>) -> Option<String> {
        self.attributes
            .remove(&(name.to_owned(), ns.map(|x| x.to_owned())))
    }

    /// Gets the first child `Element` with the specified name and namespace. When no child
    /// with the specified name exists `None` is returned.
    pub fn get_child<'a>(&'a self, name: &str, ns: Option<&str>) -> Option<&'a Element> {
        self.get_children(name, ns).next()
    }

    /// Get all children `Element` with the specified name and namespace. When no child
    /// with the specified name exists an empty vetor is returned.
    pub fn get_children<'a, 'b>(
        &'a self,
        name: &'b str,
        ns: Option<&'b str>,
    ) -> ChildElements<'a, 'b> {
        ChildElements {
            elems: self.children.iter(),
            name,
            ns,
        }
    }

    /// Appends a child element. Returns a reference to the added element.
    pub fn tag(&mut self, child: Element) -> &mut Element {
        self.children.push(Xml::ElementNode(child));
        match self.children.last_mut() {
            Some(Xml::ElementNode(ref mut elem)) => elem,
            _ => unreachable!("Could not get reference to just added element!"),
        }
    }

    /// Appends a child element. Returns a mutable reference to self.
    pub fn tag_stay(&mut self, child: Element) -> &mut Element {
        self.children.push(Xml::ElementNode(child));
        self
    }

    /// Appends characters. Returns a mutable reference to self.
    pub fn text(&mut self, text: String) -> &mut Element {
        self.children.push(Xml::CharacterNode(text));
        self
    }

    /// Appends CDATA. Returns a mutable reference to self.
    pub fn cdata(&mut self, text: String) -> &mut Element {
        self.children.push(Xml::CDATANode(text));
        self
    }

    /// Appends a comment. Returns a mutable reference to self.
    pub fn comment(&mut self, text: String) -> &mut Element {
        self.children.push(Xml::CommentNode(text));
        self
    }

    /// Appends processing information. Returns a mutable reference to self.
    pub fn pi(&mut self, text: String) -> &mut Element {
        self.children.push(Xml::PINode(text));
        self
    }
}

impl FromStr for Element {
    type Err = BuilderError;
    #[inline]
    fn from_str(data: &str) -> Result<Element, BuilderError> {
        let s = data.as_bytes();
        let mut p = Parser::new(s);
        let mut e = ElementBuilder::new();

        p.find_map(|x| e.handle_event(x))
            .unwrap_or(Err(BuilderError::NoElement))
    }
}

#[cfg(test)]
mod tests {
    use super::Element;

    #[test]
    fn test_get_children() {
        let elem: Element = "<a><b/><c/><b/></a>".parse().unwrap();
        assert_eq!(
            elem.get_children("b", None).collect::<Vec<_>>(),
            vec![
                &Element::new("b".to_owned(), None, vec![]),
                &Element::new("b".to_owned(), None, vec![])
            ],
        );
    }

    #[test]
    fn test_get_child() {
        let elem: Element = "<a><b/><c/><b/></a>".parse().unwrap();
        assert_eq!(
            elem.get_child("b", None),
            Some(&Element::new("b".to_owned(), None, vec![])),
        );
    }

    #[test]
    #[cfg(feature = "ordered_attrs")]
    fn test_attribute_order_new() {
        let input_attributes = vec![
            ("href".to_owned(), None, "/".to_owned()),
            ("title".to_owned(), None, "Home".to_owned()),
            ("target".to_owned(), None, "_blank".to_owned()),
        ];

        // Run this 5 times to make it unlikely this test succeeds at random
        for _ in 0..5 {
            let elem = Element::new("a".to_owned(), None, input_attributes.clone());
            for (expected, actual) in input_attributes.iter().zip(elem.attributes) {
                assert_eq!(expected.0, (actual.0).0);
                assert_eq!(expected.1, (actual.0).1);
                assert_eq!(expected.2, actual.1);
            }
        }
    }

    #[test]
    #[cfg(feature = "ordered_attrs")]
    fn test_attribute_order_added() {
        let input_attributes = vec![
            ("href".to_owned(), None, "/".to_owned()),
            ("title".to_owned(), None, "Home".to_owned()),
            ("target".to_owned(), None, "_blank".to_owned()),
        ];

        // Run this 5 times to make it unlikely this test succeeds at random
        for _ in 0..5 {
            let mut elem = Element::new("a".to_owned(), None, vec![]);
            for attr in &input_attributes {
                elem.set_attribute(attr.0.clone(), attr.1.clone(), attr.2.clone());
            }
            for (expected, actual) in input_attributes.iter().zip(elem.attributes) {
                assert_eq!(expected.0, (actual.0).0);
                assert_eq!(expected.1, (actual.0).1);
                assert_eq!(expected.2, actual.1);
            }
        }
    }
}
