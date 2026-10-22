//! GitHub Actions' expression language: what goes between `${{` and `}}`,
//! and the bare conditions of `if:`.
//!
//! The rules are GitHub's ("Evaluate expressions in workflows and
//! actions"): literals `null`, booleans, numbers and single-quoted strings
//! (`''` for a quote); property access with `.`, `[...]` and the `.*`
//! filter; `!`, comparisons, `==`/`!=` with loose typing, `&&` and `||`
//! returning an operand rather than a boolean; string comparisons that
//! ignore case; and the functions `contains`, `startsWith`, `endsWith`,
//! `format`, `join`, `toJSON`, `fromJSON`, `hashFiles` and the four status
//! checks. Values are JSON values; numbers are doubles, as in GitHub.

use serde_json::{Map, Value};

/// `hashFiles(patterns...)`, as the runner answers it.
pub type HashFiles<'a> = &'a dyn Fn(&[String]) -> String;

/// Where an expression is evaluated: the contexts (`github`, `env`,
/// `matrix`, `steps`, ...) and the run so far, for the status functions.
pub struct Scope<'a> {
    pub contexts: &'a Map<String, Value>,
    /// Something before this failed (a step, or a job this one needs).
    pub failed: bool,
    /// The run was cancelled.
    pub cancelled: bool,
    /// `hashFiles(...)`: the runner hashes files in the workspace; where
    /// there is no workspace it is the empty string.
    pub hash_files: Option<HashFiles<'a>>,
}

