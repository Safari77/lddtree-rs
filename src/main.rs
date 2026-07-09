use std::collections::{HashMap, HashSet};
use std::env;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process;

use lddtree::{DependencyAnalyzer, Library};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    if let Some(pathname) = args.next() {
        let root = args
            .next()
            .map(|s| PathBuf::from(&s))
            .unwrap_or_else(|| PathBuf::from("/"));
        let lib_paths = args.map(|s| PathBuf::from(&s)).collect();
        let analyzer = DependencyAnalyzer::new(root).library_paths(lib_paths);
        let deps = analyzer.analyze(pathname)?;
        if let Some(interp) = deps.interpreter {
            // The interpreter is keyed by its soname (basename) in `libraries`
            let interp_name = Path::new(&interp)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&interp);
            if let Some(path) = deps
                .libraries
                .get(interp_name)
                .and_then(|lib| lib.realpath.as_ref())
            {
                println!("{} => {}", interp, path.display());
            } else {
                println!("{} => not found", interp);
            }
        }
        // Create a single shared history set for the entire tree
        let mut history = HashSet::new();

        // Claim all root dependencies BEFORE traversal
        let mut root_deps = Vec::new();
        for needed in &deps.needed {
            let path_to_claim = deps
                .libraries
                .get(needed)
                .and_then(|lib| lib.realpath.clone())
                .unwrap_or_else(|| PathBuf::from(needed));

            if history.insert(path_to_claim) {
                root_deps.push(needed.clone());
            }
        }

        for needed in root_deps {
            print_library(&needed, &deps.libraries, 0, &mut history);
        }
    } else {
        eprintln!("USAGE: lddtree <pathname> [root] [library path...]");
        process::exit(1);
    }
    Ok(())
}

fn print_library(
    name: &str,
    libraries: &HashMap<String, Library>,
    level: usize,
    history: &mut HashSet<PathBuf>,
) {
    let padding = " ".repeat(level);
    if let Some(lib) = libraries.get(name) {
        if let Some(path) = lib.realpath.as_ref() {
            println!("{}{} => {}", padding, name, path.display());
        } else {
            println!("{}{} => not found", padding, name);
        }

        // Claim children before recursing.
        // This prevents duplicate prints and keeps shared foundational
        // libraries as high up in the dependency tree as possible.
        let mut children_to_visit = Vec::new();

        for needed in &lib.needed {
            let path_to_claim = libraries
                .get(needed)
                .and_then(|dep_lib| dep_lib.realpath.clone())
                .unwrap_or_else(|| PathBuf::from(needed));

            // If we haven't seen this dependency anywhere higher up in the tree,
            // claim it for this level and add it to the traversal queue.
            if history.insert(path_to_claim) {
                children_to_visit.push(needed.clone());
            }
        }

        for needed in children_to_visit {
            print_library(&needed, libraries, level + 4, history);
        }
    }
}
