use crate::composition::{APP, Error, data_dir};
use cw_app::Env;
use cw_app::env::{HOME, ROOT};
use std::path::{Path, PathBuf};

fn env(root: Option<&str>, home: Option<&str>) -> Env {
    let vars = [(ROOT.name_for(APP), root), (HOME.name_for(APP), home)];
    Env::from_vars(
        vars.into_iter()
            .filter_map(|(name, value)| value.map(|value| (name, value))),
    )
}

#[test]
fn the_flag_beats_the_environment_which_beats_home() {
    let all = env(Some("/from-env"), Some("/home/u"));
    assert_eq!(
        data_dir(&all, Some(Path::new("/from-flag"))).unwrap(),
        PathBuf::from("/from-flag")
    );
    assert_eq!(data_dir(&all, None).unwrap(), PathBuf::from("/from-env"));
    assert_eq!(
        data_dir(&env(None, Some("/home/u")), None).unwrap(),
        PathBuf::from("/home/u/.tmpl-cli")
    );
}

#[test]
fn empty_variables_count_as_unset() {
    assert_eq!(
        data_dir(&env(Some(""), Some("/home/u")), None).unwrap(),
        PathBuf::from("/home/u/.tmpl-cli")
    );
    let err = data_dir(&env(Some(""), Some("")), None).unwrap_err();
    assert_eq!(err, Error::NoDataDir);
    assert_eq!(cw_base::Error::from(err).code(), "no_data_dir");
}
