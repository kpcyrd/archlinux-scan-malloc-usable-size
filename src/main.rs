use anyhow::{anyhow, Context, Result};
use env_logger::Env;
use log::error;
use memfile::MemFile;
use std::env;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;
use std::process::Stdio;
use std::sync::mpsc::channel;
use std::thread;
use threadpool::ThreadPool;
use walkdir::WalkDir;

fn check_elf_file<R: Read>(mut f: R) -> Result<bool> {
    let mut elf_magic = [0u8; 4];
    if let Err(err) = f.read_exact(&mut elf_magic) {
        if err.kind() == ErrorKind::UnexpectedEof {
            return Ok(false);
        } else {
            return Err(err.into());
        }
    }
    if elf_magic != [0x7f, 0x45, 0x4c, 0x46] {
        return Ok(false);
    }

    let mut mem = MemFile::create_default("")?;
    mem.write_all(&elf_magic)?;
    io::copy(&mut f, &mut mem).context("Failed to read elf from archive")?;
    mem.rewind()?;

    let output = Command::new("readelf")
        .arg("-Ws")
        .arg("/dev/stdin")
        .stdin(mem)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .context("Failed to run readelf")?;

    for line in output.stdout.lines() {
        // let line = line.context("Failed to parse readelf line")?;
        if let Ok(line) = line {
            if line.contains(" malloc_usable_size@GLIBC") {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

fn check_tar<R: Read>(r: R) -> Result<bool> {
    let mut tar = tar::Archive::new(r);
    for entry in tar.entries()? {
        let entry = entry?;
        let path = entry.path()?.into_owned();
        if !entry.header().entry_type().is_file() {
            continue;
        }

        if check_elf_file(entry).with_context(|| anyhow!("Failed to check elf file: {:?}", path))? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn check_pkg(path: &Path) -> Result<bool> {
    let Some(file_name) = path.file_name() else {
        return Ok(false);
    };
    let Some(file_name) = file_name.to_str() else {
        return Ok(false);
    };

    if file_name.ends_with(".pkg.tar.zst") {
        let f = File::open(path)?;
        let r = zstd::Decoder::new(f)?;
        check_tar(r)
    } else if file_name.ends_with(".pkg.tar.xz") {
        let f = File::open(path)?;
        let r = lzma::LzmaReader::new_decompressor(f)?;
        check_tar(r)
    } else {
        Ok(false)
    }
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let path = env::args()
        .skip(1)
        .next()
        .context("Missing path argument")?;

    let rx = {
        let (tx, rx) = channel();
        thread::spawn(move || {
            let pool = ThreadPool::new(num_cpus::get());
            for entry in WalkDir::new(path) {
                let entry = entry.unwrap();
                if !entry.file_type().is_file() {
                    continue;
                }
                let tx = tx.clone();
                let path = entry.path().to_owned();
                pool.execute(move || {
                    match check_pkg(&path) {
                        Ok(true) => {
                            tx.send(path).ok();
                        }
                        Ok(false) => {}
                        Err(err) => {
                            error!("Failed to check package ({path:?}): {err:#}");
                        }
                    };
                });
            }
        });
        rx
    };

    for result in rx {
        println!("{}", result.display());
    }

    eprintln!("done");
    Ok(())
}
