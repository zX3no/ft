use mini::{defer_results, profile};
use std::{
    collections::BTreeMap,
    fs::create_dir_all,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    thread,
    time::Instant,
};

//TODO: Maybe a progress bar on another thread?
// fn progress() {
//     use std::io::Write;
//     print!("\x1b[2K\x1b[G");
//     std::io::stdout().flush().unwrap();
// }

fn generate_tree(path: &str) -> BTreeMap<PathBuf, u64> {
    // Ideally this would be &'a str or &'a OsStr, I could do this with winwalk not sure about MacOS.
    let mut btree: BTreeMap<PathBuf, u64> = BTreeMap::new();

    for entry in walkdir::WalkDir::new(path).sort_by_file_name() {
        profile!("file entry");

        if let Ok(entry) = entry {
            let Ok(metadata) = entry.metadata() else {
                continue;
            };

            if metadata.is_dir() {
                continue;
            }

            let flsz = metadata.size();
            //Normalize the paths and remove the parent directory folder.
            let path = entry.path().as_os_str().to_string_lossy().replace(path, "");

            btree.insert(PathBuf::from(path), flsz);
        }
    }

    btree
}

fn copy(file: &str, source_path: &str, destination_path: &str) {
    let from = format!("{}{}", source_path, file);
    let to = format!("{}{}", destination_path, file);
    println!("Copying {} to {}", from, to);

    let to = Path::new(&to);
    if let Some(parent) = to.parent() {
        if !parent.exists() {
            create_dir_all(parent).unwrap();
        }
    }

    std::fs::copy(&from, to).unwrap();
}

fn normalize(mut path: String, home: &str) -> String {
    path = path.replace("~/", home);

    if !path.ends_with('/') {
        path.push('/');
    }

    if !path.starts_with('/') {
        path.insert(0, '/');
    }

    path
}

fn main() {
    defer_results!();
    profile!();

    let now = Instant::now();
    let args: Vec<String> = std::env::args().skip(1).collect();

    //ft <source> <destination> --cleanup
    if args.len() < 2 {
        eprintln!("usage: ft <source> <destion>");
        return;
    }

    let home = home::home_dir().unwrap();
    let home = home.to_string_lossy();

    let source_path = normalize(args[0].clone(), &home);
    let destination_path = normalize(args[1].clone(), &home);

    if !Path::new(&source_path).exists() {
        return eprintln!("error: {} does not exist.", source_path);
    }

    if !Path::new(&destination_path).exists() {
        return eprintln!("error: {} does not exist.", destination_path);
    }

    let sp = source_path.clone();
    let dp = destination_path.clone();

    let source = thread::spawn(move || generate_tree(&sp));
    let destination = thread::spawn(move || generate_tree(&dp));

    let destination = destination.join().unwrap();
    let source = source.join().unwrap();

    if source.is_empty() {
        return eprintln!("{} is empty, no files to copy.", source_path);
    }

    for (key, hash) in &source {
        if let Some(dest_hash) = destination.get(key) {
            if hash != dest_hash {
                println!(
                    "'{}' expected hash: {} but found: {}",
                    key.file_name().unwrap_or_default().to_string_lossy(),
                    hash,
                    dest_hash
                );
                let file = key.as_os_str().to_string_lossy();
                copy(&file, &source_path, &destination_path);
            }
        } else {
            let file = key.as_os_str().to_string_lossy();
            copy(&file, &source_path, &destination_path);
        }
    }

    //Check for redundant files.
    for (key, _) in destination {
        if !source.contains_key(&key) {
            if let Some(file_name) = key.file_name() {
                if file_name == ".DS_Store" {
                    continue;
                }
            }

            println!("Key {:#?} should not exist", key);
            //TODO: Move to trash.
        }
    }

    println!(
        "Finished cloning {} in {} seconds",
        &source_path,
        now.elapsed().as_secs()
    );
}
