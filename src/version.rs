use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Num(String),
    Alpha(String),
}

const PRE_RELEASE: [&str; 5] = ["dev", "alpha", "beta", "pre", "rc"];

fn pre_rank(s: &str) -> Option<usize> {
    PRE_RELEASE.iter().position(|p| *p == s)
}

fn tokenize(v: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut cur_is_digit = false;

    let flush = |cur: &mut String, is_digit: bool, out: &mut Vec<Token>| {
        if cur.is_empty() {
            return;
        }
        if is_digit {
            let trimmed = cur.trim_start_matches('0');
            out.push(Token::Num(if trimmed.is_empty() { "0".into() } else { trimmed.into() }));
        } else {
            out.push(Token::Alpha(cur.to_lowercase()));
        }
        cur.clear();
    };

    for c in v.chars() {
        if c.is_ascii_digit() {
            if !cur.is_empty() && !cur_is_digit {
                flush(&mut cur, cur_is_digit, &mut out);
            }
            cur_is_digit = true;
            cur.push(c);
        } else if c.is_alphabetic() {
            if !cur.is_empty() && cur_is_digit {
                flush(&mut cur, cur_is_digit, &mut out);
            }
            cur_is_digit = false;
            cur.push(c);
        } else {
            flush(&mut cur, cur_is_digit, &mut out);
        }
    }
    flush(&mut cur, cur_is_digit, &mut out);
    out
}

fn cmp_num(a: &str, b: &str) -> Ordering {
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

fn cmp_alpha(a: &str, b: &str) -> Ordering {
    match (pre_rank(a), pre_rank(b)) {
        (Some(x), Some(y)) => x.cmp(&y),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.cmp(b),
    }
}

fn cmp_token(a: &Token, b: &Token) -> Ordering {
    match (a, b) {
        (Token::Num(x), Token::Num(y)) => cmp_num(x, y),
        (Token::Alpha(x), Token::Alpha(y)) => cmp_alpha(x, y),
        (Token::Num(_), Token::Alpha(_)) => Ordering::Greater,
        (Token::Alpha(_), Token::Num(_)) => Ordering::Less,
    }
}

/// Compares two version strings segment by segment
pub fn compare(a: &str, b: &str) -> Ordering {
    let ta = tokenize(a);
    let tb = tokenize(b);

    for (x, y) in ta.iter().zip(tb.iter()) {
        let o = cmp_token(x, y);
        if o != Ordering::Equal {
            return o;
        }
    }

    match ta.len().cmp(&tb.len()) {
        Ordering::Equal => Ordering::Equal,
        Ordering::Greater => tail_order(&ta[tb.len()..]),
        Ordering::Less => tail_order(&tb[ta.len()..]).reverse(),
    }
}

/// Order of a version that has these extra tokens relative to the same version without them
fn tail_order(rest: &[Token]) -> Ordering {
    if rest.iter().all(|t| matches!(t, Token::Num(n) if n == "0")) {
        return Ordering::Equal;
    }
    if let Some(Token::Alpha(w)) = rest.first() {
        if pre_rank(w).is_some() {
            return Ordering::Less;
        }
    }
    Ordering::Greater
}

pub fn latest(versions: &[String]) -> Option<&String> {
    versions.iter().max_by(|a, b| compare(a, b))
}

fn matches(requested: &str, version: &str) -> bool {
    if requested == version {
        return true;
    }
    version
        .strip_prefix(requested)
        .and_then(|rest| rest.chars().next())
        .is_some_and(|c| matches!(c, '.' | '-' | '_' | '+'))
}

pub fn select(available: &[String], requested: Option<&str>) -> Result<String, String> {
    if available.is_empty() {
        return match requested {
            None => Ok(String::new()),
            Some(r) => Err(format!("package does not declare any versions, can't select '{}'", r)),
        };
    }

    let Some(req) = requested else {
        return Ok(latest(available).cloned().unwrap_or_default());
    };

    if let Some(exact) = available.iter().find(|v| v.as_str() == req) {
        return Ok(exact.clone());
    }

    let candidates: Vec<String> = available.iter().filter(|v| matches(req, v)).cloned().collect();
    match latest(&candidates) {
        Some(v) => Ok(v.clone()),
        None => Err(format!(
            "version '{}' not found, available: {}",
            req,
            sorted_desc(available).join(", ")
        )),
    }
}

pub fn sorted_desc(versions: &[String]) -> Vec<String> {
    let mut v = versions.to_vec();
    v.sort_by(|a, b| compare(b, a));
    v
}

#[derive(Debug, PartialEq, Eq)]
pub struct PackageSpec<'a> {
    pub name: &'a str,
    pub version: Option<&'a str>,
}

