use std::env;

use color_eyre::eyre;
use ssg::store::Store;

use crate::cli::Cli;

mod cli;
fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse()?;
    match cli {
        Cli::Init(path) => {
            let _ = Store::init(path.into());
        }
        Cli::Put(bytes) => {
            let curr_dir = env::current_dir()?;
            let store = Store::init(curr_dir)?;
            let key = store.put(bytes.as_bytes())?;
            println!("Stored as {key}")
        }
        Cli::Get(key) => {
            let key = blake3::Hash::from_hex(key)?;
            let curr_dir = env::current_dir()?;
            let store = Store::init(curr_dir)?;
            let bytes = store.get(key)?;
            println!("Obtained bytes: {bytes:?}")
        }
    };
    Ok(())
}
