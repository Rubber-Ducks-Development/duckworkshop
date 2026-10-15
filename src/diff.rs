use crate::{Result, xml_document};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, Serialize)]
pub struct Row {
    pub vehicle: String,
    pub field: String,
    pub before: String,
    pub after: String,
    pub status: String,
}
// Exact decimal normalization: no float rounding, even below machine epsilon.
pub fn canonical(text: &str) -> String {
    let s = text.trim();
    let negative = s.starts_with('-');
    let s = s.strip_prefix(['+', '-']).unwrap_or(s);
    let parts: Vec<_> = s.split(['e', 'E']).collect();
    if parts.len() > 2 {
        return text.trim().into();
    }
    let Ok(mut exp) = parts.get(1).copied().unwrap_or("0").parse::<i32>() else {
        return text.trim().into();
    };
    if exp.unsigned_abs() > 10000 {
        return text.trim().into();
    }
    let mut digits = String::new();
    let mut decimal = false;
    let mut count = 0;
    for c in parts[0].chars() {
        if c == '.' && !decimal {
            decimal = true;
        } else if c.is_ascii_digit() {
            digits.push(c);
            count += 1;
            if decimal {
                exp -= 1
            }
        } else {
            return text.trim().into();
        }
    }
    if count == 0 {
        return text.trim().into();
    }
    digits = digits.trim_start_matches('0').into();
    if digits.is_empty() {
        return "0".into();
    }
    while digits.ends_with('0') {
        digits.pop();
        exp += 1;
    }
    format!("{}{}e{}", if negative { "-" } else { "" }, digits, exp)
}
pub fn records(text: &str) -> Result<BTreeMap<String, BTreeMap<String, String>>> {
    let doc = xml_document(text)?;
    if !doc.root_element().has_tag_name("CHandlingDataMgr") {
        return Err("Expected a CHandlingDataMgr handling.meta file.".into());
    }
    let container = doc
        .root_element()
        .children()
        .find(|n| n.has_tag_name("HandlingData"))
        .ok_or("Missing HandlingData")?;
    let mut out = BTreeMap::new();
    fn walk(n: roxmltree::Node<'_, '_>, path: String, fields: &mut BTreeMap<String, String>) {
        for a in n.attributes() {
            fields.insert(format!("{path}/@{}", a.name()), a.value().trim().into());
        }
        let children: Vec<_> = n.children().filter(|n| n.is_element()).collect();
        if children.is_empty() {
            let text = n.text().unwrap_or("").trim();
            if !text.is_empty() {
                fields.insert(path, text.into());
            }
        } else {
            let mut seen = BTreeMap::new();
            for c in &children {
                let name = c.tag_name().name();
                let total = children.iter().filter(|n| n.has_tag_name(name)).count();
                let index = seen.entry(name).or_insert(0);
                let suffix = if total > 1 {
                    format!("[{index}]")
                } else {
                    String::new()
                };
                *index += 1;
                walk(*c, format!("{path}/{name}{suffix}"), fields);
            }
        }
    }
    for item in container.children().filter(|n| n.has_tag_name("Item")) {
        let name = item
            .children()
            .find(|n| n.has_tag_name("handlingName"))
            .and_then(|n| n.text())
            .unwrap_or("")
            .trim();
        if name.is_empty() {
            return Err("Handling entry has no handlingName.".into());
        }
        if out.contains_key(name) {
            return Err(format!(
                "Duplicate handlingName {name}. Resolve duplicates before comparing."
            ));
        }
        let mut fields = BTreeMap::new();
        walk(item, String::new(), &mut fields);
        out.insert(name.into(), fields);
    }
    Ok(out)
}
pub fn compare(a: &str, b: &str) -> Result<Vec<Row>> {
    let left = records(a)?;
    let right = records(b)?;
    let names: BTreeSet<_> = left.keys().chain(right.keys()).collect();
    let empty = BTreeMap::new();
    let mut rows = vec![];
    for name in names {
        let l = left.get(name).unwrap_or(&empty);
        let r = right.get(name).unwrap_or(&empty);
        let fields: BTreeSet<_> = l.keys().chain(r.keys()).collect();
        for field in fields {
            let before = l.get(field);
            let after = r.get(field);
            let status = match (before, after) {
                (None, _) => "added",
                (_, None) => "removed",
                (Some(a), Some(b))
                    if a == b || (field.contains("/@") && canonical(a) == canonical(b)) =>
                {
                    "same"
                }
                _ => "changed",
            };
            rows.push(Row {
                vehicle: name.clone(),
                field: field.clone(),
                before: before.cloned().unwrap_or_default(),
                after: after.cloned().unwrap_or_default(),
                status: status.into(),
            });
        }
    }
    Ok(rows)
}
pub fn csv(rows: &[Row]) -> String {
    let quote = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
    let mut out = "Vehicle,Field,Before,After,Status\r\n".to_string();
    for r in rows {
        out.push_str(
            &[&r.vehicle, &r.field, &r.before, &r.after, &r.status]
                .iter()
                .map(|s| quote(s))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push_str("\r\n")
    }
    out
}
