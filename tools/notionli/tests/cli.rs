use std::process::Command;

#[test]
fn schema_errors_returns_json() {
    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", temp_dir(), "schema", "errors"])
        .output()
        .expect("run notionli");
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["ok"], true);
    assert!(value["errors"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["code"] == "auth_error"));
}

#[test]
fn alias_round_trip_uses_local_sqlite_state() {
    let home = temp_dir();
    let set = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            &home,
            "alias",
            "set",
            "roadmap",
            "page:16d8004e5f6a42a6981151c22ddada12",
        ])
        .output()
        .expect("set alias");
    assert!(
        set.status.success(),
        "{}",
        String::from_utf8_lossy(&set.stderr)
    );

    let resolve = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", &home, "resolve", "roadmap"])
        .output()
        .expect("resolve alias");
    assert!(
        resolve.status.success(),
        "{}",
        String::from_utf8_lossy(&resolve.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&resolve.stdout).unwrap();
    assert_eq!(
        value["result"]["id"],
        "16d8004e-5f6a-42a6-9811-51c22ddada12"
    );
}

#[test]
fn page_patch_is_dry_run_by_default() {
    let home = temp_dir();
    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            &home,
            "page",
            "patch",
            "page:16d8004e5f6a42a6981151c22ddada12",
            "--section",
            "Action Items",
            "--append-text",
            "Follow up.",
        ])
        .output()
        .expect("patch dry run");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["dry_run"], true);
    assert_eq!(value["changed"], false);
}

#[test]
fn profile_create_use_and_show_round_trip() {
    let home = temp_dir();
    for args in [
        vec!["--home", &home, "profile", "create", "work"],
        vec!["--home", &home, "profile", "use", "work"],
        vec!["--home", &home, "profile", "show", "work"],
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_notionli"))
            .args(args)
            .output()
            .expect("profile command");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let show_named = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", &home, "profile", "show", "work"])
        .output()
        .expect("profile show named");
    assert!(show_named.status.success());
    let value: serde_json::Value = serde_json::from_slice(&show_named.stdout).unwrap();
    assert_eq!(value["profile"], "work");
}

#[test]
fn select_and_selected_support_dot_resolution() {
    let home = temp_dir();
    let set = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            &home,
            "select",
            "page:16d8004e5f6a42a6981151c22ddada12",
        ])
        .output()
        .expect("select");
    assert!(set.status.success());

    let selected = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", &home, "selected"])
        .output()
        .expect("selected");
    assert!(selected.status.success());
    let value: serde_json::Value = serde_json::from_slice(&selected.stdout).unwrap();
    assert_eq!(
        value["selected"]["id"],
        "16d8004e-5f6a-42a6-9811-51c22ddada12"
    );

    let resolve_dot = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", &home, "resolve", "."])
        .output()
        .expect("resolve dot");
    assert!(resolve_dot.status.success());
}

fn temp_dir() -> &'static str {
    let dir = tempfile::tempdir().unwrap();
    Box::leak(dir.keep().to_string_lossy().to_string().into_boxed_str())
}
