/// Runtime identification: release version, deployment environment, source commit.
/// Environment comes from `MDMORE_ENV` at runtime; commit is stamped by build.rs.
/// Either falls back to "unknown" rather than guessing.
pub fn version_json() -> String {
    let environment = std::env::var("MDMORE_ENV").unwrap_or_else(|_| "unknown".into());
    let commit = option_env!("MDMORE_COMMIT").unwrap_or("unknown");
    render(env!("CARGO_PKG_VERSION"), &environment, commit)
}

#[derive(serde::Serialize)]
struct VersionInfo<'a> {
    version: &'a str,
    environment: &'a str,
    commit: &'a str,
}

fn render(version: &str, environment: &str, commit: &str) -> String {
    serde_json::to_string(&VersionInfo { version, environment, commit })
        .expect("string fields always serialise")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_emits_version_environment_and_commit() {
        let json = render("1.2.3", "prod", "a3c1077");
        assert_eq!(
            json,
            r#"{"version":"1.2.3","environment":"prod","commit":"a3c1077"}"#
        );
    }

    #[test]
    fn version_json_reports_crate_version() {
        let value: serde_json::Value = serde_json::from_str(&version_json()).unwrap();
        assert_eq!(value["version"], env!("CARGO_PKG_VERSION"));
        assert!(value["environment"].is_string());
        assert!(value["commit"].is_string());
    }
}
