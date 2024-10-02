use blake3::*;
use mini::{defer_results, profile};
use std::{
    collections::BTreeMap, fs::create_dir_all, io::Cursor, os::unix::fs::MetadataExt, path::{Path, PathBuf}, thread
};
use walkdir::WalkDir;

pub fn hash(path: impl AsRef<Path>) -> Result<Hash, std::io::Error> {
    profile!();
    let file = std::fs::read(path)?;
    let cursor = Cursor::new(file);
    let mut hasher = Hasher::new();
    hasher.update(cursor.get_ref());

    let output = hasher.finalize();

    // u64::from_be_bytes(output.as_bytes()[0..8].try_into().unwrap())

    Ok(output)
}

#[inline]
//CRC Collisions can happen, but for a file with the same name...
pub fn hash_crc(path: impl AsRef<Path>) -> Result<u32, std::io::Error> {
    profile!();
    let file = std::fs::read(path)?;
    Ok(crc32fast::hash(file.as_slice()))
}

//The hash of every file on the client and host must be calculated.
//The hashes must be compared, if they are the same no file transfer is needed.
//If they are different they should be copied.
//Ideally hashing would happen on the client and server in parallel.
//For client only uses, the local file hash and network hash should be done on a different thread.
//Ideally we should compare hashes as soon as possible.

//This program should support one way file syncs.
//The target stores the files
//The destination will have target files copied there.

//If we have one thread scanning one folder and another thread scanning another.
//How should we transfer data between the threads.

//Ideally we should sort the paths in the exact same way on both threads.
//So that the same files are hash at the same time and can be copied while
//other files are being hashed.

//How should the files be sorted?
//Should all the paths be collected first then hashed, or should that be done in parallel?

//Neither are capable of being parallelised because they both rely on the file io, which is synchronous.

// BTreeMap<(Path, Hash)>

fn generate_tree(path: &str) -> BTreeMap<PathBuf, u64> {
    let mut total = 0;
    let mut size = 0;
    // Ideally this would be &'a str or &'a OsStr, I could do this with winwalk not sure about MacOS.
    let mut btree: BTreeMap<PathBuf, u64> = BTreeMap::new();
    for entry in WalkDir::new(path).sort_by_file_name() {
        if let Ok(entry) = entry {
            let Ok(metadata) = entry.metadata() else {
                continue;
            };

            if metadata.is_dir() {
                continue;
            }

            let flsz = metadata.size();

            total += 1;
            size += flsz;
            // if let Ok(hash) = hash_crc(entry.path()) {
            //     //Normalize the paths and remove the parent directory folder.
            //     let path = entry.path().as_os_str().to_string_lossy().replace(path, "");
            //     btree.insert(PathBuf::from(path), hash as u64);
            // }

            //Normalize the paths and remove the parent directory folder.
            let path = entry.path().as_os_str().to_string_lossy().replace(path, "");

            btree.insert(PathBuf::from(path), flsz);
        }
    }

    // println!("{:#?}", btree);

    println!(
        "Hashed {} items with a total size of: {}mb",
        total,
        size / 1000000
    );

    return btree;
}

fn main() {
    defer_results!();
    profile!();

    let args: Vec<String> = std::env::args().skip(1).collect();

    //ft <source> <destination> --cleanup
    if args.len() < 2 {
        eprintln!("usage: ft <source> <destion>");
        return;
    }

    let home = home::home_dir().unwrap();
    let home = home.to_string_lossy();

    let source_path = &args[0].replace("~/", &home);
    let destination_path = &args[1].replace("~/", &home);
    dbg!(source_path, destination_path);

    let sp = source_path.clone();
    let dp = destination_path.clone();

    let source = thread::spawn(move || generate_tree(&sp));
    //This is super slow because it's over the network.
    let destination = thread::spawn(move || generate_tree(&dp));

    let source = source.join().unwrap();
    let destination = destination.join().unwrap();

    // dbg!(source);
    // dbg!(destination);
    // return;

    for (key, hash) in &source {
        if let Some(dest_hash) = destination.get(key) {
            if hash != dest_hash {
                //TODO: Create and test a crc mismatch.
                println!(
                    "'{}' expected hash: {} but found: {}",
                    key.file_name().unwrap_or_default().to_string_lossy(),
                    // key,
                    hash,
                    dest_hash
                );
                let str = key.as_os_str().to_string_lossy();
                let _to = str.replace(source_path, &destination_path);
                // println!("Copying {:#?} to {}", &key, to);
            }
        } else {
            let str = key.as_os_str().to_string_lossy();
            let from = format!("{}{}", source_path, str);
            let to = format!("{}{}", destination_path, str);
            println!("Did not find {} copying to {}", from, to);
            let to = Path::new(&to);
            if let Some(parent) = to.parent() {
                if !parent.exists() {
                    create_dir_all(parent).unwrap();
                }
            }
            // std::fs::copy(&from, to).unwrap();
        }
    }

    //Check for redundant files.
    for (key, _) in destination {
        if !source.contains_key(&key) {
            println!("Key {:#?} should not exist", key);
        }
    }
}
