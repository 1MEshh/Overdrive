use clap::{Args, Parser, Subcommand};
use overdrive::chain::{shell_escape, LaunchChainBuilder};
use overdrive::doctor::run_diagnostics;
use overdrive::profile::{
    list_builtin_profiles, resolve_profile, Profile,
};
use overdrive::telemetry::{calculate_metrics, parse_frametimes_csv};
use overdrive::vdf::{parse_app_manifest, parse_library_folders, parse_vdf};
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "overdrive",
    author = "1MEshh <152591660+1MEshh@users.noreply.github.com>",
    version = "0.1.0",
    about = "High-performance Linux gaming optimizer, Steam launch wrapper compiler, Proton/VKD3D environment tweaker, and frametime pacing telemetry engine."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Audit host Linux gaming environment (kernel sysctls, Vulkan ICDs, CPU governors)
    Doctor {
        /// Output diagnostics report in JSON format
        #[arg(long)]
        json: bool,
    },

    /// Synthesize and execute optimized gaming launch wrappers
    Run(RunArgs),

    /// Ingest and analyze frametime telemetry CSV from MangoHud, CapFrameX, or PresentMon
    Bench {
        /// Path to frametime CSV file
        csv_file: PathBuf,

        /// Output benchmark telemetry metrics in JSON format
        #[arg(long)]
        json: bool,

        /// Export summary benchmark metrics to specified CSV file
        #[arg(long)]
        export_csv: Option<PathBuf>,
    },

    /// Inspect, list, and template gaming profiles
    Profiles {
        #[command(subcommand)]
        sub: ProfilesSubcommand,
    },

    /// Inspect and parse Valve KeyValues VDF and ACF manifest files
    Vdf {
        #[command(subcommand)]
        sub: VdfSubcommand,
    },
}

#[derive(Args, Debug)]
pub struct RunArgs {
    /// Target Steam AppID (resolves manifest and default profile)
    #[arg(long)]
    appid: Option<u32>,

    /// Profile name or path (e.g. competitive-fps, unreal-engine-5, low-latency-wayland)
    #[arg(short, long)]
    profile: Option<String>,

    /// Force enable Feral GameMode (gamemoderun) wrapper
    #[arg(long)]
    gamemode: bool,

    /// Force disable GameMode
    #[arg(long)]
    no_gamemode: bool,

    /// Force enable MangoHud overlay
    #[arg(long)]
    mangohud: bool,

    /// Force disable MangoHud
    #[arg(long)]
    no_mangohud: bool,

    /// Force enable Gamescope nested compositor
    #[arg(long)]
    gamescope: bool,

    /// Force disable Gamescope
    #[arg(long)]
    no_gamescope: bool,

    /// Gamescope target output resolution (format: WxH, e.g. 1920x1080 or 2560x1440)
    #[arg(long)]
    resolution: Option<String>,

    /// Gamescope or driver framerate cap (FPS)
    #[arg(long)]
    fps_limit: Option<u32>,

    /// Custom environment variables to inject (format: KEY=VALUE)
    #[arg(short = 'e', long = "env", value_parser = parse_key_val)]
    env_vars: Vec<(String, String)>,

    /// Print synthesized command and environment without launching process
    #[arg(long)]
    dry_run: bool,

    /// Generate launch string formatted for Steam's Launch Options box (%command%)
    #[arg(long)]
    print_steam: bool,

    /// Target game command and arguments to execute (after --)
    #[arg(trailing_var_arg = true)]
    command: Vec<String>,
}

fn parse_key_val(s: &str) -> Result<(String, String), String> {
    s.split_once('=')
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .ok_or_else(|| format!("Invalid key=value pair: '{}'", s))
}

#[derive(Subcommand, Debug)]
enum ProfilesSubcommand {
    /// List all available built-in and user profiles
    List,

    /// Display TOML contents of a profile
    Show {
        /// Profile name or path
        name: String,
    },

