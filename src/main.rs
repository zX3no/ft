use blake3::*;
use mini::{defer_results, profile};
use std::{
    collections::BTreeMap,
    io::Cursor,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
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

fn generate_tree(path: &str) -> BTreeMap<PathBuf, u32> {
    let mut total = 0;
    let mut size = 0;
    // Ideally this would be &'a str or &'a OsStr, I could do this with winwalk not sure about MacOS.
    let mut btree: BTreeMap<PathBuf, u32> = BTreeMap::new();
    for entry in WalkDir::new(path).sort_by_file_name() {
        if let Ok(entry) = entry {
            total += 1;
            size += entry.metadata().unwrap().size();
            if let Ok(hash) = hash_crc(entry.path()) {
                //Normalize the paths and remove the parent directory folder.
                let path = entry.path().as_os_str().to_string_lossy().replace(path, "");
                btree.insert(PathBuf::from(path), hash);
            }
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

    let source_path = args.get(0).unwrap();
    let destination_path = args.get(1).unwrap();
    dbg!(source_path, destination_path);

    let source_path = "/Users/Bay/Music/Opus/Iglooghost";
    let source = generate_tree(source_path);
    let destination_path = "/Users/Bay/Music/Example";
    let destination = generate_tree(destination_path);

    for (key, hash) in &source {
        if let Some(dest_hash) = destination.get(key) {
            if hash != dest_hash {
                //TODO: Create and test a crc mismatch.
                println!("Key {:?} has crc mismatch", &key);
                let str = key.as_os_str().to_string_lossy();
                let to = str.replace(source_path, &destination_path);
                println!("Copying {:#?} to {}", &key, to);
            }
        } else {
            let str = key.as_os_str().to_string_lossy();
            let from = format!("{}{}", source_path, str);
            let to = format!("{}{}", destination_path, str);

            println!("Copying {} to {}", from, to);
            // std::fs::copy(&key, to);
        }
    }

    //Check for redundant files.
    for (key, _) in destination {
        if !source.contains_key(&key) {
            println!("Key {:#?} should not exist", key);
        }
    }
}