impl<'a> PackageSpec<'a> {
    pub fn parse(input: &'a str) -> Self {
        match input.rsplit_once('@') {
            Some((name, ver)) if !name.is_empty() => PackageSpec {
                name,
                version: if ver.is_empty() { None } else { Some(ver) },
            },
            _ => PackageSpec { name: input, version: None },
        }
    }
}

// Installed vs available (-P)
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Status {
    UpToDate,
    Upgradable,
    Outdated,
}

pub fn status(installed: &str, available: &[String]) -> Status {
    if !available.iter().any(|v| v == installed) {
        return Status::Outdated;
    }
    match latest(available) {
        Some(l) if compare(installed, l) == Ordering::Less => Status::Upgradable,
        _ => Status::UpToDate,
    }
}

pub fn expand_placeholders(template: &str, version: &str) -> String {
    let mut parts = version.split(['.', '-', '_', '+']);
    let major = parts.next().unwrap_or("");
    let minor = parts.next().unwrap_or("");
    template
        .replace("{version}", version)
        .replace("{major}", major)
        .replace("{minor}", minor)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn numeric_not_lexicographic() {
        assert_eq!(compare("10.0", "6.9"), Ordering::Greater);
        assert_eq!(compare("6.18.2", "6.9.12"), Ordering::Greater);
        assert_eq!(compare("7.1.6", "6.18.2"), Ordering::Greater);
        assert_eq!(compare("6.18.2", "6.18.10"), Ordering::Less);
    }

    #[test]
    fn trailing_zeros_are_equal() {
        assert_eq!(compare("1.0", "1.0.0"), Ordering::Equal);
        assert_eq!(compare("2.12", "2.12.0.0"), Ordering::Equal);
        assert_eq!(compare("2.12.1", "2.12"), Ordering::Greater);
    }

    #[test]
    fn leading_zeros_and_big_numbers() {
        assert_eq!(compare("1.05", "1.5"), Ordering::Equal);
        assert_eq!(compare("20240101", "20231231"), Ordering::Greater);
        assert_eq!(compare("99999999999999999999999", "100000000000000000000000"), Ordering::Less);
    }

    #[test]
    fn prereleases_are_older() {
        assert_eq!(compare("1.0-rc1", "1.0"), Ordering::Less);
        assert_eq!(compare("7.1-rc3", "7.1"), Ordering::Less);
        assert_eq!(compare("1.0-rc1", "1.0-rc2"), Ordering::Less);
        assert_eq!(compare("1.0-beta", "1.0-rc1"), Ordering::Less);
        assert_eq!(compare("1.0-rc1", "0.9.9"), Ordering::Greater);
        assert_eq!(compare("1.0.1", "1.0-rc1"), Ordering::Greater);
    }

    #[test]
    fn letter_suffix_is_newer() {
        assert_eq!(compare("1.1.1w", "1.1.1"), Ordering::Greater);
        assert_eq!(compare("1.1.1w", "1.1.1k"), Ordering::Greater);
        assert_eq!(compare("1.1.1w", "1.1.2"), Ordering::Less);
    }

    #[test]
    fn compare_is_antisymmetric() {
        let all = ["1.0", "1.0.0", "1.0-rc1", "1.1.1w", "6.18.2", "7.1.6", "10", "2.12.1"];
        for a in all {
            for b in all {
                assert_eq!(compare(a, b), compare(b, a).reverse(), "{a} vs {b}");
            }
        }
    }

    #[test]
    fn latest_ignores_array_order() {
        assert_eq!(latest(&v(&["6.18.2", "7.1.6"])).unwrap(), "7.1.6");
        assert_eq!(latest(&v(&["7.1.6", "6.18.2"])).unwrap(), "7.1.6");
        assert_eq!(latest(&v(&["6.9", "10.0", "6.18"])).unwrap(), "10.0");
        assert!(latest(&[]).is_none());
    }

    #[test]
    fn select_default_is_latest() {
        assert_eq!(select(&v(&["6.18.2", "7.1.6"]), None).unwrap(), "7.1.6");
    }

    #[test]
    fn select_exact() {
        assert_eq!(select(&v(&["6.18.2", "7.1.6"]), Some("6.18.2")).unwrap(), "6.18.2");
    }

    #[test]
    fn select_prefix_takes_newest_match() {
        let a = v(&["6.18.2", "6.18.10", "6.12.5", "7.1.6"]);
        assert_eq!(select(&a, Some("6.18")).unwrap(), "6.18.10");
        assert_eq!(select(&a, Some("6")).unwrap(), "6.18.10");
        assert_eq!(select(&a, Some("7")).unwrap(), "7.1.6");
    }

    #[test]
    fn select_prefix_respects_boundary() {
        let a = v(&["6.180.1", "6.18.2"]);
        assert_eq!(select(&a, Some("6.18")).unwrap(), "6.18.2");
        assert!(select(&a, Some("6.1")).is_err());
    }

    #[test]
    fn select_unknown_lists_available_newest_first() {
        let err = select(&v(&["6.18.2", "7.1.6"]), Some("5.0")).unwrap_err();
        assert!(err.contains("'5.0'"), "{err}");
        assert!(err.ends_with("7.1.6, 6.18.2"), "{err}");
    }

    #[test]
    fn select_without_declared_versions() {
        assert_eq!(select(&[], None).unwrap(), "");
        assert!(select(&[], Some("1.0")).is_err());
    }

    // atom@version
    #[test]
    fn spec_plain() {
        assert_eq!(PackageSpec::parse("linux"), PackageSpec { name: "linux", version: None });
        assert_eq!(
            PackageSpec::parse("sys-kernel/linux"),
            PackageSpec { name: "sys-kernel/linux", version: None }
        );
    }

    #[test]
    fn spec_with_version() {
        assert_eq!(
            PackageSpec::parse("linux@7.1.6"),
            PackageSpec { name: "linux", version: Some("7.1.6") }
        );
        assert_eq!(
            PackageSpec::parse("sys-kernel/linux@6.18"),
            PackageSpec { name: "sys-kernel/linux", version: Some("6.18") }
        );
    }

    #[test]
    fn spec_edge_cases() {
        assert_eq!(PackageSpec::parse("linux@"), PackageSpec { name: "linux", version: None });
        assert_eq!(PackageSpec::parse("@1.0"), PackageSpec { name: "@1.0", version: None });
        assert_eq!(
            PackageSpec::parse("a@b@1.0"),
            PackageSpec { name: "a@b", version: Some("1.0") }
        );
    }

    #[test]
    fn status_up_to_date() {
        assert_eq!(status("7.1.6", &v(&["6.18.2", "7.1.6"])), Status::UpToDate);
        assert_eq!(status("2.12.1", &v(&["2.12.1"])), Status::UpToDate);
    }

    #[test]
    fn status_upgradable_is_not_outdated() {
        assert_eq!(status("6.18.2", &v(&["6.18.2", "7.1.6"])), Status::Upgradable);
    }

    #[test]
    fn status_outdated_when_version_was_dropped() {
        assert_eq!(status("6.12.1", &v(&["6.18.2", "7.1.6"])), Status::Outdated);
        assert_eq!(status("2.11", &v(&["2.12.1"])), Status::Outdated);
        assert_eq!(status("1.0", &[]), Status::Outdated);
    }

    #[test]
    fn placeholders() {
        let t = "https://cdn.kernel.org/pub/linux/kernel/v{major}.x/linux-{version}.tar.xz";
        assert_eq!(
            expand_placeholders(t, "7.1.6"),
            "https://cdn.kernel.org/pub/linux/kernel/v7.x/linux-7.1.6.tar.xz"
        );
        assert_eq!(expand_placeholders("x-{major}.{minor}", "6.18.2"), "x-6.18");
    }

    #[test]
    fn placeholders_leave_plain_urls_alone() {
        let u = "https://ftp.gnu.org/gnu/hello/hello-2.12.1.tar.gz";
        assert_eq!(expand_placeholders(u, "2.12.1"), u);
    }
}