    /// Generate a template profile TOML file
    Create {
        /// Name of the new profile to create
        name: String,

        /// Output directory (defaults to current directory)
        #[arg(short, long, default_value = ".")]
        out_dir: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum VdfSubcommand {
    /// Parse and display structured KeyValues tree from a VDF or ACF file
    Parse {
        /// Path to VDF or ACF file
        file: PathBuf,
    },

    /// Parse an appmanifest_<appid>.acf file and extract application metadata
    Manifest {
        /// Path to appmanifest_<appid>.acf file
        file: PathBuf,
    },

    /// Parse libraryfolders.vdf and display all discovered Steam library paths and AppIDs
    Libraries {
        /// Path to libraryfolders.vdf file
        file: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Commands::Doctor { json } => handle_doctor(json),
        Commands::Run(args) => handle_run(args),
        Commands::Bench {
            csv_file,
            json,
            export_csv,
        } => handle_bench(csv_file, json, export_csv),
        Commands::Profiles { sub } => handle_profiles(sub),
        Commands::Vdf { sub } => handle_vdf(sub),
    }
}

fn handle_doctor(json: bool) -> ExitCode {
    let report = run_diagnostics();
    if json {
        println!("{}", report.to_json());
    } else {
        println!("{}", report.to_ascii_report());
    }

    if report.is_critical() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn handle_run(args: RunArgs) -> ExitCode {
    // 1. Resolve base profile
    let mut profile = if let Some(p_name) = &args.profile {
        match resolve_profile(p_name, &[PathBuf::from("profiles"), PathBuf::from(".")]) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("[ERROR] Failed to resolve profile '{}': {}", p_name, e);
                return ExitCode::from(1);
            }
        }
    } else if let Some(appid) = args.appid {
        // Try to load <appid>.toml, otherwise fall back to default profile
        let app_profile_name = format!("{}", appid);
        resolve_profile(
            &app_profile_name,
            &[PathBuf::from("profiles"), PathBuf::from(".")],
        )
        .unwrap_or_default()
    } else {
        Profile::default()
    };

    // 2. Apply CLI overrides
    if args.gamemode {
        profile.wrapper.gamemode = true;
    }
    if args.no_gamemode {
        profile.wrapper.gamemode = false;
    }

    if args.mangohud {
        profile.wrapper.mangohud = true;
    }
    if args.no_mangohud {
        profile.wrapper.mangohud = false;
    }

    if args.gamescope {
        profile.wrapper.gamescope = true;
        profile.gamescope.enabled = true;
    }
    if args.no_gamescope {
        profile.wrapper.gamescope = false;
        profile.gamescope.enabled = false;
    }

    if let Some(res_str) = &args.resolution {
        if let Some((w_str, h_str)) = res_str.split_once('x') {
            if let (Ok(w), Ok(h)) = (w_str.parse::<u32>(), h_str.parse::<u32>()) {
                profile.gamescope.output_width = Some(w);
                profile.gamescope.output_height = Some(h);
                profile.gamescope.enabled = true;
            } else {
                eprintln!("[ERROR] Invalid resolution format '{}'. Expected WxH (e.g. 1920x1080)", res_str);
                return ExitCode::from(1);
            }
        }
    }

    if let Some(fps) = args.fps_limit {
        profile.gamescope.framerate_limit = Some(fps);
    }

    for (k, v) in args.env_vars {
        profile.env.insert(k, v);
    }

    // 3. Build command chain
    let builder = LaunchChainBuilder::from_profile(&profile, &args.command);

    let synth = builder.build();

    if args.print_steam {
        println!("{}", synth.to_steam_launch_string());
        return ExitCode::SUCCESS;
    }

    if args.dry_run {
        println!("+--------------------------------------------------------------+");
        println!("|           OVERDRIVE SYNTHESIZED LAUNCH CONFIGURATION         |");
        println!("+--------------------------------------------------------------+");
        println!("Environment Variables:");
        for (k, v) in &synth.env {
            println!("  export {}={}", k, shell_escape(v));
        }
        println!("\nExecutable Binary:");
        println!("  {}", synth.program);
        println!("\nExecution Arguments:");
        for (i, arg) in synth.args.iter().enumerate() {
            println!("  [{:>2}] {}", i, shell_escape(arg));
        }
        println!("\nFull Shell Invocation:");
        println!("  {}", synth.to_shell_string());
        println!("+--------------------------------------------------------------+");
        return ExitCode::SUCCESS;
    }

    // Direct execution
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let mut cmd = synth.to_command();
        let err = cmd.exec();
        eprintln!("[ERROR] Failed to execute process '{}': {}", synth.program, err);
        ExitCode::from(1)
    }