impl<'a> Scope<'a> {
    pub fn new(contexts: &'a Map<String, Value>) -> Scope<'a> {
        Scope {
            contexts,
            failed: false,
            cancelled: false,
            hash_files: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Ident(String),
    Dot,
    Star,
    LBracket,
    RBracket,
    LParen,
    RParen,
    Comma,
    Not,
    Op(&'static str),
}

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        match c {
            '\'' => {
                let mut s = String::new();
                i += 1;
                loop {
                    match chars.get(i) {
                        None => return Err("a string is not closed".into()),
                        Some('\'') if chars.get(i + 1) == Some(&'\'') => {
                            s.push('\'');
                            i += 2;
                        }
                        Some('\'') => {
                            i += 1;
                            break;
                        }
                        Some(&ch) => {
                            s.push(ch);
                            i += 1;
                        }
                    }
                }
                out.push(Tok::Str(s));
            }
            '.' if !chars.get(i + 1).is_some_and(char::is_ascii_digit)
                || out.last().is_some_and(|t| {
                    matches!(t, Tok::Ident(_) | Tok::RBracket | Tok::RParen | Tok::Star)
                }) =>
            {
                out.push(Tok::Dot);
                i += 1;
            }
            '*' => {
                out.push(Tok::Star);
                i += 1;
            }
            '[' => {
                out.push(Tok::LBracket);
                i += 1;
            }
            ']' => {
                out.push(Tok::RBracket);
                i += 1;
            }
            '(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            ',' => {
                out.push(Tok::Comma);
                i += 1;
            }
            '!' if chars.get(i + 1) == Some(&'=') => {
                out.push(Tok::Op("!="));
                i += 2;
            }
            '!' => {
                out.push(Tok::Not);
                i += 1;
            }
            '=' if chars.get(i + 1) == Some(&'=') => {
                out.push(Tok::Op("=="));
                i += 2;
            }
            '<' | '>' => {
                let eq = chars.get(i + 1) == Some(&'=');
                out.push(Tok::Op(match (c, eq) {
                    ('<', true) => "<=",
                    ('<', false) => "<",
                    ('>', true) => ">=",
                    _ => ">",
                }));
                i += if eq { 2 } else { 1 };
            }
            '&' if chars.get(i + 1) == Some(&'&') => {
                out.push(Tok::Op("&&"));
                i += 2;
            }
            '|' if chars.get(i + 1) == Some(&'|') => {
                out.push(Tok::Op("||"));
                i += 2;
            }
            c if c.is_ascii_digit() || c == '.' || c == '-' || c == '+' => {
                let start = i;
                i += 1;
                while i < chars.len()
                    && (chars[i].is_ascii_alphanumeric()
                        || chars[i] == '.'
                        || ((chars[i] == '-' || chars[i] == '+')
                            && matches!(chars[i - 1], 'e' | 'E')))
                {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                out.push(Tok::Num(
                    parse_number(&text).ok_or_else(|| format!("{text} is not a number"))?,
                ));
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len()
                    && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '-')
                {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                out.push(match word.as_str() {
                    "null" => Tok::Null,
                    "true" => Tok::Bool(true),
                    "false" => Tok::Bool(false),
                    "NaN" => Tok::Num(f64::NAN),
                    "Infinity" => Tok::Num(f64::INFINITY),
                    _ => Tok::Ident(word),
                });
            }
            other => return Err(format!("unexpected {other:?}")),
        }
    }
    Ok(out)
}

fn parse_number(text: &str) -> Option<f64> {
    let t = text.trim();
    if t.is_empty() {
        return Some(0.0);
    }
    let (neg, body) = match t.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let v = if let Some(hex) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        i64::from_str_radix(hex, 16).ok()? as f64
    } else if let Some(oct) = body.strip_prefix("0o") {
        i64::from_str_radix(oct, 8).ok()? as f64
    } else if body == "Infinity" {
        f64::INFINITY
    } else {
        if body
            .chars()
            .any(|c| c.is_alphabetic() && c != 'e' && c != 'E')
        {
            return None;
        }
        body.parse::<f64>().ok()?
    };
    Some(if neg { -v } else { v })
}

#[derive(Debug, Clone)]
enum Node {
    Lit(Value),
    Context(String),
    Prop(Box<Node>, String),
    Index(Box<Node>, Box<Node>),
    Star(Box<Node>),
    Not(Box<Node>),
    Bin(&'static str, Box<Node>, Box<Node>),
    Call(String, Vec<Node>),
}

struct Parser {
    toks: Vec<Tok>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.at)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.at).cloned();
        self.at += 1;
        t
    }
    fn expect(&mut self, t: Tok) -> Result<(), String> {
        match self.next() {
            Some(got) if got == t => Ok(()),
            got => Err(format!("expected {t:?}, found {got:?}")),
        }
    }
    fn or(&mut self) -> Result<Node, String> {
        let mut left = self.and()?;
        while self.peek() == Some(&Tok::Op("||")) {
            self.at += 1;
            left = Node::Bin("||", Box::new(left), Box::new(self.and()?));
        }
        Ok(left)
    }
    fn and(&mut self) -> Result<Node, String> {
        let mut left = self.equality()?;
        while self.peek() == Some(&Tok::Op("&&")) {
            self.at += 1;
            left = Node::Bin("&&", Box::new(left), Box::new(self.equality()?));
        }
        Ok(left)
    }
    fn equality(&mut self) -> Result<Node, String> {
        let mut left = self.comparison()?;
        while let Some(Tok::Op(op @ ("==" | "!="))) = self.peek().cloned() {
            self.at += 1;
            left = Node::Bin(op, Box::new(left), Box::new(self.comparison()?));
        }
        Ok(left)
    }
    fn comparison(&mut self) -> Result<Node, String> {
        let mut left = self.unary()?;
        while let Some(Tok::Op(op @ ("<" | "<=" | ">" | ">="))) = self.peek().cloned() {
            self.at += 1;
            left = Node::Bin(op, Box::new(left), Box::new(self.unary()?));
        }
        Ok(left)
    }
    fn unary(&mut self) -> Result<Node, String> {
        if self.peek() == Some(&Tok::Not) {
            self.at += 1;
            return Ok(Node::Not(Box::new(self.unary()?)));
        }
        self.postfix()
    }
    fn postfix(&mut self) -> Result<Node, String> {
        let mut node = self.primary()?;
        loop {
            match self.peek() {
                Some(Tok::Dot) => {
                    self.at += 1;
                    match self.next() {
                        Some(Tok::Ident(name)) => node = Node::Prop(Box::new(node), name),
                        Some(Tok::Star) => node = Node::Star(Box::new(node)),
                        // `a.true`, `a.null`: names that lex as literals.
                        Some(Tok::Bool(b)) => node = Node::Prop(Box::new(node), b.to_string()),
                        Some(Tok::Null) => node = Node::Prop(Box::new(node), "null".into()),
                        t => return Err(format!("expected a property name, found {t:?}")),
                    }
                }
                Some(Tok::LBracket) => {
                    self.at += 1;
                    if self.peek() == Some(&Tok::Star) {
                        self.at += 1;
                        self.expect(Tok::RBracket)?;
                        node = Node::Star(Box::new(node));
                    } else {
                        let index = self.or()?;
                        self.expect(Tok::RBracket)?;
                        node = Node::Index(Box::new(node), Box::new(index));
                    }
                }
                _ => return Ok(node),
            }
        }
    }
    fn primary(&mut self) -> Result<Node, String> {
        match self.next() {
            Some(Tok::Null) => Ok(Node::Lit(Value::Null)),
            Some(Tok::Bool(b)) => Ok(Node::Lit(Value::Bool(b))),
            Some(Tok::Num(n)) => Ok(Node::Lit(num(n))),
            Some(Tok::Str(s)) => Ok(Node::Lit(Value::String(s))),
            Some(Tok::LParen) => {
                let inner = self.or()?;
                self.expect(Tok::RParen)?;
                Ok(inner)
            }
            Some(Tok::Ident(name)) => {
                if self.peek() == Some(&Tok::LParen) {
                    self.at += 1;
                    let mut args = Vec::new();
                    if self.peek() != Some(&Tok::RParen) {
                        loop {
                            args.push(self.or()?);
                            match self.next() {
                                Some(Tok::Comma) => continue,
                                Some(Tok::RParen) => break,
                                t => return Err(format!("expected , or ), found {t:?}")),
                            }
                        }
                    } else {
                        self.at += 1;
                    }
                    Ok(Node::Call(name.to_ascii_lowercase(), args))
                } else {
                    Ok(Node::Context(name.to_ascii_lowercase()))
                }
            }
            t => Err(format!("unexpected {t:?}")),
        }
    }
}

