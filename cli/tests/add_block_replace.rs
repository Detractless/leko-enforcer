//! The save is all-or-nothing (AC-011): a save that fails at the write, at the move, or because the
//! folder cannot be written leaves the old file byte for byte and no temporary file, and says so; a
//! good save leaves no temporary file, keeps the permission bits, and through a link replaces the file
//! the link points to and keeps the link. The write and the move are made to fail by test steps.

mod add_block_runs;
mod run_capture;
mod scratch_folder;
mod settings_builder;

use add_block_runs::{add, entries_of, plain_and, saved_block};
use leko_cli::add_block::file_replace::{replace_file, Steps};
use scratch_folder::Scratch;
use serde_json::json;
use settings_builder::{always, block, settings_bytes, write_settings};
use std::fs::File;
use std::io::{Error, ErrorKind, Write};
use std::path::{Path, PathBuf};

/// A write that puts half of the bytes down and then fails, as a full disk does.
struct HalfThenFull {
    temp_seen: Option<PathBuf>,
}

impl Steps for HalfThenFull {
    fn write(&mut self, file: &mut File, bytes: &[u8]) -> std::io::Result<()> {
        file.write_all(&bytes[..bytes.len() / 2])?;
        file.flush()?;
        self.temp_seen = Some(PathBuf::from("half written"));
        Err(Error::new(ErrorKind::Other, "disk full"))
    }
}

/// A move that finds the new file whole beside the old one, and is refused.
struct MoveRefused {
    new_bytes: Vec<u8>,
    old_bytes: Vec<u8>,
    moved_from: Option<PathBuf>,
}

impl Steps for MoveRefused {
    fn rename(&mut self, from: &Path, to: &Path) -> std::io::Result<()> {
        assert_eq!(std::fs::read(from).unwrap(), self.new_bytes, "the new file is whole before the move");
        assert_eq!(std::fs::read(to).unwrap(), self.old_bytes, "the old file is still the old one before the move");
        self.moved_from = Some(from.to_path_buf());
        Err(Error::new(ErrorKind::PermissionDenied, "move refused"))
    }
}

fn old_and_new() -> (Vec<u8>, Vec<u8>) {
    let old = settings_bytes(&[block("old", json!({"apps": ["com.old.app"]}), json!([always("r", &[])]))]);
    let new = settings_bytes(&[block("old", json!({"apps": ["com.old.app"]}), json!([always("r", &[])])), block("new", json!({"apps": ["com.new.app"]}), json!([always("r", &[])]))]);
    (old, new)
}

fn left_as_it_was(sentence: &str, file: &Path, folder: &Scratch, old: &[u8], step: &str) {
    assert!(sentence.starts_with(&format!("The block could not be saved: {step} (")), "{sentence}");
    assert!(sentence.ends_with("was left as it was.") && !sentence.contains('\n'), "{sentence}");
    assert_eq!(std::fs::read(file).unwrap(), old, "the old file is byte for byte as it was");
    assert_eq!(folder.names(), ["settings.zip"], "no temporary file is left");
}