    #[cfg(not(unix))]
    {
        let mut cmd = synth.to_command();
        match cmd.status() {
            Ok(status) => {
                if let Some(code) = status.code() {
                    ExitCode::from(code as u8)
                } else {
                    ExitCode::from(1)
                }
            }
            Err(e) => {
                eprintln!("[ERROR] Failed to spawn process '{}': {}", synth.program, e);
                ExitCode::from(1)
            }
        }
    }
}

fn handle_bench(csv_file: PathBuf, json: bool, export_csv: Option<PathBuf>) -> ExitCode {
    if !csv_file.exists() {
        eprintln!(
            "[ERROR] Telemetry CSV file not found: {}",
            csv_file.display()
        );
        return ExitCode::from(1);
    }

    let content = match fs::read_to_string(&csv_file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "[ERROR] Failed to read {}: {}",
                csv_file.display(),
                e
            );
            return ExitCode::from(1);
        }
    };

    let frametimes = match parse_frametimes_csv(&content) {
        Ok(ft) => ft,
        Err(e) => {
            eprintln!("[ERROR] Failed to parse frametime telemetry: {}", e);
            return ExitCode::from(1);
        }
    };

    let metrics = match calculate_metrics(&frametimes) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("[ERROR] Failed to calculate telemetry metrics: {}", e);
            return ExitCode::from(1);
        }
    };

    if let Some(export_path) = export_csv {
        let summary = metrics.to_csv_summary();
        if let Err(e) = fs::write(&export_path, summary) {
            eprintln!(
                "[ERROR] Failed to export metrics CSV to {}: {}",
                export_path.display(),
                e
            );
            return ExitCode::from(1);
        }
        println!("[OK] Telemetry metrics summary exported to: {}", export_path.display());
    }

    if json {
        println!("{}", metrics.to_json());
    } else {
        println!("{}", metrics.to_ascii_card());
    }

    ExitCode::SUCCESS
}

fn handle_profiles(sub: ProfilesSubcommand) -> ExitCode {
    match sub {
        ProfilesSubcommand::List => {
            println!("+------------------------------------------------------------------------------+");
            println!("|                      OVERDRIVE BUILT-IN GAMING PROFILES                      |");
            println!("+----------------------+-------------------------------------------------------+");
            println!("| PROFILE NAME         | DESCRIPTION                                           |");
            println!("+----------------------+-------------------------------------------------------+");
            for (name, desc) in list_builtin_profiles() {
                println!("| {:<20} | {:<53} |", name, desc);
            }
            println!("+----------------------+-------------------------------------------------------+");

            // Look for user profiles
            let mut user_profiles = Vec::new();
            if let Ok(config_home) = std::env::var("OVERDRIVE_CONFIG_DIR") {
                let p = PathBuf::from(config_home).join("profiles");
                if let Ok(entries) = fs::read_dir(p) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().map_or(false, |ext| ext == "toml") {
                            user_profiles.push(path);
                        }
                    }
                }
            }

            if !user_profiles.is_empty() {
                println!("\nUser-Defined Profiles:");
                for p in user_profiles {
                    println!("  - {}", p.display());
                }
            }

            ExitCode::SUCCESS
        }
        ProfilesSubcommand::Show { name } => {
            match resolve_profile(&name, &[PathBuf::from("profiles"), PathBuf::from(".")]) {
                Ok(profile) => match profile.to_toml() {
                    Ok(toml_str) => {
                        println!("{}", toml_str);
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("[ERROR] Failed to format profile to TOML: {}", e);
                        ExitCode::from(1)
                    }
                },
                Err(e) => {
                    eprintln!("[ERROR] Profile '{}' not found: {}", name, e);
                    ExitCode::from(1)
                }
            }
        }
        ProfilesSubcommand::Create { name, out_dir } => {
            let file_name = if name.ends_with(".toml") {
                name
            } else {
                format!("{}.toml", name)
            };

            let target_path = out_dir.join(&file_name);
            if target_path.exists() {
                eprintln!("[ERROR] File '{}' already exists.", target_path.display());
                return ExitCode::from(1);
            }

            let template = format!(
                r#"[profile]
name = "{}"
description = "Custom tuned profile for {}"
target_gpu = "any"

[env]
vblank_mode = "0"
MESA_VK_WSI_PRESENT_MODE = "immediate"

[wrapper]
gamemode = true
gamescope = false
mangohud = true

[gamescope]
enabled = false
fullscreen = true

[cpu]
governor = "performance"
nice_priority = -5

[compat]
wine_sync = true
"#,
                file_name.trim_end_matches(".toml"),
                file_name.trim_end_matches(".toml")
            );

            if let Err(e) = fs::write(&target_path, template) {
                eprintln!("[ERROR] Failed to write template to '{}': {}", target_path.display(), e);
                ExitCode::from(1)
            } else {
                println!("[OK] Created template profile at: {}", target_path.display());
                ExitCode::SUCCESS
            }
        }
    }
}