fn parse(src: &str) -> Result<Node, String> {
    let mut p = Parser {
        toks: lex(src)?,
        at: 0,
    };
    let node = p.or()?;
    if p.at != p.toks.len() {
        return Err(format!("unexpected {:?}", p.toks[p.at]));
    }
    Ok(node)
}

fn num(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() < 9.0e15 {
        Value::from(n as i64)
    } else {
        serde_json::Number::from_f64(n)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

/// GitHub's truthiness.
pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn to_number(v: &Value) -> f64 {
    match v {
        Value::Null => 0.0,
        Value::Bool(b) => f64::from(u8::from(*b)),
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Value::String(s) => parse_number(s).unwrap_or(f64::NAN),
        Value::Array(_) | Value::Object(_) => f64::NAN,
    }
}

/// How a value reads when it lands in a string: `${{ }}` in text.
pub fn to_text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && f.abs() < 9.0e15 => (f as i64).to_string(),
            _ => n.to_string(),
        },
        Value::String(s) => s.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    }
}

fn loose_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::String(x), Value::String(y)) => x.to_lowercase() == y.to_lowercase(),
        (Value::Null, Value::Null) => true,
        (Value::Array(_) | Value::Object(_), _) | (_, Value::Array(_) | Value::Object(_)) => false,
        _ => to_number(a) == to_number(b),
    }
}

fn compare(op: &str, a: &Value, b: &Value) -> bool {
    if let (Value::String(x), Value::String(y)) = (a, b) {
        let (x, y) = (x.to_lowercase(), y.to_lowercase());
        return match op {
            "<" => x < y,
            "<=" => x <= y,
            ">" => x > y,
            _ => x >= y,
        };
    }
    let (x, y) = (to_number(a), to_number(b));
    match op {
        "<" => x < y,
        "<=" => x <= y,
        ">" => x > y,
        _ => x >= y,
    }
}

fn prop(v: &Value, key: &str) -> Value {
    match v {
        Value::Object(m) => m
            .get(key)
            .or_else(|| {
                m.iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(key))
                    .map(|(_, v)| v)
            })
            .cloned()
            .unwrap_or(Value::Null),
        // `.x` on the result of `.*`: the property of each element.
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|i| prop(i, key))
                .filter(|v| !v.is_null())
                .collect(),
        ),
        _ => Value::Null,
    }
}

