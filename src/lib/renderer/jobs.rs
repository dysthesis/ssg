use color_eyre::{Section, eyre::Result};
use pulldown_cmark::{CowStr, Event};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::renderer::{CodeblockHighlighter, MathRenderer};

/// How mnay threads each page should have to render code blocks and math blocks
const NUM_THREADS: usize = 8;

/// How many jobs each thread should have to make paralelism worth it
const JOBS_PER_THREAD: usize = 2;

pub trait Job: Sync {
    fn execute(&self) -> (usize, Event<'static>);
}

/// A job to render a code block
pub struct CodeBlockJob<H>
where
    H: CodeblockHighlighter + Sync,
{
    /// The highlighter to highlight the code block syntax ith
    pub highlighter: H,
    /// The position where the corresponding event exists in the events list.
    pub idx: usize,
    /// The inner contents of the code block.
    pub source: String,
    /// The language to highlight the code block with.
    pub lang: String,
}

impl<H> Job for CodeBlockJob<H>
where
    H: CodeblockHighlighter + Sync,
{
    fn execute(&self) -> (usize, Event<'static>) {
        let lang_opt = if self.lang.is_empty() {
            None
        } else {
            Some(self.lang.as_str())
        };
        let highlighted = self
            .highlighter
            .render_codeblock(&self.source, lang_opt)
            .to_string();

        (self.idx, Event::Html(CowStr::from(highlighted)))
    }
}

pub struct InlineMathJob<M>
where
    M: MathRenderer + Sync,
{
    /// The renderer to render the inline math block with.
    pub renderer: M,
    /// The position where the corresponding event exists in the events list.
    pub idx: usize,
    /// The inner contents of the inline math.
    pub source: String,
}

impl<M> Job for InlineMathJob<M>
where
    M: MathRenderer + Sync,
{
    fn execute(&self) -> (usize, Event<'static>) {
        let rendered = self.renderer.render_math(&self.source, false).to_string();
        (self.idx, Event::Html(CowStr::from(rendered)))
    }
}

pub struct DisplayMathJob<M>
where
    M: MathRenderer,
{
    /// The renderer to render the inline math block with.
    pub renderer: M,
    /// The position where the corresponding event exists in the events list.
    pub idx: usize,
    /// The inner contents of the display math.
    pub source: String,
}

impl<M> Job for DisplayMathJob<M>
where
    M: MathRenderer + Sync,
{
    fn execute(&self) -> (usize, Event<'static>) {
        let rendered = self.renderer.render_math(&self.source, true).to_string();
        (self.idx, Event::Html(CowStr::from(rendered)))
    }
}

/// A queue of rendering jobs to execute.
pub struct Jobs<'a>(Vec<&'a dyn Job>);
impl<'a> Jobs<'a> {
    /// Check whether the number of jobs warrants running in parallel
    pub fn should_paralellise(&self) -> bool {
        let job_cnt = self.0.len();
        let num_threads = rayon::current_num_threads();
        job_cnt >= num_threads.saturating_mul(JOBS_PER_THREAD)
    }

    /// Run jobs sequentially
    pub fn execute_seq(&self) -> Vec<(usize, Event<'static>)> {
        self.0.iter().map(|job| job.execute()).collect()
    }

    /// Run jobs in parallel
    pub fn execute_par(&self) -> Result<Vec<(usize, Event<'static>)>> {
        let thread_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(NUM_THREADS)
            .build()
            .with_note(
                || "Encountered while constructing a thread pool for rendering page in parllel",
            )?;

        Ok(thread_pool.install(|| self.0.par_iter().map(|job| job.execute()).collect()))
    }

    /// Run all of the jobs
    pub fn execute(&self) -> Result<Vec<(usize, Event<'static>)>> {
        if self.should_paralellise() {
            self.execute_par()
        } else {
            Ok(self.execute_seq())
        }
    }
}

impl<'a> FromIterator<&'a dyn Job> for Jobs<'a> {
    fn from_iter<T: IntoIterator<Item = &'a dyn Job>>(iter: T) -> Self {
        let inner = iter.into_iter().collect();
        Self(inner)
    }
}
