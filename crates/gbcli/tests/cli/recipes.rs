//! `gbcli recipes`' dev loop, run line by line as written: every command in
//! it must work today, in order, from an empty directory.

use crate::common::Project;

/// The loop's lines: from its heading to the next blank line.
fn dev_loop(recipes: &str) -> Vec<String> {
    recipes
        .lines()
        .skip_while(|l| !l.starts_with("The dev loop"))
        .skip(1)
        .take_while(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string())
        .collect()
}

/// Splits a command as a shell would for these lines: words, and '…'
/// quoted words.
fn words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    for c in line.chars() {
        match c {
            '\'' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !word.is_empty() {
                    out.push(std::mem::take(&mut word));
                }
            }
            c => word.push(c),
        }
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

#[test]
fn the_dev_loop_recipe_runs_as_written() {
    let outside = Project::new("recipe-loop");
    let recipes = outside.gbcli(&["recipes"]).stdout;
    let lines = dev_loop(&recipes);
    assert!(lines.len() >= 8, "no dev loop in:\n{recipes}");
    let mut inside: Option<Project> = None;
    let mut ran = 0;
    for line in &lines {
        if let Some(dir) = line.strip_prefix("cd ") {
            inside = Some(Project {
                dir: outside.dir.join(dir),
            });
            continue;
        }
        let Some(command) = line.strip_prefix("gbcli ") else {
            assert!(
                line.starts_with('('),
                "neither a command nor a note: {line}"
            );
            continue;
        };
        let args = words(command);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let run = inside.as_ref().unwrap_or(&outside).rgbds(&args);
        assert_eq!(run.code, 0, "{line}\n{}{}", run.stdout, run.stderr);
        ran += 1;
    }
    assert!(ran >= 9, "ran {ran} commands");
    let inside = inside.expect("the loop works inside the new project");
    assert!(inside.path("first.png").is_file(), "the shot");
}
