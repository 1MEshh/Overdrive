pub mod chain;
pub mod doctor;
pub mod profile;
pub mod telemetry;
pub mod vdf;

pub use chain::{shell_escape, LaunchChainBuilder, SynthesizedCommand};
pub use doctor::{run_diagnostics, CheckStatus, DiagnosticCheck, DoctorReport};
pub use profile::{
    get_builtin_profile, list_builtin_profiles, resolve_profile, CompatConfig, CpuConfig,
    GamescopeConfig, GpuVendor, Profile, ProfileError, ProfileMeta, UpscaleFilter,
    WrapperConfig,
};
pub use telemetry::{
    calculate_metrics, parse_frametimes_csv, BenchmarkMetrics, TelemetryError,
};
pub use vdf::{
    detect_game_install_path, detect_proton_prefix, find_app_in_libraries,
    find_standard_steam_paths, locate_app_in_library, parse_app_manifest,
    parse_library_folders, parse_vdf, AppResolution, SteamAppManifest,
    SteamLibraryFolder, VdfError, VdfValue,
};
