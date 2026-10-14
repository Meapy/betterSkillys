//! The account server's XML answers (`App/Controllers`, `Shared/database` `ToXml`).

/// An `<Error>` answer, or XML we couldn't read.
pub fn error_text(xml: &str) -> Option<String> {
    let doc = roxmltree::Document::parse(xml).ok()?;
    let root = doc.root_element();
    (root.tag_name().name() == "Error").then(|| root.text().unwrap_or("").trim().to_owned())
}

#[derive(Debug, Clone, PartialEq)]
pub struct ServerEntry {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub admin_only: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CharSummary {
    pub id: i32,
    pub object_type: u16,
    pub level: i32,
    pub dead: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CharList {
    pub next_char_id: i32,
    pub max_num_chars: i32,
    pub chars: Vec<CharSummary>,
    pub servers: Vec<ServerEntry>,
}

fn child_text<'a>(node: roxmltree::Node<'a, 'a>, name: &str) -> Option<&'a str> {
    node.children().find(|c| c.has_tag_name(name)).and_then(|c| c.text()).map(str::trim)
}

fn parse_int(s: &str) -> Option<i64> {
    let s = s.trim();
    match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(hex) => i64::from_str_radix(hex, 16).ok(),
        None => s.parse().ok(),
    }
}

/// `/char/list`.
pub fn parse_char_list(xml: &str) -> Result<CharList, String> {
    if let Some(e) = error_text(xml) {
        return Err(e);
    }
    let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
    let root = doc.root_element();
    if !root.has_tag_name("Chars") {
        return Err(format!("expected <Chars>, got <{}>", root.tag_name().name()));
    }
    let attr = |n: &str| root.attribute(n).and_then(parse_int).unwrap_or(0) as i32;
    let mut list = CharList { next_char_id: attr("nextCharId"), max_num_chars: attr("maxNumChars"), ..Default::default() };
    for c in root.children().filter(|c| c.has_tag_name("Char")) {
        let int = |n: &str| child_text(c, n).and_then(parse_int).unwrap_or(0);
        list.chars.push(CharSummary {
            id: c.attribute("id").and_then(parse_int).unwrap_or(0) as i32,
            object_type: int("ObjectType") as u16,
            level: int("Level") as i32,
            dead: child_text(c, "Dead").is_some_and(|d| d.eq_ignore_ascii_case("true")),
        });
    }
    if let Some(servers) = root.children().find(|c| c.has_tag_name("Servers")) {
        for s in servers.children().filter(|c| c.has_tag_name("Server")) {
            list.servers.push(ServerEntry {
                name: child_text(s, "Name").unwrap_or("").to_owned(),
                host: child_text(s, "DNS").unwrap_or("").to_owned(),
                port: child_text(s, "Port").and_then(parse_int).unwrap_or(2050) as u16,
                admin_only: child_text(s, "AdminOnly").is_some_and(|a| a.eq_ignore_ascii_case("true")),
            });
        }
    }
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn char_list() {
        let xml = r#"<Chars nextCharId="5" maxNumChars="2">
            <Char id="3"><ObjectType>782</ObjectType><Level>20</Level><Dead>False</Dead></Char>
            <Char id="4"><ObjectType>0x0300</ObjectType><Level>1</Level></Char>
            <Servers><Server><Name>Localhost</Name><DNS>127.0.0.1</DNS><Port>2053</Port>
                <AdminOnly>false</AdminOnly></Server></Servers>
        </Chars>"#;
        let list = parse_char_list(xml).unwrap();
        assert_eq!((list.next_char_id, list.max_num_chars), (5, 2));
        assert_eq!(list.chars[0], CharSummary { id: 3, object_type: 782, level: 20, dead: false });
        assert_eq!(list.chars[1].object_type, 0x300);
        assert_eq!(list.servers[0], ServerEntry { name: "Localhost".into(), host: "127.0.0.1".into(), port: 2053, admin_only: false });
    }

    #[test]
    fn error_answer() {
        assert_eq!(parse_char_list("<Error>Account credentials not valid</Error>"), Err("Account credentials not valid".into()));
    }
}
