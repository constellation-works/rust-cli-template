use crate::app::{AppEnv, resolve_actor, resolve_root};
use crate::error::CliError;
use std::path::{Path, PathBuf};

fn env(root: Option<&str>, home: Option<&str>) -> AppEnv {
    AppEnv {
        root: root.map(Into::into),
        home: home.map(Into::into),
        user: None,
        username: None,
    }
}

#[test]
fn the_flag_beats_the_environment_which_beats_home() {
    let all = env(Some("/from-env"), Some("/home/u"));
    assert_eq!(
        resolve_root(Some(Path::new("/from-flag")), &all).unwrap(),
        PathBuf::from("/from-flag")
    );
    assert_eq!(
        resolve_root(None, &all).unwrap(),
        PathBuf::from("/from-env")
    );
    assert_eq!(
        resolve_root(None, &env(None, Some("/home/u"))).unwrap(),
        PathBuf::from("/home/u/.tmpl-cli")
    );
}

#[test]
fn empty_variables_count_as_unset() {
    assert_eq!(
        resolve_root(None, &env(Some(""), Some("/home/u"))).unwrap(),
        PathBuf::from("/home/u/.tmpl-cli")
    );
    assert!(matches!(
        resolve_root(None, &env(Some(""), Some(""))),
        Err(CliError::NoDataDir)
    ));
}

#[test]
fn actor_prefers_user_then_username_then_none() {
    let both = AppEnv {
        user: Some("alice".into()),
        username: Some("bob".into()),
        ..AppEnv::default()
    };
    assert_eq!(resolve_actor(&both).as_deref(), Some("alice"));

    let fallback = AppEnv {
        user: Some("".into()),
        username: Some("bob".into()),
        ..AppEnv::default()
    };
    assert_eq!(resolve_actor(&fallback).as_deref(), Some("bob"));

    let neither = AppEnv {
        user: None,
        username: Some("".into()),
        ..AppEnv::default()
    };
    assert_eq!(resolve_actor(&neither), None);
}
