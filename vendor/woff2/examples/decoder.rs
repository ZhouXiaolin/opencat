use std::{io, path::PathBuf};

use clap::Parser;
use thiserror::Error;
use woff2::decode::{convert_woff2_to_ttf, DecodeError};

#[derive(Debug, Error)]
enum Error {
    #[error(transparent)]
    Woff(#[from] DecodeError),
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[derive(Debug, Parser)]
struct Args {
    in_path: PathBuf,
    out_path: PathBuf,
}

fn main() -> Result<(), Error> {
    let args = Args::parse();
    let input = std::fs::read(args.in_path)?;
    let ttf = convert_woff2_to_ttf(&mut io::Cursor::new(input))?;
    std::fs::write(args.out_path, ttf)?;
    Ok(())
}
