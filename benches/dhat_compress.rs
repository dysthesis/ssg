use std::io::Write;

use brotli::CompressorWriter;
use dhat::{DhatAlloc, Profiler};
use flate2::{Compression, write::GzEncoder};

mod fixtures;
use fixtures::rust_snippet;

#[global_allocator]
static ALLOC: DhatAlloc = DhatAlloc;

fn main() {
    let _prof = Profiler::builder().file_name("dhat-compress.json").build();

    let data = rust_snippet(15_000);

    // Gzip
    let mut gz = GzEncoder::new(Vec::new(), Compression::best());
    gz.write_all(data.as_bytes()).unwrap();
    let gz_out = gz.finish().unwrap();

    // Brotli
    let mut br = CompressorWriter::new(Vec::new(), 4096, 11, 22);
    br.write_all(data.as_bytes()).unwrap();
    let br_out = br.into_inner();

    // Keep outputs alive to be counted.
    dhat::md::black_box((gz_out.len(), br_out.len()));
}
