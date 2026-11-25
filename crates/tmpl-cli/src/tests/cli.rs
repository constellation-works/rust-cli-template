use crate::cli::Cli;
use clap::CommandFactory;

/// Every subcommand path in the unbuilt tree, including the root.
fn all_commands(
    cmd: &clap::Command,
    path: &mut Vec<String>,
    out: &mut Vec<(String, clap::Command)>,
) {
    out.push((path.join(" "), cmd.clone()));
    for sub in cmd.get_subcommands() {
        path.push(sub.get_name().to_owned());
        all_commands(sub, path, out);
        path.pop();
    }
}

#[test]
fn no_subcommand_argument_shares_an_id_with_a_global_one() {
    // STD-01 R26: clap merges arguments that share an id, so a subcommand
    // field named like a global flag would silently feed the global. Walk the
    // tree before `build()` propagates the globals into every subcommand.
    let root = Cli::command();
    let globals: Vec<String> = root
        .get_arguments()
        .filter(|arg| arg.is_global_set())
        .map(|arg| arg.get_id().to_string())
        .collect();
    assert!(globals.len() >= 3, "the walk must see the global flags");
    let mut unbuilt = Vec::new();
    all_commands(&root, &mut vec![String::from("tmpl-cli")], &mut unbuilt);
    for (path, cmd) in unbuilt.into_iter().skip(1) {
        for arg in cmd.get_arguments() {
            let id = arg.get_id().as_str();
            assert!(
                !globals.iter().any(|global| global == id),
                "`{path}` argument `{id}` shares its id with a global argument"
            );
        }
    }
}
