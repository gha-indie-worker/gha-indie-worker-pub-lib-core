use regex::Regex;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use walkdir::WalkDir;

const MUTATORS: [&str; 9] = [
    "initialize",
    "enqueue",
    "add_to_front",
    "insert_before",
    "insert_after",
    "remove",
    "dequeue",
    "remove_last",
    "clear",
];

const READERS: [&str; 8] = [
    "size",
    "is_empty",
    "contains",
    "get",
    "first",
    "last",
    "ordered_keys",
    "validate",
];

fn validate_root(root: &Path) -> Result<Vec<String>, String> {
    let legacy_conditional =
        Regex::new(r";\s*(then|do)(?:\s|$)").map_err(|error| error.to_string())?;
    let compact_conditional =
        Regex::new(r"\b(then|do)\b.*\b(fi|return)\b").map_err(|error| error.to_string())?;
    let java_interop =
        Regex::new(r#"\bimport\s+.*?from\s+["']java:"#).map_err(|error| error.to_string())?;
    let readonly_queue =
        Regex::new(r"\blet\s+IntLinkedQueue\s+\w+\s*=").map_err(|error| error.to_string())?;

    let mut files = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "ores")
        })
        .collect::<Vec<_>>();
    files.sort();

    let mut errors = Vec::new();
    for path in files {
        let relative = path.strip_prefix(root).unwrap_or(&path);
        let text = fs::read_to_string(&path).map_err(|error| {
            format!("{}: cannot read UTF-8 source: {error}", relative.display())
        })?;

        if !text.is_empty() && !text.ends_with('\n') {
            errors.push(format!("{}: missing final newline", relative.display()));
        }

        for (index, line) in text.lines().enumerate() {
            let line_no = index + 1;
            let stripped = line.trim_start();
            if stripped.is_empty() || stripped.starts_with("//") {
                continue;
            }

            let indentation_len = line.len() - stripped.len();
            if line[..indentation_len].contains('\t') {
                errors.push(format!("{}:{line_no}: tab indentation", relative.display()));
            }

            if stripped.starts_with("if ") || stripped.starts_with("elif ") {
                if legacy_conditional.is_match(stripped) {
                    errors.push(format!(
                        "{}:{line_no}: legacy conditional separator",
                        relative.display()
                    ));
                }
                if compact_conditional.is_match(stripped) {
                    errors.push(format!(
                        "{}:{line_no}: compact conditional body",
                        relative.display()
                    ));
                }
            }

            if java_interop.is_match(stripped) {
                errors.push(format!(
                    "{}:{line_no}: forbidden Java interop",
                    relative.display()
                ));
            }
        }

        if relative
            .components()
            .next()
            .is_some_and(|part| part.as_os_str() == "tests")
            && readonly_queue.is_match(&text)
            && relative
                .file_name()
                .is_none_or(|name| name != "readonly_binding.ores")
        {
            errors.push(format!(
                "{}: queue test needs a writable val owner",
                relative.display()
            ));
        }
    }

    let source = fs::read_to_string(root.join("src/int_linked_queue.ores"))
        .map_err(|error| format!("src/int_linked_queue.ores: cannot read source: {error}"))?;
    let declarations_source = source
        .lines()
        .map(|line| line.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    let any_declaration =
        Regex::new(r"(?m)^\s*pub\s+([A-Za-z_]\w*)\s*\(").map_err(|error| error.to_string())?;
    let mutable_declaration =
        Regex::new(r"(?m)^\s*pub\s+([A-Za-z_]\w*)\s*\(\s*mut\s+self\s*\)\s*\(")
            .map_err(|error| error.to_string())?;

    let declarations = any_declaration
        .captures_iter(&declarations_source)
        .map(|capture| capture[1].to_owned())
        .collect::<Vec<_>>();
    let mutable = mutable_declaration
        .captures_iter(&declarations_source)
        .map(|capture| capture[1].to_owned())
        .collect::<Vec<_>>();

    for method in MUTATORS {
        let count = declarations
            .iter()
            .filter(|name| name.as_str() == method)
            .count();
        let mutable_count = mutable
            .iter()
            .filter(|name| name.as_str() == method)
            .count();
        if count != 1 || mutable_count != 1 {
            errors.push(format!(
                "src/int_linked_queue.ores: {method} must appear once with mut self"
            ));
        }
    }

    for method in READERS {
        let count = declarations
            .iter()
            .filter(|name| name.as_str() == method)
            .count();
        let mutable_count = mutable
            .iter()
            .filter(|name| name.as_str() == method)
            .count();
        if count != 1 || mutable_count != 0 {
            errors.push(format!(
                "src/int_linked_queue.ores: {method} must appear once without mut self"
            ));
        }
    }

    for field in ["previous", "following", "values", "occupied"] {
        let expression = Regex::new(&format!(
            r"\blet\s+mut\s+Array<int>\s+{}\s*=",
            regex::escape(field)
        ))
        .map_err(|error| error.to_string())?;
        if !expression.is_match(&declarations_source) {
            errors.push(format!(
                "src/int_linked_queue.ores: {field} lacks mutable referent binding"
            ));
        }
    }

    Ok(errors)
}

