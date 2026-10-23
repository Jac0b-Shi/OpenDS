//! Shared catalog validation used by the application and its build script.
use fluent_syntax::{ast::*, parser};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_BYTES: usize = 2 * 1024 * 1024;
#[derive(Debug, Clone)]
pub struct Catalog {
    pub code: String,
    pub name: String,
    pub variables: BTreeMap<String, BTreeSet<String>>,
}
#[derive(Default)]
struct Definition {
    variables: BTreeSet<String>,
    references: Vec<(String, BTreeSet<String>)>,
}
fn expression(e: &Expression<&str>, d: &mut Definition) {
    match e {
        Expression::Inline(i) => inline(i, d),
        Expression::Select { selector, variants } => {
            inline(selector, d);
            for v in variants {
                pattern(&v.value, d);
            }
        }
    }
}
fn args(a: &CallArguments<&str>, d: &mut Definition) {
    for i in &a.positional {
        inline(i, d);
    }
    for n in &a.named {
        inline(&n.value, d);
    }
}
fn inline(i: &InlineExpression<&str>, d: &mut Definition) {
    match i {
        InlineExpression::VariableReference { id } => {
            d.variables.insert(id.name.into());
        }
        InlineExpression::Placeable { expression: e } => expression(e, d),
        InlineExpression::FunctionReference { arguments, .. } => args(arguments, d),
        InlineExpression::MessageReference { id, attribute } => {
            d.references
                .push((reference(id.name, attribute.as_ref()), BTreeSet::new()));
        }
        InlineExpression::TermReference {
            id,
            attribute,
            arguments,
        } => {
            let mut bound = BTreeSet::new();
            if let Some(a) = arguments {
                args(a, d);
                bound.extend(a.named.iter().map(|n| n.name.name.to_owned()));
            }
            d.references.push((
                reference(&format!("-{}", id.name), attribute.as_ref()),
                bound,
            ));
        }
        _ => {}
    }
}
fn reference(id: &str, attribute: Option<&Identifier<&str>>) -> String {
    attribute.map_or_else(|| id.into(), |a| format!("{id}.{}", a.name))
}
fn pattern(p: &Pattern<&str>, d: &mut Definition) {
    for e in &p.elements {
        if let PatternElement::Placeable { expression: e } = e {
            expression(e, d);
        }
    }
}
fn variables(
    key: &str,
    defs: &BTreeMap<String, Definition>,
    stack: &mut Vec<String>,
) -> Result<BTreeSet<String>, String> {
    if stack.len() >= 64 || stack.iter().any(|s| s == key) {
        return Err(format!("Cyclic or excessively nested reference: {key}"));
    }
    let d = defs
        .get(key)
        .ok_or_else(|| format!("Unknown reference: {key}"))?;
    stack.push(key.into());
    let mut vars = d.variables.clone();
    for (reference, bound) in &d.references {
        vars.extend(
            variables(reference, defs, stack)?
                .difference(bound)
                .cloned(),
        );
    }
    stack.pop();
    Ok(vars)
}
pub fn parse(text: &str) -> Result<Catalog, String> {
    if text.len() > MAX_BYTES || text.contains('\0') {
        return Err("Language file exceeds 2 MiB or contains NUL".into());
    }
    let mut depth = 0usize;
    for c in text.chars() {
        if c == '{' {
            depth += 1;
        }
        if c == '}' {
            depth = depth.saturating_sub(1);
        }
        if depth > 64 {
            return Err("Language file nesting exceeds 64".into());
        }
    }
    let resource = parser::parse(text.trim_start_matches('\u{feff}'))
        .map_err(|(_, e)| format!("Invalid Fluent syntax: {e:?}"))?;
    let mut defs = BTreeMap::new();
    let mut ids = BTreeSet::new();
    let mut meta = BTreeMap::new();
    for entry in &resource.body {
        let (id, value, attributes) = match entry {
            Entry::Message(m) => (m.id.name.to_owned(), m.value.as_ref(), &m.attributes),
            Entry::Term(t) => (format!("-{}", t.id.name), Some(&t.value), &t.attributes),
            _ => continue,
        };
        if !ids.insert(id.clone()) {
            return Err(format!("Duplicate Fluent ID: {id}"));
        }
        if id.starts_with("meta-") {
            let p = value.ok_or_else(|| format!("Missing metadata: {id}"))?;
            let mut s = String::new();
            for e in &p.elements {
                if let PatternElement::TextElement { value } = e {
                    s.push_str(value);
                } else {
                    return Err(format!("Metadata must be literal text: {id}"));
                }
            }
            meta.insert(id.clone(), s);
        }
        if let Some(p) = value {
            let mut d = Definition::default();
            pattern(p, &mut d);
            defs.insert(id.clone(), d);
        }
        for a in attributes {
            let mut d = Definition::default();
            pattern(&a.value, &mut d);
            defs.insert(format!("{id}.{}", a.id.name), d);
        }
    }
    let required = |id: &str| {
        meta.get(id)
            .filter(|s| !s.is_empty())
            .cloned()
            .ok_or_else(|| format!("Missing metadata: {id}"))
    };
    let code = required("meta-language-code")?;
    let name = required("meta-language-name")?;
    if required("meta-schema-version")? != "1" {
        return Err("Unsupported language schema version".into());
    }
    let mut all = BTreeMap::new();
    for key in defs.keys() {
        let v = variables(key, &defs, &mut vec![])?;
        if !key.starts_with(['-', '.']) && !key.starts_with("meta-") && !key.contains('.') {
            all.insert(key.clone(), v);
        }
    }
    Ok(Catalog {
        code,
        name,
        variables: all,
    })
}
pub fn compare(c: &Catalog, english: &Catalog) -> Result<usize, String> {
    let mut found = 0;
    for (key, vars) in &c.variables {
        if let Some(expected) = english.variables.get(key) {
            if vars != expected {
                return Err(format!(
                    "Parameter mismatch for {key}: expected {expected:?}, found {vars:?}"
                ));
            }
            found += 1;
        }
    }
    Ok(found)
}
