/// The result of parsing a `get` argument.
pub struct Target {
    pub host: String,
    /// Path components after the host. Depth is unrestricted
    /// (e.g. GitLab subgroups).
    pub path: Vec<String>,
}

impl Target {
    pub fn is_github(&self) -> bool {
        self.host == "github.com"
    }

    /// `owner/repo` (for github.com) or `group/sub/repo`.
    pub fn path_str(&self) -> String {
        self.path.join("/")
    }

    pub fn https_url(&self) -> String {
        format!("https://{}/{}", self.host, self.path_str())
    }
}

/// Accepted forms:
/// - `owner/repo` … host is github.com
/// - `host/owner/repo` … the first component is a host when it contains `.`;
///   the rest may be arbitrarily deep
/// - URL … `https://`, `ssh://`, or scp form (`git@host:owner/repo`)
pub fn parse(spec: &str) -> Result<Target, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err("empty spec".to_string());
    }

    if let Some(rest) = strip_scheme(spec) {
        return from_parts(&split(rest), spec, true);
    }

    // scp form: [user@]host:path
    if let Some((before, after)) = spec.split_once(':')
        && !before.contains('/')
        && !after.starts_with('/')
    {
        let host = before.rsplit('@').next().unwrap_or(before);
        let mut parts = vec![host.to_string()];
        parts.extend(split(after));
        return from_parts(&parts, spec, true);
    }

    from_parts(&split(spec), spec, false)
}

fn strip_scheme(spec: &str) -> Option<&str> {
    for scheme in ["https://", "http://", "ssh://", "git://"] {
        if let Some(rest) = spec.strip_prefix(scheme) {
            // Drop the user part of ssh://git@host/...
            return Some(match rest.split_once('@') {
                Some((user, after)) if !user.contains('/') => after,
                _ => rest,
            });
        }
    }
    None
}

fn split(s: &str) -> Vec<String> {
    s.trim_matches('/')
        .split('/')
        .filter(|p| !p.is_empty())
        .map(|p| p.to_string())
        .collect()
}

fn from_parts(parts: &[String], spec: &str, host_is_explicit: bool) -> Result<Target, String> {
    let mut parts = parts.to_vec();
    if let Some(last) = parts.last_mut()
        && let Some(stripped) = last.strip_suffix(".git")
    {
        *last = stripped.to_string();
    }
    parts.retain(|p| !p.is_empty());

    // A leading `-` would be taken as a flag by gh / git when passed through.
    if let Some(bad) = parts.iter().find(|p| p.starts_with('-')) {
        return Err(format!("component must not start with '-': {bad}"));
    }

    if host_is_explicit {
        if parts.len() < 3 {
            return Err(format!("not in host/owner/repo form: {spec}"));
        }
        let host = parts.remove(0);
        return Ok(Target { host, path: parts });
    }

    match parts.len() {
        0 | 1 => Err(format!("not in owner/repo form: {spec}")),
        2 => Ok(Target {
            host: "github.com".to_string(),
            path: parts,
        }),
        _ => {
            // With three or more components, the first is treated as a host
            // only when it contains `.`.
            if !parts[0].contains('.') {
                return Err(format!(
                    "first component does not look like a host (no `.`): {spec}"
                ));
            }
            let host = parts.remove(0);
            Ok(Target { host, path: parts })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(spec: &str) -> (String, String) {
        let t = parse(spec).expect(spec);
        let path = t.path_str();
        (t.host, path)
    }

    #[test]
    fn owner_repo() {
        assert_eq!(ok("cli/cli"), ("github.com".into(), "cli/cli".into()));
    }

    #[test]
    fn host_owner_repo() {
        assert_eq!(
            ok("gitlab.com/foo/bar"),
            ("gitlab.com".into(), "foo/bar".into())
        );
    }

    #[test]
    fn subgroup_keeps_depth() {
        assert_eq!(
            ok("gitlab.com/group/sub/repo"),
            ("gitlab.com".into(), "group/sub/repo".into())
        );
    }

    #[test]
    fn https_url() {
        assert_eq!(
            ok("https://github.com/cli/cli"),
            ("github.com".into(), "cli/cli".into())
        );
    }

    #[test]
    fn scp_url() {
        assert_eq!(
            ok("git@github.com:cli/cli.git"),
            ("github.com".into(), "cli/cli".into())
        );
    }

    #[test]
    fn ssh_url() {
        assert_eq!(
            ok("ssh://git@gitlab.com/group/sub/repo.git"),
            ("gitlab.com".into(), "group/sub/repo".into())
        );
    }

    #[test]
    fn dotless_three_parts_is_error() {
        assert!(parse("foo/bar/baz").is_err());
    }

    #[test]
    fn single_part_is_error() {
        assert!(parse("cli").is_err());
    }
}
