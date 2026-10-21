use regex::Regex;
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process;
use walkdir::WalkDir;

struct Patterns {
    import: Regex,
    amp_mut: Regex,
    amp_unary: Regex,
    c_pointer_nominal: Regex,
    c_pointer_primitive: Regex,
    deref: Regex,
    callable_channel: Regex,
}

impl Patterns {
    fn new() -> Result<Self, String> {
        Ok(Self {
            import: Regex::new(
                r#"^\s*import\s+(class|fnc|interface|trait|struct|types)\s+(.+?)\s+from\s+['"]([^'"]+)['"]\s*;\s*$"#,
            )
            .map_err(|error| error.to_string())?,
            amp_mut: Regex::new(r"&\s*mut\b").map_err(|error| error.to_string())?,
            amp_unary: Regex::new(
                r"(?:^|[=(,:;<\[]|\breturn\s+)\s*&\s*[A-Za-z_][A-Za-z0-9_]*\b",
            )
            .map_err(|error| error.to_string())?,
            c_pointer_nominal: Regex::new(
                r"\b[A-Z][A-Za-z0-9_<>,]*\*\s*[A-Za-z_][A-Za-z0-9_]*\b",
            )
            .map_err(|error| error.to_string())?,
            c_pointer_primitive: Regex::new(
                r"\b(?:int|float|bool|String|string)\s*\*\s*[A-Za-z_][A-Za-z0-9_]*\b",
            )
            .map_err(|error| error.to_string())?,
            deref: Regex::new(r"(?:^|[=(,:;])\s*\*[A-Za-z_][A-Za-z0-9_]*\b")
                .map_err(|error| error.to_string())?,
            callable_channel: Regex::new(r"Channel\s*<[^>]*(?:Fnc|routine|->)")
                .map_err(|error| error.to_string())?,
        })
    }
}

fn code_only(line: &str) -> String {
    let mut output = String::with_capacity(line.len());
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            output.push(' ');
            continue;
        }

        if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            output.push(' ');
            continue;
        }

        if ch == '/' && chars.peek() == Some(&'/') {
            break;
        }

        output.push(ch);
    }

    output
}

fn imported_names(raw: &str) -> Vec<String> {
    let mut value = raw.trim();
    if value == "*" {
        return Vec::new();
    }
    if value.starts_with('(') && value.ends_with(')') {
        value = &value[1..value.len() - 1];
    }

    value
        .split(',')
        .filter_map(|part| {
            let mut token = part.trim();
            if token.is_empty() {
                return None;
            }
            if let Some((before, _)) = token.split_once(" as ") {
                token = before.trim();
            }
            (!token.is_empty()).then(|| token.to_owned())
        })
        .collect()
}

fn declaration_pattern(kind: &str, name: &str) -> Result<Option<Regex>, String> {
    let name = regex::escape(name);
    let expression = match kind {
        "class" => format!(r"\b(?:pub\s+define\s+class|define\s+(?:pub\s+)?class)\s+{name}\b"),
        "fnc" => format!(r"\b(?:pub\s+)?(?:async\s+|nlex\s+|pure\s+)*fnc\s+{name}\b"),
        "interface" => {
            format!(r"\b(?:pub\s+define\s+interface|define\s+(?:pub\s+)?interface)\s+{name}\b")
        }
        "trait" => format!(r"\b(?:pub\s+define\s+trait|define\s+(?:pub\s+)?trait)\s+{name}\b"),
        "struct" => format!(r"\b(?:pub\s+define\s+struct|define\s+(?:pub\s+)?struct)\s+{name}\b"),
        "types" => {
            format!(r"\b(?:type|define\s+(?:pub\s+)?(?:class|interface|trait|struct))\s+{name}\b")
        }
        _ => return Ok(None),
    };
    Regex::new(&expression)
        .map(Some)
        .map_err(|error| error.to_string())
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            other => result.push(other.as_os_str()),
        }
    }
    result
}