fn format_fn(args: &[Value]) -> Result<Value, String> {
    let Some(Value::String(fmt)) = args.first() else {
        return Ok(Value::String(args.first().map(to_text).unwrap_or_default()));
    };
    let chars: Vec<char> = fmt.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '{' if chars.get(i + 1) == Some(&'{') => {
                out.push('{');
                i += 2;
            }
            '}' if chars.get(i + 1) == Some(&'}') => {
                out.push('}');
                i += 2;
            }
            '{' => {
                let end = chars[i..]
                    .iter()
                    .position(|&c| c == '}')
                    .ok_or("format: a { is not closed")?;
                let n: String = chars[i + 1..i + end].iter().collect();
                let n: usize = n.trim().parse().map_err(|_| "format: {N} takes a number")?;
                out.push_str(
                    &args
                        .get(n + 1)
                        .map(to_text)
                        .ok_or("format: fewer arguments than placeholders")?,
                );
                i += end + 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    Ok(Value::String(out))
}

fn eval(node: &Node, scope: &Scope<'_>) -> Result<Value, String> {
    Ok(match node {
        Node::Lit(v) => v.clone(),
        Node::Context(name) => scope.contexts.get(name).cloned().unwrap_or(Value::Null),
        Node::Prop(base, key) => prop(&eval(base, scope)?, key),
        Node::Index(base, index) => {
            let base = eval(base, scope)?;
            match (&base, eval(index, scope)?) {
                (Value::Array(items), i) => {
                    let n = to_number(&i);
                    if n >= 0.0 && n.fract() == 0.0 {
                        items.get(n as usize).cloned().unwrap_or(Value::Null)
                    } else {
                        Value::Null
                    }
                }
                (_, key) => prop(&base, &to_text(&key)),
            }
        }
        Node::Star(base) => match eval(base, scope)? {
            Value::Object(m) => Value::Array(m.into_iter().map(|(_, v)| v).collect()),
            Value::Array(items) => Value::Array(items),
            _ => Value::Array(Vec::new()),
        },
        Node::Not(inner) => Value::Bool(!truthy(&eval(inner, scope)?)),
        Node::Bin(op, a, b) => {
            let left = eval(a, scope)?;
            match *op {
                "&&" => {
                    if truthy(&left) {
                        eval(b, scope)?
                    } else {
                        left
                    }
                }
                "||" => {
                    if truthy(&left) {
                        left
                    } else {
                        eval(b, scope)?
                    }
                }
                "==" => Value::Bool(loose_eq(&left, &eval(b, scope)?)),
                "!=" => Value::Bool(!loose_eq(&left, &eval(b, scope)?)),
                op => Value::Bool(compare(op, &left, &eval(b, scope)?)),
            }
        }
        Node::Call(name, args) => {
            let args: Vec<Value> = args
                .iter()
                .map(|a| eval(a, scope))
                .collect::<Result<_, _>>()?;
            call(name, &args, scope)?
        }
    })
}

fn call(name: &str, args: &[Value], scope: &Scope<'_>) -> Result<Value, String> {
    let arg = |i: usize| args.get(i).cloned().unwrap_or(Value::Null);
    let text = |i: usize| to_text(&arg(i)).to_lowercase();
    Ok(match name {
        "success" => Value::Bool(!scope.failed && !scope.cancelled),
        "failure" => Value::Bool(scope.failed && !scope.cancelled),
        "cancelled" => Value::Bool(scope.cancelled),
        "always" => Value::Bool(true),
        "contains" => Value::Bool(match arg(0) {
            Value::Array(items) => items.iter().any(|i| loose_eq(i, &arg(1))),
            _ => text(0).contains(&text(1)),
        }),
        "startswith" => Value::Bool(text(0).starts_with(&text(1))),
        "endswith" => Value::Bool(text(0).ends_with(&text(1))),
        "format" => format_fn(args)?,
        "join" => {
            let sep = if args.len() > 1 {
                to_text(&arg(1))
            } else {
                ",".into()
            };
            match arg(0) {
                Value::Array(items) => {
                    Value::String(items.iter().map(to_text).collect::<Vec<_>>().join(&sep))
                }
                other => Value::String(to_text(&other)),
            }
        }
        "tojson" => Value::String(serde_json::to_string_pretty(&arg(0)).unwrap_or_default()),
        "fromjson" => {
            serde_json::from_str(&to_text(&arg(0))).map_err(|e| format!("fromJSON: {e}"))?
        }
        "hashfiles" => {
            let patterns: Vec<String> = args.iter().map(to_text).collect();
            Value::String(scope.hash_files.map(|f| f(&patterns)).unwrap_or_default())
        }
        other => return Err(format!("there is no function {other}()")),
    })
}

/// Evaluate one expression (without the `${{ }}`).
pub fn evaluate(src: &str, scope: &Scope<'_>) -> Result<Value, String> {
    eval(&parse(src)?, scope)
}

/// Whether an expression calls one of the status functions, which decides
/// whether `if:` gets GitHub's implicit `success() &&`.
fn uses_status(node: &Node) -> bool {
    match node {
        Node::Call(name, args) => {
            matches!(
                name.as_str(),
                "success" | "failure" | "cancelled" | "always"
            ) || args.iter().any(uses_status)
        }
        Node::Prop(b, _) | Node::Star(b) | Node::Not(b) => uses_status(b),
        Node::Index(a, b) | Node::Bin(_, a, b) => uses_status(a) || uses_status(b),
        Node::Lit(_) | Node::Context(_) => false,
    }
}

/// An `if:`: a bare expression or one in `${{ }}`, with `success() &&`
/// in front unless it checks the status itself. Missing is `success()`.
pub fn condition(src: Option<&str>, scope: &Scope<'_>) -> Result<bool, String> {
    let src = src.map(str::trim).unwrap_or("");
    let inner = match src.strip_prefix("${{").and_then(|s| s.strip_suffix("}}")) {
        Some(e) if !e.contains("${{") => e.trim(),
        _ if src.contains("${{") => {
            // Text with expressions in it: the result is a string.
            return Ok(truthy(&Value::String(interpolate(src, scope)?)));
        }
        _ => src,
    };
    if inner.is_empty() {
        return Ok(!scope.failed && !scope.cancelled);
    }
    let node = parse(inner)?;
    let value = eval(&node, scope)?;
    if uses_status(&node) {
        Ok(truthy(&value))
    } else {
        Ok(!scope.failed && !scope.cancelled && truthy(&value))
    }
}

/// Replace every `${{ ... }}` in `text` with its value.
pub fn interpolate(text: &str, scope: &Scope<'_>) -> Result<String, String> {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("${{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 3..];
        let end = closing(after).ok_or_else(|| format!("{text}: a ${{{{ is not closed"))?;
        out.push_str(&to_text(&evaluate(&after[..end], scope)?));
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Where the `}}` that closes an expression is, skipping quoted strings.
fn closing(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut quoted = false;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' => quoted = !quoted,
            b'}' if !quoted && bytes.get(i + 1) == Some(&b'}') => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

/// A value from the workflow with every string in it interpolated. A
/// string that is one whole `${{ }}` keeps the value's type, as GitHub
/// does for `strategy.matrix: ${{ fromJSON(...) }}`.
pub fn interpolate_value(v: &Value, scope: &Scope<'_>) -> Result<Value, String> {
    Ok(match v {
        Value::String(s) => {
            let t = s.trim();
            match t.strip_prefix("${{").and_then(|x| x.strip_suffix("}}")) {
                Some(inner) if !inner.contains("${{") && closing(inner).is_none() => {
                    evaluate(inner, scope)?
                }
                _ => Value::String(interpolate(s, scope)?),
            }
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|i| interpolate_value(i, scope))
                .collect::<Result<_, _>>()?,
        ),
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, v)| Ok((k.clone(), interpolate_value(v, scope)?)))
                .collect::<Result<_, String>>()?,
        ),
        other => other.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx() -> Map<String, Value> {
        json!({
            "github": { "ref": "refs/heads/main", "event_name": "push", "sha": "abc" },
            "matrix": { "os": "ubuntu", "node": 20 },
            "env": { "NAME": "LosOS" },
            "steps": { "build": { "outputs": { "size": "12" }, "outcome": "success" } },
            "needs": { "a": { "result": "success" }, "b": { "result": "failure" } },
            "list": [{ "name": "x" }, { "name": "y" }],
        })
        .as_object()
        .unwrap()
        .clone()
    }

    fn ev(src: &str) -> Value {
        let c = ctx();
        evaluate(src, &Scope::new(&c)).unwrap()
    }

    #[test]
    fn literals_and_operators() {
        assert_eq!(ev("1 == 1.0"), json!(true));
        assert_eq!(ev("'abc' == 'ABC'"), json!(true));
        assert_eq!(ev("'1' == 1"), json!(true));
        assert_eq!(ev("null == 0"), json!(true));
        assert_eq!(ev("'' || 'fallback'"), json!("fallback"));
        assert_eq!(ev("'x' && 'y'"), json!("y"));
        assert_eq!(ev("!0"), json!(true));
        assert_eq!(ev("0x10 > 15"), json!(true));
        assert_eq!(ev("'it''s'"), json!("it's"));
        assert_eq!(ev("-1.5 < 0"), json!(true));
        assert_eq!(ev("(1 == 2) || (2 >= 2)"), json!(true));
    }

    #[test]
    fn contexts() {
        assert_eq!(ev("github.ref"), json!("refs/heads/main"));
        assert_eq!(ev("github['event_name']"), json!("push"));
        assert_eq!(ev("matrix.node"), json!(20));
        assert_eq!(ev("steps.build.outputs.size"), json!("12"));
        assert_eq!(ev("steps.missing.outputs.size"), json!(null));
        assert_eq!(ev("list.*.name"), json!(["x", "y"]));
        assert_eq!(ev("list[1].name"), json!("y"));
        assert_eq!(ev("contains(needs.*.result, 'failure')"), json!(true));
        assert_eq!(ev("GITHUB.REF"), json!("refs/heads/main"));
    }

    #[test]
    fn functions() {
        assert_eq!(ev("contains('Hello world', 'WORLD')"), json!(true));
        assert_eq!(ev("startsWith(github.ref, 'refs/heads/')"), json!(true));
        assert_eq!(ev("endsWith('a.tar.gz', '.GZ')"), json!(true));
        assert_eq!(
            ev("format('{0} on {1}{{x}}', env.NAME, matrix.os)"),
            json!("LosOS on ubuntu{x}")
        );
        assert_eq!(ev("join(list.*.name, '+')"), json!("x+y"));
        assert_eq!(ev("fromJSON('{\"a\":[1,2]}').a[1]"), json!(2));
        assert_eq!(ev("toJSON(matrix.node)"), json!("20"));
        assert_eq!(ev("hashFiles('**/Cargo.lock')"), json!(""));
    }

    #[test]
    fn conditions() {
        let c = ctx();
        let ok = Scope::new(&c);
        let failed = Scope {
            failed: true,
            ..Scope::new(&c)
        };
        assert!(condition(None, &ok).unwrap());
        assert!(!condition(None, &failed).unwrap());
        assert!(condition(Some("github.event_name == 'push'"), &ok).unwrap());
        assert!(!condition(Some("github.event_name == 'push'"), &failed).unwrap());
        assert!(condition(Some("${{ failure() }}"), &failed).unwrap());
        assert!(condition(Some("always()"), &failed).unwrap());
        assert!(!condition(Some("${{ github.ref == 'refs/heads/dev' }}"), &ok).unwrap());
    }

    #[test]
    fn interpolation() {
        let c = ctx();
        let s = Scope::new(&c);
        assert_eq!(
            interpolate("node-${{ matrix.node }} on ${{ matrix.os }}", &s).unwrap(),
            "node-20 on ubuntu"
        );
        assert_eq!(interpolate("${{ '}}' }}", &s).unwrap(), "}}");
        assert_eq!(
            interpolate_value(&json!("${{ fromJSON('[1,2]') }}"), &s).unwrap(),
            json!([1, 2])
        );
        assert!(interpolate("${{ nope( }}", &s).is_err());
    }
}
