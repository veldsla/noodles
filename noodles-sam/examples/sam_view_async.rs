//! Validates and prints the records in a SAM file.
//!
//! The result matches the output of `samtools view <src>`.

use std::env;

use futures::TryStreamExt;
use noodles_sam as sam;
use tokio::{
    fs::File,
    io::{self, BufReader},
};

#[tokio::main]
async fn main() -> io::Result<()> {
    let src = env::args().nth(1).expect("missing src");

    let mut reader = File::open(src)
        .await
        .map(BufReader::new)
        .map(sam::AsyncReader::new)?;

    reader.read_header().await?;

    let mut records = reader.records();

    while let Some(record) = records.try_next().await? {
        println!("{}", record);
    }

    Ok(())
}