#[test]
fn a_failed_save_leaves_the_old_file_and_no_temp_file_and_a_good_save_keeps_mode_and_link() {
    let (old, new) = old_and_new();

    // The write fails part-way: the temporary file held half the bytes, and is gone.
    let folder = Scratch::new("replace-write");
    let file = folder.path().join("settings.zip");
    std::fs::write(&file, &old).unwrap();
    let mut failing = HalfThenFull { temp_seen: None };
    let sentence = replace_file(&file.to_string_lossy(), &new, &mut failing).unwrap_err();
    assert!(failing.temp_seen.is_some() && sentence.contains("(disk full)"), "{sentence}");
    left_as_it_was(&sentence, &file, &folder, &old, "the new file could not be written");

    // The move is refused: the new file was whole and the old one untouched, and now the new one is gone.
    let folder = Scratch::new("replace-move");
    let file = folder.path().join("settings.zip");
    std::fs::write(&file, &old).unwrap();
    let mut refusing = MoveRefused { new_bytes: new.clone(), old_bytes: old.clone(), moved_from: None };
    let sentence = replace_file(&file.to_string_lossy(), &new, &mut refusing).unwrap_err();
    assert!(refusing.moved_from.is_some() && sentence.contains("(move refused)"), "{sentence}");
    left_as_it_was(&sentence, &file, &folder, &old, "the new file could not be moved into place");

    // A folder nobody may write to: the temporary file cannot be made. Through the command, with exit 1.
    #[cfg(unix)]
    {
        let folder = Scratch::new("replace-readonly");
        let file = write_settings(folder.path(), "settings.zip", &[block("old", json!({"apps": ["com.old.app"]}), json!([always("r", &[])]))]);
        folder.set_mode(0o555);
        if std::fs::File::create(folder.path().join("probe")).is_err() {
            let run = add(&file, &plain_and(&["--write"]));
            assert_eq!((run.code, run.out.as_str()), (1, ""), "{}", run.err);
            assert!(run.err.starts_with("The block could not be saved: a temporary file could not be made beside it (Permission denied"), "{}", run.err);
            assert!(run.err.ends_with("was left as it was.\n") && run.err.lines().count() == 1, "{}", run.err);
            assert_eq!(std::fs::read(&file).unwrap(), settings_bytes(&[block("old", json!({"apps": ["com.old.app"]}), json!([always("r", &[])]))]));
            assert_eq!(folder.names(), ["settings.zip"]);
        }
        folder.set_mode(0o755);
    }

    // A good save keeps the permission bits (including a file nobody may write) and leaves no temp file.
    #[cfg(unix)]
    for mode in [0o600, 0o640, 0o644, 0o444, 0o666] {
        use std::os::unix::fs::PermissionsExt;
        let folder = Scratch::new("replace-mode");
        let file = folder.path().join("settings.zip");
        std::fs::write(&file, &old).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(mode)).unwrap();
        let run = add(&file, &plain_and(&["--write"]));
        assert_eq!((run.code, run.err.as_str()), (0, ""), "mode {mode:o}: {}", run.out);
        assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o7777, mode, "the permission bits are kept");
        assert_eq!(folder.names(), ["settings.zip"], "mode {mode:o}");
        assert!(saved_block(&file, "social-media").is_some(), "mode {mode:o}");
    }

    // A link is followed: the file it points to is replaced and the link stays, whether it points by
    // an absolute path or a relative one, from another folder, or through a chain of links.
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let folder = Scratch::new("replace-link");
        std::fs::create_dir(folder.path().join("real")).unwrap();
        std::fs::create_dir(folder.path().join("links")).unwrap();
        let real = folder.path().join("real").join("settings.zip");
        std::fs::write(&real, &old).unwrap();
        let (absolute, relative, chain) = (folder.path().join("links").join("absolute.zip"), folder.path().join("links").join("relative.zip"), folder.path().join("links").join("chain.zip"));
        symlink(&real, &absolute).unwrap();
        symlink("../real/settings.zip", &relative).unwrap();
        symlink("absolute.zip", &chain).unwrap();
        let targets = |link: &Path| std::fs::read_link(link).unwrap();
        let before = (targets(&absolute), targets(&relative), targets(&chain));
        for (number, link) in [&absolute, &relative, &chain].into_iter().enumerate() {
            let name = format!("Block {number}");
            let run = add(link, &["--name", &name, "--app", "com.example.app", "--always", "--write"]);
            assert_eq!((run.code, run.err.as_str()), (0, ""), "{}", run.out);
            assert!(std::fs::symlink_metadata(link).unwrap().file_type().is_symlink(), "{} is still a link", link.display());
            assert!(saved_block(&real, &format!("block-{number}")).is_some(), "the file the link points to was replaced");
        }
        assert_eq!((targets(&absolute), targets(&relative), targets(&chain)), before, "the links point where they did");
        assert_eq!(entries_of(&real).iter().filter(|entry| entry.0.starts_with("blocks/block-")).count(), 3);
        assert_eq!(folder.names(), ["links", "real"]);
        let mut in_real = std::fs::read_dir(folder.path().join("real")).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect::<Vec<_>>();
        in_real.sort();
        assert_eq!(in_real, ["settings.zip"], "the temporary file was made beside the real file and is gone");
        assert_eq!(std::fs::read_dir(folder.path().join("links")).unwrap().count(), 3, "nothing came into the folder of the links");
    }

    // A temporary name that is already taken is never overwritten; the next number is used.
    let folder = Scratch::new("replace-stale");
    let file = folder.path().join("settings.zip");
    std::fs::write(&file, &old).unwrap();
    std::fs::write(folder.path().join(".settings.zip.leko-tmp-0"), b"someone else's file").unwrap();
    let run = add(&file, &plain_and(&["--write"]));
    assert_eq!((run.code, run.err.as_str()), (0, ""), "{}", run.out);
    assert_eq!(std::fs::read(folder.path().join(".settings.zip.leko-tmp-0")).unwrap(), b"someone else's file");
    assert_eq!(folder.names(), [".settings.zip.leko-tmp-0", "settings.zip"], "only the other program's file is left beside it");
    assert!(saved_block(&file, "social-media").is_some());
}
