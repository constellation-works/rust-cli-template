use crate::app::{AppEnv, resolve_root};
use crate::error::CliError;
use std::path::{Path, PathBuf};

fn env(root: Option<&str>, home: Option<&str>) -> AppEnv {
    AppEnv {
        root: root.map(Into::into),
        home: home.map(Into::into),
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
