//! Behavioural tests for the `cargo redox-mc` development CLI.
//!
//! These tests are the specification for the CLI contract. They cover the
//! pieces that do not require compiling a whole mod (that path is exercised
//! end-to-end by `tests/test_cargo_redox_mc.py`), so they run in milliseconds.

use cargo_redox_mc::cli::{normalize_args, Cli, Command, SideArg};
use cargo_redox_mc::commands::run::{build_launch_command, LaunchCommand, LaunchTarget};
use cargo_redox_mc::project::ModProject;
use cargo_redox_mc::repo::{find_artifact, RedoxRepo};
use cargo_redox_mc::templates::{derive_mod_id, scaffold, ScaffoldOptions};
use clap::Parser;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

/// Creates a unique scratch directory without pulling in `tempfile`.
fn unique_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "redox_cli_{tag}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

#[test]
fn normalizes_cargo_subcommand_invocation() {
    // Cargo invokes `cargo-redox-mc redox-mc <args>` for `cargo redox-mc <args>`.
    let args = vec![
        "cargo-redox-mc".to_string(),
        "redox-mc".to_string(),
        "run".to_string(),
        "--server".to_string(),
    ];
    assert_eq!(
        normalize_args(args),
        vec!["cargo-redox-mc", "run", "--server"]
    );
}

#[test]
fn keeps_direct_invocation_unchanged() {
    let args = vec![
        "cargo-redox-mc".to_string(),
        "new".to_string(),
        "demo_mod".to_string(),
    ];
    assert_eq!(
        normalize_args(args.clone()),
        vec!["cargo-redox-mc", "new", "demo_mod"]
    );
}

#[test]
fn does_not_strip_redox_mc_when_it_is_a_value() {
    let args = vec![
        "cargo-redox-mc".to_string(),
        "redox-mc".to_string(),
        "new".to_string(),
        "redox-mc".to_string(),
    ];
    assert_eq!(
        normalize_args(args),
        vec!["cargo-redox-mc", "new", "redox-mc"]
    );
}

#[test]
fn parses_server_run_with_cargo_prefix() {
    let args = normalize_args(vec![
        "cargo-redox-mc".to_string(),
        "redox-mc".to_string(),
        "run".to_string(),
        "--server".to_string(),
    ]);
    let cli = Cli::try_parse_from(args).expect("run --server must parse");
    match cli.command {
        Command::Run(run) => {
            assert!(run.server);
            assert!(!run.client);
            assert_eq!(run.port, None);
        }
        other => panic!("expected run subcommand, got {other:?}"),
    }
}

#[test]
fn parses_explicit_server_port() {
    let cli = Cli::try_parse_from(["cargo-redox-mc", "run", "--server", "--port", "25570"])
        .expect("explicit port must parse");
    match cli.command {
        Command::Run(run) => assert_eq!(run.port, Some(25570)),
        other => panic!("expected run subcommand, got {other:?}"),
    }
}

