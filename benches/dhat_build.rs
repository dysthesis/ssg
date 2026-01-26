use dhat::{DhatAlloc, Profiler};

use ssg::pipeline::build_at;

mod fixtures;
use fixtures::{SiteOptions, make_site};

#[global_allocator]
static ALLOC: DhatAlloc = DhatAlloc;

fn main() {
    let _prof = Profiler::builder().file_name("dhat-build.json").build();

    let opts = SiteOptions {
        posts: 120,
        body_bytes: 6_000,
        with_code: true,
        with_math: true,
        with_footnotes: true,
        with_images: true,
    };

    let site = make_site(&opts);
    build_at(site.path()).expect("build succeeds");
}
