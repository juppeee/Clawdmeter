//! Flash a board from a local release folder, through the same code path as
//! the app's flasher, without opening a window.
//!
//!     cargo run -p clawdmeter-flash --example flash_local -- <dist-dir> <env> <port>
//!
//! `<dist-dir>` holds `manifest.json` and the merged images
//! (`tools/ci/merge_firmware.py` + `make_manifest.py` produce it).

use std::path::PathBuf;
use std::{env, fs, process};

use clawdmeter_flash::device::{self, FlashProgress};
use clawdmeter_flash::release;
use clawdmeter_flash::Manifest;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let [dist, env_name, port] = args.as_slice() else {
        eprintln!("usage: flash_local <dist-dir> <env> <port>");
        process::exit(2);
    };
    if let Err(e) = run(PathBuf::from(dist), env_name, port) {
        eprintln!("error: {e}");
        process::exit(1);
    }
}

fn run(dist: PathBuf, env_name: &str, port: &str) -> Result<(), Box<dyn std::error::Error>> {
    let manifest = Manifest::parse(&fs::read_to_string(dist.join("manifest.json"))?)?;
    let board = manifest.board(env_name).ok_or_else(|| format!("{env_name} is not in the manifest"))?;
    let image = fs::read(dist.join(&board.file))?;
    release::verify(board, &image)?;
    println!("{} {} verified ({} bytes)", board.file, manifest.version, image.len());

    let mut total = 0;
    let mut last_pct = usize::MAX;
    device::flash_merged(port, &image, board, |p| match p {
        FlashProgress::Start { total: t } => total = t.max(1),
        FlashProgress::Write { done } => {
            let pct = done * 100 / total.max(1);
            if pct / 10 != last_pct / 10 {
                println!("  {pct}%");
                last_pct = pct;
            }
        }
        FlashProgress::Verify => println!("  verifying"),
        FlashProgress::Done => println!("  done, resetting"),
    })?;
    Ok(())
}