#[test]
fn rejects_run_with_both_server_and_client() {
    let err = Cli::try_parse_from(["cargo-redox-mc", "run", "--server", "--client"])
        .expect_err("--server and --client must conflict");
    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn derives_valid_mod_id_from_name() {
    assert_eq!(derive_mod_id("My-Mod").unwrap(), "my_mod");
    assert_eq!(derive_mod_id("demo_mod").unwrap(), "demo_mod");
    assert_eq!(derive_mod_id("Cool Mod 2").unwrap(), "cool_mod_2");
}

#[test]
fn rejects_mod_id_with_accents() {
    let err = derive_mod_id("café").expect_err("non-ascii names are rejected");
    assert!(
        err.to_string().contains("caf"),
        "error should name the offending input: {err}"
    );
}

#[test]
fn scaffold_writes_a_buildable_mod_layout() {
    let dir = unique_dir("scaffold");
    let options = ScaffoldOptions {
        id: "demo_mod".to_string(),
        display_name: "Demo Mod".to_string(),
        side: "both",
        author: "Tester".to_string(),
        api_path: PathBuf::from("/opt/redox-mc/redox-mc-api"),
    };

    scaffold(&dir, &options).expect("scaffold must succeed");

    let cargo_toml = fs::read_to_string(dir.join("Cargo.toml")).expect("Cargo.toml exists");
    assert!(cargo_toml.contains("crate-type = [\"cdylib\"]"));
    assert!(cargo_toml.contains("name = \"demo_mod\""));

    let mod_toml = fs::read_to_string(dir.join("mod.toml")).expect("mod.toml exists");
    assert!(mod_toml.contains("id = \"demo_mod\""));
    assert!(mod_toml.contains("name = \"Demo Mod\""));
    assert!(mod_toml.contains("side = \"both\""));
    assert!(mod_toml.contains("version = \"0.1.0\""));

    let lib_rs = fs::read_to_string(dir.join("src/lib.rs")).expect("src/lib.rs exists");
    assert!(lib_rs.contains("redox_mod_init"));
    assert!(lib_rs.contains("redox_mod_on_server_chat"));

    assert!(
        dir.join(".gitignore").exists(),
        ".gitignore must be created"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn scaffold_refuses_to_overwrite_an_existing_project() {
    let dir = unique_dir("scaffold_twice");
    let options = ScaffoldOptions {
        id: "demo_mod".to_string(),
        display_name: "Demo Mod".to_string(),
        side: "both",
        author: "Tester".to_string(),
        api_path: PathBuf::from("/opt/redox-mc/redox-mc-api"),
    };
    scaffold(&dir, &options).expect("first scaffold must succeed");
    let err = scaffold(&dir, &options).expect_err("second scaffold must be refused");
    assert!(err.to_string().contains("already exists"), "got: {err}");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn finds_project_root_from_a_nested_directory() {
    let root = unique_dir("project");
    let nested = root.join("src/commands");
    fs::create_dir_all(&nested).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"demo-mod\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\nname = \"demo_mod\"\ncrate-type = [\"cdylib\"]\n",
    )
    .unwrap();
    fs::write(
        root.join("mod.toml"),
        "id = \"demo_mod\"\nname = \"Demo Mod\"\nversion = \"0.1.0\"\nside = \"both\"\n",
    )
    .unwrap();

    let project = ModProject::find(&nested).expect("project must be discovered upward");
    assert_eq!(project.root, root);
    assert_eq!(project.mod_id, "demo_mod");
    assert_eq!(project.lib_name, "demo_mod");
    assert_eq!(
        project.library_file_name(),
        format!(
            "{}demo_mod{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX
        )
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn find_artifact_prefers_debug_over_target_triples() {
    let target = unique_dir("target");
    let triple = target.join("x86_64-unknown-linux-gnu/debug");
    fs::create_dir_all(&triple).unwrap();
    fs::create_dir_all(target.join("release")).unwrap();

    let triple_artifact = triple.join("libdemo.so");
    fs::write(&triple_artifact, b"triple").unwrap();

    assert_eq!(
        find_artifact(&target, "libdemo.so"),
        Some(triple_artifact.clone())
    );

    let release_artifact = target.join("release/libdemo.so");
    fs::write(&release_artifact, b"release").unwrap();
    assert_eq!(find_artifact(&target, "libdemo.so"), Some(release_artifact));

    let _ = fs::remove_dir_all(&target);
}

#[test]
fn resolves_agent_library_from_the_repo_target_directory() {
    let root = unique_dir("repo");
    let repo = RedoxRepo::new(root.clone());
    let debug = repo.target_dir().join("debug");
    fs::create_dir_all(&debug).unwrap();
    let agent = debug.join(format!(
        "{}redox_mc_loader{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    ));
    fs::write(&agent, b"fake").unwrap();

    assert_eq!(repo.agent_library().unwrap(), agent);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn builds_server_launch_command_with_injected_agent() {
    let command = build_launch_command(
        OsStr::new("java"),
        Path::new("/opt/redox/libredox_mc_loader.so"),
        Path::new("/opt/mc/server.jar"),
        LaunchTarget::Server,
        Path::new("/work/.redox-mc/server"),
    );

    assert_eq!(command.program, OsString::from("java"));
    assert_eq!(
        command.args,
        vec![
            OsString::from("-agentpath:/opt/redox/libredox_mc_loader.so"),
            OsString::from("-jar"),
            OsString::from("/opt/mc/server.jar"),
            OsString::from("nogui"),
        ]
    );
    assert_eq!(command.cwd, PathBuf::from("/work/.redox-mc/server"));
}

#[test]
fn builds_client_launch_command_without_nogui() {
    let command = build_launch_command(
        OsStr::new("java"),
        Path::new("/opt/redox/libredox_mc_loader.so"),
        Path::new("/opt/mc/client.jar"),
        LaunchTarget::Client,
        Path::new("/work/.redox-mc/client"),
    );

    assert_eq!(
        command.args,
        vec![
            OsString::from("-agentpath:/opt/redox/libredox_mc_loader.so"),
            OsString::from("-jar"),
            OsString::from("/opt/mc/client.jar"),
        ]
    );
}

#[test]
fn launch_command_exposes_a_spawn_helper() {
    let command = LaunchCommand {
        program: OsString::from("definitely-not-a-real-binary"),
        args: Vec::new(),
        cwd: std::env::temp_dir(),
    };
    let err = command
        .spawn()
        .expect_err("spawning a bogus binary must fail");
    assert!(err.to_string().contains("definitely-not-a-real-binary"));
}

#[test]
fn side_argument_maps_to_manifest_strings() {
    assert_eq!(SideArg::Both.as_str(), "both");
    assert_eq!(SideArg::Server.as_str(), "server");
    assert_eq!(SideArg::Client.as_str(), "client");
}