fn source_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for source_root in ["src", "tests", "examples"] {
        let base = root.join(source_root);
        if !base.exists() {
            continue;
        }
        files.extend(
            WalkDir::new(base)
                .follow_links(false)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().is_file())
                .map(|entry| entry.into_path())
                .filter(|path| {
                    path.extension()
                        .is_some_and(|extension| extension == "ores")
                }),
        );
    }
    files.sort();
    files
}

fn push_error(errors: &mut Vec<String>, root: &Path, path: &Path, line_no: usize, message: &str) {
    let relative = path.strip_prefix(root).unwrap_or(path);
    errors.push(format!("{}:{line_no}: {message}", relative.display()));
}

fn validate_root(root: &Path) -> Result<(usize, Vec<String>), String> {
    let root = fs::canonicalize(root).map_err(|error| format!("cannot resolve root: {error}"))?;
    let patterns = Patterns::new()?;
    let files = source_files(&root);
    let mut errors = Vec::new();

    for path in &files {
        let text = fs::read_to_string(path)
            .map_err(|error| format!("{}: cannot read source: {error}", path.display()))?;

        for (index, raw) in text.lines().enumerate() {
            let line_no = index + 1;
            let code = code_only(raw);

            if patterns.amp_mut.is_match(&code) {
                push_error(
                    &mut errors,
                    &root,
                    path,
                    line_no,
                    "Rust-style &mut ownership syntax is forbidden",
                );
            }
            if patterns.amp_unary.is_match(&code) {
                push_error(
                    &mut errors,
                    &root,
                    path,
                    line_no,
                    "pointer-style unary/type ampersand ownership syntax is forbidden; use rt borrow/take/copy/share",
                );
            }
            if patterns.c_pointer_nominal.is_match(&code)
                || patterns.c_pointer_primitive.is_match(&code)
            {
                push_error(
                    &mut errors,
                    &root,
                    path,
                    line_no,
                    "C-style pointer declaration syntax is forbidden",
                );
            }
            if patterns.deref.is_match(&code) {
                push_error(
                    &mut errors,
                    &root,
                    path,
                    line_no,
                    "unary pointer dereference syntax is forbidden",
                );
            }
            if patterns.callable_channel.is_match(&code) {
                push_error(
                    &mut errors,
                    &root,
                    path,
                    line_no,
                    "callable/executable channel payload is forbidden",
                );
            }

            let Some(capture) = patterns.import.captures(raw) else {
                continue;
            };
            let kind = capture.get(1).unwrap().as_str();
            let raw_names = capture.get(2).unwrap().as_str();
            let target = capture.get(3).unwrap().as_str();
            if !target.starts_with('.') {
                continue;
            }

            let parent = path.parent().ok_or("source file has no parent")?;
            let mut target_path = lexical_normalize(&parent.join(target));
            if target_path
                .extension()
                .is_none_or(|extension| extension != "ores")
            {
                target_path.set_extension("ores");
            }

            if !target_path.starts_with(&root) {
                push_error(
                    &mut errors,
                    &root,
                    path,
                    line_no,
                    &format!("relative import escapes repository: {target}"),
                );
                continue;
            }

            if !target_path.is_file() {
                push_error(
                    &mut errors,
                    &root,
                    path,
                    line_no,
                    &format!("relative import target does not exist: {target}"),
                );
                continue;
            }

            let resolved_target = fs::canonicalize(&target_path).map_err(|error| {
                format!("{}: cannot resolve import: {error}", target_path.display())
            })?;
            if !resolved_target.starts_with(&root) {
                push_error(
                    &mut errors,
                    &root,
                    path,
                    line_no,
                    &format!("relative import escapes repository: {target}"),
                );
                continue;
            }

            let target_text = fs::read_to_string(&resolved_target).map_err(|error| {
                format!(
                    "{}: cannot read import target: {error}",
                    resolved_target.display()
                )
            })?;
            for name in imported_names(raw_names) {
                if let Some(pattern) = declaration_pattern(kind, &name)? {
                    if !pattern.is_match(&target_text) {
                        let relative_target = resolved_target
                            .strip_prefix(&root)
                            .unwrap_or(&resolved_target)
                            .display();
                        push_error(
                            &mut errors,
                            &root,
                            path,
                            line_no,
                            &format!(
                                "imported {kind} {name:?} is not declared in {relative_target}"
                            ),
                        );
                    }
                }
            }
        }
    }

    Ok((files.len(), errors))
}

