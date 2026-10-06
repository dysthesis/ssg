use std::{
    env::{self, current_dir},
    path::PathBuf,
};

use color_eyre::eyre;
use ssg::{
    ctx::Ctx,
    fs::Dir,
    query::read::Read,
    store::{Id, Store},
};
use walkdir::WalkDir;

use crate::cli::Cli;

const STORE_PATH: &'static str = "store/";
const DB_PATH: &'static str = "ssg.db";

mod cli;
fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse()?;
    match cli {
        Cli::Init(path) => {
            let root = std::path::absolute(path)?;
            let _ = Store::init(Dir::new(&root)?);
        }
        Cli::Put(bytes) => {
            let curr_dir = env::current_dir()?;
            let store = Store::init(Dir::new(&curr_dir)?)?;
            let key = store.put(bytes.as_bytes())?;
            println!("Stored as {key}")
        }
        Cli::Get(key) => {
            let key = key.parse::<Id>()?;
            let curr_dir = env::current_dir()?;
            let store = Store::init(Dir::new(&curr_dir)?)?;
            let bytes = store.get(key)?;
            println!("Obtained bytes: {bytes:?}")
        }
        Cli::Read(dir) => {
            let mut ctx = Ctx::new(&current_dir()?.join(STORE_PATH), &PathBuf::from(DB_PATH))?;
            for file in WalkDir::new(dir)
                .into_iter()
                .filter_map(|entry| match entry {
                    Ok(entry) if entry.file_type().is_file() => Some(entry.into_path()),
                    Ok(_) => None,
                    Err(_err) => None,
                })
            {
                let query = Read::new(file);
                let id = ctx.register_query(query);
                ctx.run(id);
            }
        }
    }
    Ok(())
}
