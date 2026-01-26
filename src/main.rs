use std::env::{self, current_dir};

use axum::Router;
use color_eyre::Section;
use notify::{EventKind, RecursiveMode, Watcher};
use ssg::{
    config::{INPUT_DIR, OUTPUT_DIR},
    pipeline::build_once,
};
use tower_http::services::ServeDir;
use tower_livereload::LiveReloadLayer;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    if env::args().any(|a| a == "serve") {
        serve().await?;
    } else {
        build_once()?;
    }

    Ok(())
}

async fn serve() -> color_eyre::Result<()> {
    // Initial build
    println!("Building site...");
    build_once()?;

    let current_dir = current_dir().with_note(|| "While getting the current working directory")?;
    let public_dir = current_dir.join(OUTPUT_DIR);
    let contents_dir = current_dir.join(INPUT_DIR);
    let css_src = current_dir.join("style.css");

    // Setup live reload
    let livereload = LiveReloadLayer::new();
    let reloader = livereload.reloader();

    // Setup file watcher
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        match res {
            Ok(event) => {
                // Ignore Access events (triggered when reading files) to
                // prevent infinite loops
                if matches!(event.kind, EventKind::Access(_)) {
                    return;
                }

                println!("Change detected, rebuilding...");
                // We ignore build errors during watch mode to keep the server
                // alive
                if let Err(e) = build_once() {
                    eprintln!("Build failed: {}", e);
                } else {
                    println!("Rebuild complete.");
                    reloader.reload();
                }
            }
            Err(e) => eprintln!("Watch error: {}", e),
        }
    })?;

    // Watch contents directory and the style.css file
    watcher.watch(&contents_dir, RecursiveMode::Recursive)?;
    if css_src.exists() {
        watcher.watch(&css_src, RecursiveMode::NonRecursive)?;
    }

    // Setup Axum router
    let app = Router::new()
        .fallback_service(ServeDir::new(public_dir))
        .layer(livereload);

    println!("Serving on http://localhost:3000");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}