fn root_from_args() -> Result<PathBuf, String> {
    let default_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut args = env::args_os().skip(1);
    match (args.next(), args.next(), args.next()) {
        (None, None, None) => Ok(default_root),
        (Some(flag), Some(root), None) if flag == "--root" => Ok(PathBuf::from(root)),
        _ => Err("usage: oreslang-static-audit [--root PATH]".to_owned()),
    }
}

fn main() {
    let root = root_from_args().unwrap_or_else(|error| {
        eprintln!("{error}");
        process::exit(2);
    });

    match validate_root(&root) {
        Ok((count, errors)) if errors.is_empty() => {
            println!("static audit passed: {count} Oreslang source/test/example files");
        }
        Ok((_, errors)) => {
            eprintln!("static audit failed:");
            for error in errors {
                eprintln!("  {error}");
            }
            process::exit(1);
        }
        Err(error) => {
            eprintln!("static audit failed: {error}");
            process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn fixture(source: &str, target: &str) -> TempDir {
        let temp = tempfile::tempdir().unwrap();
        write(&temp.path().join("src/main.ores"), source);
        write(&temp.path().join("src/dep.ores"), target);
        temp
    }

    #[test]
    fn strings_and_comments_do_not_trigger_pointer_rules() {
        let temp = fixture(
            "fnc main() {\n  let String a = \"&mut x *ptr\"; // &mut ignored\n}\n",
            "define pub class Foo as\nend\n",
        );
        let (_, errors) = validate_root(temp.path()).unwrap();
        assert!(errors.is_empty(), "{errors:?}");
    }

    #[test]
    fn ownership_pointer_and_callable_channel_forms_are_rejected() {
        let cases = [
            "fnc main() {\n  let x = &mut value;\n}\n",
            "fnc main() {\n  return &value;\n}\n",
            "fnc main() {\n  int *ptr;\n}\n",
            "fnc main() {\n  let x = *ptr;\n}\n",
            "fnc main(Channel<Fnc(int) -> int> jobs) {}\n",
        ];
        for source in cases {
            let temp = fixture(source, "define pub class Foo as\nend\n");
            let (_, errors) = validate_root(temp.path()).unwrap();
            assert!(!errors.is_empty(), "unexpectedly accepted: {source}");
        }
    }

    #[test]
    fn relative_import_requires_declared_symbol() {
        let temp = fixture(
            "import class Foo from './dep';\n",
            "define pub class Foo as\nend\n",
        );
        let (_, errors) = validate_root(temp.path()).unwrap();
        assert!(errors.is_empty(), "{errors:?}");

        write(
            &temp.path().join("src/dep.ores"),
            "define pub class Bar as\nend\n",
        );
        let (_, errors) = validate_root(temp.path()).unwrap();
        assert!(errors.iter().any(|error| error.contains("is not declared")));
    }

    #[test]
    fn relative_import_cannot_escape_repository() {
        let temp = tempfile::tempdir().unwrap();
        write(
            &temp.path().join("src/main.ores"),
            "import class Foo from '../../outside';\n",
        );
        let (_, errors) = validate_root(temp.path()).unwrap();
        assert!(errors
            .iter()
            .any(|error| error.contains("escapes repository")));
    }

    #[test]
    fn imported_name_aliases_and_wildcards_match_python_contract() {
        assert_eq!(imported_names("*"), Vec::<String>::new());
        assert_eq!(
            imported_names("(Foo as LocalFoo, Bar)"),
            vec!["Foo".to_owned(), "Bar".to_owned()]
        );
    }
}
