//! The account server's XML answers (`App/Controllers`, `Shared/database` `ToXml`).

/// An `<Error>` answer, or XML we couldn't read.
pub fn error_text(xml: &str) -> Option<String> {
    let doc = roxmltree::Document::parse(xml).ok()?;
    let root = doc.root_element();
    (root.tag_name().name() == "Error").then(|| root.text().unwrap_or("").trim().to_owned())
}

/// A `<Success/>` answer (register, delete), or the `<Error>`'s text.
pub fn success(xml: &str) -> Result<(), String> {
    if let Some(e) = error_text(xml) {
        return Err(e);
    }
    let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
    if doc.root_element().has_tag_name("Success") { Ok(()) } else { Err("unexpected answer".into()) }
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
    pub exp: i32,
    /// `<CurrentFame>`.
    pub fame: i32,
    /// Item types, -1 for empty; the first four are worn.
    pub equipment: Vec<i32>,
    pub tex1: i32,
    pub tex2: i32,
    /// The skin (`<Texture>`), 0 for none.
    pub skin: i32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CharList {
    pub next_char_id: i32,
    pub max_num_chars: i32,
    pub chars: Vec<CharSummary>,
    pub servers: Vec<ServerEntry>,
    /// `<Account>`: the player's name, gold and fame.
    pub name: String,
    pub credits: i32,
    pub fame: i32,
    /// `<ClassStats>`: the best level and fame reached with each class.
    pub class_stats: std::collections::HashMap<u16, (i32, i32)>,
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
            exp: int("Exp") as i32,
            fame: int("CurrentFame") as i32,
            equipment: child_text(c, "Equipment")
                .map(|e| e.split(',').map(|i| parse_int(i).unwrap_or(-1) as i32).collect())
                .unwrap_or_default(),
            tex1: int("Tex1") as i32,
            tex2: int("Tex2") as i32,
            skin: int("Texture") as i32,
        });
    }
    if let Some(account) = root.children().find(|c| c.has_tag_name("Account")) {
        list.name = child_text(account, "Name").unwrap_or("").to_owned();
        list.credits = child_text(account, "Credits").and_then(parse_int).unwrap_or(0) as i32;
        if let Some(stats) = account.children().find(|c| c.has_tag_name("Stats")) {
            list.fame = child_text(stats, "Fame").and_then(parse_int).unwrap_or(0) as i32;
            for cs in stats.children().filter(|c| c.has_tag_name("ClassStats")) {
                let t = cs.attribute("objectType").and_then(parse_int).unwrap_or(0) as u16;
                let best = |n: &str| child_text(cs, n).and_then(parse_int).unwrap_or(0) as i32;
                list.class_stats.insert(t, (best("BestLevel"), best("BestFame")));
            }
        }
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
        assert_eq!((list.chars[0].id, list.chars[0].object_type, list.chars[0].level, list.chars[0].dead), (3, 782, 20, false));
        assert_eq!(list.chars[1].object_type, 0x300);
        assert_eq!(list.servers[0], ServerEntry { name: "Localhost".into(), host: "127.0.0.1".into(), port: 2053, admin_only: false });
    }

    #[test]
    fn account_and_equipment() {
        let xml = r#"<Chars nextCharId="2" maxNumChars="2">
            <Char id="1"><ObjectType>782</ObjectType><Equipment>2580, -1, 0x0A14</Equipment>
                <CurrentFame>12</CurrentFame><Tex1>16777216</Tex1></Char>
            <Account><Name>Tester</Name><Credits>50</Credits>
                <Stats><ClassStats objectType="0x030e"><BestLevel>7</BestLevel><BestFame>3</BestFame></ClassStats>
                <Fame>99</Fame></Stats></Account>
        </Chars>"#;
        let list = parse_char_list(xml).unwrap();
        assert_eq!(list.chars[0].equipment, vec![2580, -1, 0xA14]);
        assert_eq!((list.chars[0].fame, list.chars[0].tex1), (12, 16777216));
        assert_eq!((list.name.as_str(), list.credits, list.fame), ("Tester", 50, 99));
        assert_eq!(list.class_stats[&0x30e], (7, 3));
        assert_eq!(success("<Success/>"), Ok(()));
        assert_eq!(success("<Error>Duplicated name</Error>"), Err("Duplicated name".into()));
    }

    #[test]
    fn error_answer() {
        assert_eq!(parse_char_list("<Error>Account credentials not valid</Error>"), Err("Account credentials not valid".into()));
    }
}
