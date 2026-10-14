//! An owned XML element tree, and AS3's rules for turning XML text into numbers, so values read
//! here match what the AIR client reads from the same files.

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    /// Text directly inside this element (trimmed).
    pub text: String,
    pub children: Vec<Element>,
}

impl Element {
    pub fn from_node(node: roxmltree::Node) -> Self {
        let mut text = String::new();
        let mut children = Vec::new();
        for c in node.children() {
            if c.is_element() {
                children.push(Element::from_node(c));
            } else if c.is_text() {
                text.push_str(c.text().unwrap_or(""));
            }
        }
        Element {
            name: node.tag_name().name().to_owned(),
            attrs: node.attributes().map(|a| (a.name().to_owned(), a.value().to_owned())).collect(),
            text: text.trim().to_owned(),
            children,
        }
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    pub fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|c| c.name == name)
    }

    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }

    /// `xml.hasOwnProperty("Name")`.
    pub fn has(&self, name: &str) -> bool {
        self.child(name).is_some()
    }

    /// `int(xml.Name)`, or `default` without the child.
    pub fn int(&self, name: &str, default: i32) -> i32 {
        self.child(name).map_or(default, |c| as3_int(&c.text))
    }

    /// `Number(xml.Name)`, or `default` without the child.
    pub fn number(&self, name: &str, default: f64) -> f64 {
        self.child(name).map_or(default, |c| as3_number(&c.text))
    }

    pub fn text_of(&self, name: &str) -> Option<&str> {
        self.child(name).map(|c| c.text.as_str())
    }

    pub fn attr_int(&self, name: &str, default: i32) -> i32 {
        self.attr(name).map_or(default, as3_int)
    }

    pub fn attr_number(&self, name: &str, default: f64) -> f64 {
        self.attr(name).map_or(default, as3_number)
    }
}

/// AS3 `Number(string)`: trimmed decimal or `0x` hex; empty is 0; anything else NaN.
pub fn as3_number(s: &str) -> f64 {
    let s = s.trim();
    if s.is_empty() {
        return 0.0;
    }
    let (neg, body) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let value = if let Some(hex) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        if neg {
            // AS3 rejects a sign before a hex literal.
            return f64::NAN;
        }
        u64::from_str_radix(hex, 16).map(|v| v as f64).unwrap_or(f64::NAN)
    } else {
        body.parse::<f64>().unwrap_or(f64::NAN)
    };
    if neg { -value } else { value }
}

/// AS3 `int(string)`: the number truncated and wrapped to 32 bits; NaN and infinities are 0.
pub fn as3_int(s: &str) -> i32 {
    to_int32(as3_number(s))
}

pub fn to_int32(v: f64) -> i32 {
    if !v.is_finite() {
        return 0;
    }
    (v.trunc() as i64 as u64 as u32) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as3_number_rules() {
        assert_eq!(as3_int("0x1a"), 26);
        assert_eq!(as3_int(" 12 "), 12);
        assert_eq!(as3_int(""), 0);
        assert_eq!(as3_int("abc"), 0);
        assert_eq!(as3_int("1.9"), 1);
        assert_eq!(as3_int("-1.9"), -1);
        assert_eq!(as3_int("0xffffffff"), -1);
        assert_eq!(as3_number(".5"), 0.5);
        assert!(as3_number("-0x10").is_nan());
    }
}
