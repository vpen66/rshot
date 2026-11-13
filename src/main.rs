use anyhow::Result;
use clap::Parser;
use log::info;
use rshot_core::{ClipboardService, ScreenCapture};
use rshot_overlay::launch_overlay;
use rshot_storage::{OutputFormat, StorageService};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "rshot",
    author = "rshot developers",
    version = "0.1.0",
    about = "A modern, high-performance cross-platform screenshot & screen recording productivity tool"
)]
struct Args {
    /// Directly capture the entire primary screen without launching the interactive overlay
    #[arg(short, long)]
    fullscreen: bool,

    /// Launch the interactive screenshot overlay immediately instead of the resident tray
    #[arg(long)]
    capture: bool,

    /// Specify a custom output path to save the screenshot
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Print all detected monitors and their display scaling
    #[arg(long)]
    list_monitors: bool,

    /// Print visible on-screen windows (useful for window snapping)
    #[arg(long)]
    list_windows: bool,

    /// Internal handoff: launch the pin window from a temp image written by the overlay
    #[arg(long, hide = true)]
    pin_image: Option<PathBuf>,

    #[arg(long, hide = true)]
    pin_x: Option<f32>,

    #[arg(long, hide = true)]
    pin_y: Option<f32>,

    #[arg(long, hide = true)]
    pin_scale: Option<f32>,
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();

    if let Some(path) = args.pin_image {
        let img = image::open(&path)?.to_rgba8();
        let _ = std::fs::remove_file(&path);
        return rshot_pin::launch_pin(
            img,
            args.pin_x.unwrap_or(0.0),
            args.pin_y.unwrap_or(0.0),
            args.pin_scale.unwrap_or(1.0),
        )
        .map_err(|e| anyhow::anyhow!("{e}"));
    }

    if args.list_monitors {
        println!("=== Detected Monitors ===");
        let monitors = ScreenCapture::get_monitors()?;
        for m in monitors {
            let primary = if m.is_primary { " [Primary]" } else { "" };
            println!(
                "ID: {} | Name: \"{}\"{} | Bounds: {}x{} at ({}, {}) | Scale: {:.2}x",
                m.id, m.name, primary, m.width, m.height, m.x, m.y, m.scale_factor
            );
        }
        return Ok(());
    }

    if args.list_windows {
        println!("=== Visible Windows ===");
        let windows = ScreenCapture::get_windows()?;
        for w in windows {
            println!(
                "[{}] \"{}\" ({}) - {}x{} at ({}, {})",
                w.id, w.title, w.app_name, w.width, w.height, w.x, w.y
            );
        }
        return Ok(());
    }

    if args.fullscreen {
        info!("Direct fullscreen capture mode triggered.");
        let (_info, image) = ScreenCapture::capture_primary_monitor()?;

        // Copy to clipboard
        if let Err(e) = ClipboardService::copy_image(&image) {
            eprintln!("Warning: Failed to copy to clipboard: {e}");
        } else {
            println!("✔ Screenshot copied to clipboard!");
        }

        // Save to file
        let saved_path = if let Some(custom_path) = args.output {
            StorageService::save_image(&image, &custom_path)?;
            custom_path
        } else {
            StorageService::save_to_default_dir(&image, OutputFormat::Png)?
        };

        println!("✔ Screenshot saved to: {}", saved_path.display());
        return Ok(());
    }

    if args.capture {
        info!("Starting interactive screenshot overlay...");
        if let Err(e) = launch_overlay() {
            eprintln!("Failed to run screenshot overlay: {e}");
        }
        return Ok(());
    }

    // Default mode: resident menu-bar app; launching does not capture immediately.
    info!("Starting RShot tray (resident)...");
    rshot_tray::run()?;

    Ok(())
}