fn handle_vdf(sub: VdfSubcommand) -> ExitCode {
    match sub {
        VdfSubcommand::Parse { file } => {
            let content = match fs::read_to_string(&file) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("[ERROR] Failed to read {}: {}", file.display(), e);
                    return ExitCode::from(1);
                }
            };

            match parse_vdf(&content) {
                Ok(val) => {
                    println!("{:#?}", val);
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("[ERROR] Failed to parse VDF: {}", e);
                    ExitCode::from(1)
                }
            }
        }
        VdfSubcommand::Manifest { file } => {
            let content = match fs::read_to_string(&file) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("[ERROR] Failed to read {}: {}", file.display(), e);
                    return ExitCode::from(1);
                }
            };

            match parse_app_manifest(&content) {
                Ok(manifest) => {
                    println!("+--------------------------------------------------------------+");
                    println!("|              STEAM APPLICATION MANIFEST RECORD               |");
                    println!("+--------------------------------------------------------------+");
                    println!("| App ID          : {:<42} |", manifest.appid);
                    println!("| Name            : {:<42} |", manifest.name);
                    println!("| Install Dir     : {:<42} |", manifest.installdir);
                    println!("| Size on Disk    : {:<36} B |", manifest.size_on_disk);
                    println!("| Build ID        : {:<42} |", manifest.build_id);
                    println!(
                        "| Proton Compat   : {:<42} |",
                        manifest.compat_tool.as_deref().unwrap_or("Native / Default")
                    );
                    println!("+--------------------------------------------------------------+");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("[ERROR] Failed to parse manifest: {}", e);
                    ExitCode::from(1)
                }
            }
        }
        VdfSubcommand::Libraries { file } => {
            let content = match fs::read_to_string(&file) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("[ERROR] Failed to read {}: {}", file.display(), e);
                    return ExitCode::from(1);
                }
            };

            match parse_library_folders(&content) {
                Ok(libraries) => {
                    println!("+--------------------------------------------------------------+");
                    println!("|               DISCOVERED STEAM LIBRARY FOLDERS               |");
                    println!("+--------------------------------------------------------------+");
                    for (i, lib) in libraries.iter().enumerate() {
                        println!("Library #{}:", i);
                        println!("  Path       : {}", lib.path.display());
                        println!("  Label      : {}", if lib.label.is_empty() { "(default)" } else { &lib.label });
                        println!("  Total Size : {} bytes", lib.totalsize);
                        println!("  Apps Count : {}", lib.apps.len());
                        println!("  App IDs    : {:?}", lib.apps);
                        println!("----------------------------------------------------------------");
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("[ERROR] Failed to parse library folders VDF: {}", e);
                    ExitCode::from(1)
                }
            }
        }
    }
}
