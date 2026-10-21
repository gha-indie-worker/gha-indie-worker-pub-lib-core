use std::{fs, path::Path, process};

const FAIL_PREFIX: &str = "project identity contract failed";

fn require(errors: &mut Vec<String>, ok: bool, description: &str) {
    if !ok {
        errors.push(format!("{FAIL_PREFIX}: {description}"));
    }
}

fn validate(
    readme: &str,
    bootstrap: &str,
    sources: &str,
    source_exists: bool,
    bootstrap_exists: bool,
    root_cargo_exists: bool,
    root_rust_main_exists: bool,
) -> Vec<String> {
    let mut errors = Vec::new();

    require(
        &mut errors,
        readme.starts_with("# Oreslang Stack CLI\n"),
        "correct project heading",
    );
    require(
        &mut errors,
        readme.contains("## Project identity: Oreslang Stack is not ORES Stack"),
        "explicit project distinction",
    );
    require(
        &mut errors,
        readme.contains("https://github.com/ores-truffle-oreslang/oreslang-stack-cli"),
        "canonical Oreslang CLI URL",
    );
    require(
        &mut errors,
        readme.contains("https://github.com/ores-stack/ores-stack-cli"),
        "cross-reference to distinct Rust CLI",
    );
    require(
        &mut errors,
        readme.contains("`oreslang-stack`") && readme.contains("`ores-stack`"),
        "separate CLI commands",
    );
    require(
        &mut errors,
        readme.contains("**private**")
            && readme.contains("Git tree SHAs")
            && readme.contains("executed"),
        "private exact-tree CI mirror constraints",
    );
    require(
        &mut errors,
        readme.contains("PR #496 is merged"),
        "record merged source dependency; do not call it unmerged",
    );
    require(
        &mut errors,
        readme.contains("installed Java/Graal runtime"),
        "separate merged source from installed runtime compatibility",
    );
    require(
        &mut errors,
        readme.contains("deploy` currently fails closed"),
        "deployment must not be represented as certified",
    );
    require(
        &mut errors,
        !readme.contains("This PR must remain") && !readme.contains("Until this dependency is merged"),
        "reject stale pull request claims",
    );
    require(
        &mut errors,
        !readme.contains("|\\n|"),
        "reject escaped newline breaking Markdown table",
    );

    let table_rows: Vec<&str> = readme.lines().filter(|line| line.starts_with("| `")).collect();
    require(
        &mut errors,
        table_rows.len() >= 7 && table_rows.iter().all(|row| row.trim_end().ends_with('|')),
        "command-table rows must be well-formed",
    );
    require(
        &mut errors,
        source_exists && sources.contains("oreslang-stack-cli"),
        "the Oreslang source implementation exists",
    );
    require(
        &mut errors,
        bootstrap_exists && bootstrap.contains("ORESLANG_BIN"),
        "the POSIX runtime bootstrap exists",
    );
    require(
        &mut errors,
        !root_cargo_exists && !root_rust_main_exists,
        "Rust CLI must not be misidentified as this repository",
    );

    errors
}

fn main() {
    let root = Path::new(".");
    let readme = fs::read_to_string(root.join("README.md")).unwrap_or_else(|error| {
        eprintln!("{FAIL_PREFIX}: cannot read README.md: {error}");
        process::exit(1);
    });
    let bootstrap_path = root.join("bin/oreslang-stack");
    let source_path = root.join("src/main.ores");
    let bootstrap = fs::read_to_string(&bootstrap_path).unwrap_or_default();
    let sources = fs::read_to_string(&source_path).unwrap_or_default();

    let errors = validate(
        &readme,
        &bootstrap,
        &sources,
        source_path.is_file(),
        bootstrap_path.is_file(),
        root.join("Cargo.toml").exists(),
        root.join("src/main.rs").exists(),
    );

    if !errors.is_empty() {
        for error in errors {
            eprintln!("{error}");
        }
        process::exit(1);
    }

    println!("Oreslang Stack identity documentation contract passed (static only)");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_readme() -> String {
        let mut text = String::from(
            "# Oreslang Stack CLI\n\n\
             ## Project identity: Oreslang Stack is not ORES Stack\n\
             https://github.com/ores-truffle-oreslang/oreslang-stack-cli\n\
             https://github.com/ores-stack/ores-stack-cli\n\
             `oreslang-stack` and `ores-stack`\n\
             **private** Git tree SHAs executed\n\
             PR #496 is merged\n\
             installed Java/Graal runtime\n\
             deploy` currently fails closed\n",
        );
        for index in 0..7 {
            text.push_str(&format!("| `cmd{index}` | ok |\n"));
        }
        text
    }

    #[test]
    fn valid_contract_passes() {
        assert!(validate(
            &valid_readme(),
            "ORESLANG_BIN=x",
            "oreslang-stack-cli",
            true,
            true,
            false,
            false,
        )
        .is_empty());
    }

    #[test]
    fn stale_pr_claim_is_rejected() {
        let readme = format!("{}This PR must remain open\n", valid_readme());
        let errors = validate(
            &readme,
            "ORESLANG_BIN=x",
            "oreslang-stack-cli",
            true,
            true,
            false,
            false,
        );
        assert!(errors.iter().any(|error| error.contains("stale pull request")));
    }

    #[test]
    fn root_rust_cli_is_rejected_but_tooling_is_not_part_of_this_check() {
        let errors = validate(
            &valid_readme(),
            "ORESLANG_BIN=x",
            "oreslang-stack-cli",
            true,
            true,
            true,
            false,
        );
        assert!(errors.iter().any(|error| error.contains("misidentified")));
    }

    #[test]
    fn malformed_command_table_is_rejected() {
        let readme = valid_readme().replace("| `cmd0` | ok |", "| `cmd0` | ok");
        let errors = validate(
            &readme,
            "ORESLANG_BIN=x",
            "oreslang-stack-cli",
            true,
            true,
            false,
            false,
        );
        assert!(errors.iter().any(|error| error.contains("command-table")));
    }
}
