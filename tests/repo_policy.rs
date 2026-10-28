//! Repository policy checks (D-03, D-06): CI runs only on a `v*` tag push
//! (release) or by hand (build-only dry run).

use serde_norway::Value;

const WORKFLOW: &str = include_str!("../.github/workflows/release.yml");

/// The workflow's `on:` mapping. YAML 1.1 parsers may read the bare key
/// `on` as boolean `true`, so accept either form.
fn triggers(doc: &Value) -> &serde_norway::Mapping {
    let map = doc.as_mapping().expect("workflow is a mapping");
    map.get("on")
        .or_else(|| map.get(Value::Bool(true)))
        .expect("workflow has an `on:` key")
        .as_mapping()
        .expect("`on:` is a mapping")
}

#[test]
fn workflow_triggers_only_tag_and_dispatch() {
    let doc: Value = serde_norway::from_str(WORKFLOW).expect("valid YAML");
    let on = triggers(&doc);

    let mut keys: Vec<&str> = on.keys().map(|k| k.as_str().expect("string key")).collect();
    keys.sort();
    assert_eq!(
        keys,
        ["push", "workflow_dispatch"],
        "only tag push and manual runs"
    );

    let push = on["push"].as_mapping().expect("`push:` is a mapping");
    let push_keys: Vec<&str> = push.keys().map(|k| k.as_str().unwrap()).collect();
    assert_eq!(
        push_keys,
        ["tags"],
        "push runs on tags only, never branches"
    );
    let tags: Vec<&str> = push["tags"]
        .as_sequence()
        .expect("tags is a list")
        .iter()
        .map(|t| t.as_str().unwrap())
        .collect();
    assert_eq!(tags, ["v*"]);
}

#[test]
fn release_step_runs_only_on_tags() {
    let doc: Value = serde_norway::from_str(WORKFLOW).expect("valid YAML");
    let steps = doc["jobs"]
        .as_mapping()
        .expect("jobs")
        .values()
        .flat_map(|job| job["steps"].as_sequence().expect("steps").iter());
    let mut releases = 0;
    for step in steps {
        let run = step["run"].as_str().unwrap_or_default();
        if run.contains("gh release") {
            releases += 1;
            assert_eq!(step["if"].as_str(), Some("github.ref_type == 'tag'"));
        }
    }
    assert_eq!(releases, 1, "exactly one release step");
}

fn step_names() -> Vec<String> {
    let doc: Value = serde_norway::from_str(WORKFLOW).expect("valid YAML");
    doc["jobs"]["build"]["steps"]
        .as_sequence()
        .expect("steps")
        .iter()
        .filter_map(|s| s["name"].as_str().map(str::to_string))
        .collect()
}

#[test]
fn version_check_runs_before_test_and_build() {
    // A tag that does not match Cargo.toml must fail before a full Windows
    // build is spent on it.
    let names = step_names();
    let pos = |n: &str| {
        names
            .iter()
            .position(|s| s == n)
            .unwrap_or_else(|| panic!("no step named {n}: {names:?}"))
    };
    assert!(pos("Version") < pos("Test"), "{names:?}");
    assert!(pos("Version") < pos("Build"), "{names:?}");
}

#[test]
fn portable_zip_ships_the_license() {
    let doc: Value = serde_norway::from_str(WORKFLOW).expect("valid YAML");
    let zip = doc["jobs"]["build"]["steps"]
        .as_sequence()
        .expect("steps")
        .iter()
        .find(|s| s["name"].as_str() == Some("Portable zip"))
        .expect("Portable zip step");
    let run = zip["run"].as_str().expect("run");
    assert!(run.contains("LICENSE"), "{run}");
}

const INSTALLER: &str = include_str!("../installer/meh.iss");

/// The `[Section]` each line of the Inno script is in, with comments and
/// blank lines left out.
fn iss_lines() -> Vec<(String, &'static str)> {
    let mut section = String::new();
    let mut out = Vec::new();
    for line in INSTALLER.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with(';') || t.starts_with("//") {
            continue;
        }
        if t.starts_with('[') && t.ends_with(']') {
            section = t.trim_matches(['[', ']']).to_ascii_lowercase();
            continue;
        }
        out.push((section.clone(), t));
    }
    out
}

/// D-09 as revised by D-18: setup installs for all users by default (the
/// only path on PCs with application control), still offers "me only",
/// and autostarts through a Startup shortcut, never a Run registry value.
#[test]
fn installer_installs_for_all_users_and_autostarts_by_shortcut() {
    let setup: Vec<&str> = iss_lines()
        .into_iter()
        .filter(|(s, _)| s == "setup")
        .map(|(_, l)| l)
        .collect();
    for want in [
        "PrivilegesRequired=admin",
        "AppMutex=meh-single-instance",
        "DefaultDirName={autopf}\\meh",
    ] {
        assert!(setup.contains(&want), "missing {want}");
    }
    assert!(
        setup
            .iter()
            .any(|l| l.starts_with("PrivilegesRequiredOverridesAllowed=") && l.contains("dialog")),
        "{setup:?}"
    );

    let lines = iss_lines();
    assert!(
        lines.iter().any(|(s, l)| s == "icons"
            && l.contains("{autostartup}\\Meh")
            && l.contains("Tasks: autostart")),
        "autostart shortcut"
    );
    assert!(lines.iter().all(|(s, _)| s != "registry"), "no [Registry]");
    // The Run key may only be named by the [Code] cleanup of an old value.
    for (s, l) in &lines {
        if l.contains("CurrentVersion\\Run") {
            assert!(s == "code" && l.contains("RegDeleteValue"), "{s}: {l}");
        }
    }
    // Never start meh from an elevated, all-users setup: it would run as
    // the admin account (D-18).
    let run: Vec<_> = lines.iter().filter(|(s, _)| s == "run").collect();
    assert!(!run.is_empty());
    for (_, l) in run {
        assert!(l.contains("Check: not IsAdminInstallMode"), "{l}");
    }
}