fn root_from_args() -> Result<PathBuf, String> {
    let default_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut args = env::args_os().skip(1);
    match (args.next(), args.next(), args.next()) {
        (None, None, None) => Ok(default_root),
        (Some(flag), Some(root), None) if flag == "--root" => Ok(PathBuf::from(root)),
        _ => Err("usage: source-contract-check [--root PATH]".to_owned()),
    }
}

fn main() {
    let root = root_from_args().unwrap_or_else(|error| {
        eprintln!("{error}");
        process::exit(2);
    });
    match validate_root(&root) {
        Ok(errors) if errors.is_empty() => {
            println!("Source contract: static style, receiver and interop checks passed.");
            println!("NOT a compilation, runtime, borrow-checker or concurrency test.");
        }
        Ok(errors) => {
            for error in errors {
                eprintln!("{error}");
            }
            process::exit(1);
        }
        Err(error) => {
            eprintln!("{error}");
            process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    struct Fixture {
        temp: TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();
            fs::create_dir_all(temp.path().join("src")).unwrap();
            let project = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
            fs::copy(
                project.join("src/int_linked_queue.ores"),
                temp.path().join("src/int_linked_queue.ores"),
            )
            .unwrap();
            Self { temp }
        }

        fn root(&self) -> &Path {
            self.temp.path()
        }

        fn edit(&self, before: &str, after: &str) {
            let file = self.root().join("src/int_linked_queue.ores");
            let original = fs::read_to_string(&file).unwrap();
            assert!(original.contains(before), "fixture text missing: {before}");
            fs::write(&file, original.replacen(before, after, 1)).unwrap();
        }
    }

    fn errors(root: &Path) -> Vec<String> {
        validate_root(root).unwrap()
    }

    #[test]
    fn valid_source_shape_passes() {
        let fixture = Fixture::new();
        assert!(errors(fixture.root()).is_empty());
    }

    #[test]
    fn comment_cannot_spoof_mutable_receiver() {
        let fixture = Fixture::new();
        fixture.edit(
            "pub enqueue(mut self)(",
            "// pub enqueue(mut self)(\n  pub enqueue(",
        );
        assert!(errors(fixture.root())
            .iter()
            .any(|error| error.contains("enqueue")));
    }

    #[test]
    fn observer_cannot_be_mutable() {
        let fixture = Fixture::new();
        fixture.edit("pub contains(int key)", "pub contains(mut self)(int key)");
        assert!(errors(fixture.root())
            .iter()
            .any(|error| error.contains("contains")));
    }

    #[test]
    fn duplicate_observer_is_rejected() {
        let fixture = Fixture::new();
        fixture.edit(
            "pub size() -> int",
            "pub size() -> int { return 0; }\n  pub size() -> int",
        );
        assert!(errors(fixture.root())
            .iter()
            .any(|error| error.contains("size")));
    }

    #[test]
    fn mutable_referent_is_required() {
        let fixture = Fixture::new();
        fixture.edit("let mut Array<int> previous", "let Array<int> previous");
        assert!(errors(fixture.root())
            .iter()
            .any(|error| error.contains("previous")));
    }

    #[test]
    fn legacy_conditional_is_rejected() {
        let fixture = Fixture::new();
        fs::create_dir_all(fixture.root().join("tests")).unwrap();
        fs::write(
            fixture.root().join("tests/legacy.ores"),
            "pub fnc main() -> int {\n  if true; do return 1; fi\n}\n",
        )
        .unwrap();
        assert!(errors(fixture.root())
            .iter()
            .any(|error| error.contains("legacy conditional")));
    }
}
