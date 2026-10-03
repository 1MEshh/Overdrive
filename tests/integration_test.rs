use overdrive::chain::{shell_escape, LaunchChainBuilder};
use overdrive::doctor::{run_diagnostics, CheckStatus};
use overdrive::profile::{
    get_builtin_profile, GamescopeConfig, GpuVendor, Profile,
    ProfileError, UpscaleFilter,
};
use overdrive::telemetry::{
    calculate_metrics, parse_frametimes_csv, TelemetryError,
};
use overdrive::vdf::{
    parse_app_manifest, parse_library_folders, parse_vdf, VdfError,
};

#[test]
fn test_vdf_basic_key_value() {
    let input = r#"
    "AppState"
    {
        "appid" "730"
        "name" "Counter-Strike 2"
        "installdir" "Counter-Strike Global Offensive"
    }
    "#;

    let parsed = parse_vdf(input).expect("VDF parsing failed");
    let state = parsed.get("AppState").expect("AppState not found");

    assert_eq!(state.get_str("appid"), Some("730"));
    assert_eq!(state.get_u32("appid"), Some(730));
    assert_eq!(state.get_str("name"), Some("Counter-Strike 2"));
    assert_eq!(
        state.get_str("installdir"),
        Some("Counter-Strike Global Offensive")
    );
}

#[test]
fn test_vdf_nested_objects() {
    let input = r#"
    "root"
    {
        "level1"
        {
            "level2"
            {
                "leaf_key" "leaf_value"
            }
        }
    }
    "#;

    let parsed = parse_vdf(input).expect("Nested VDF parsing failed");
    let leaf = parsed
        .get("root")
        .and_then(|r| r.get("level1"))
        .and_then(|l1| l1.get("level2"))
        .and_then(|l2| l2.get_str("leaf_key"));

    assert_eq!(leaf, Some("leaf_value"));
}

#[test]
fn test_vdf_comments_and_whitespace() {
    let input = r#"
    // Header comment
    "Library" // Inline comment
    {
        // Internal comment
        "path" "/mnt/storage/games" // Path comment
        "enabled" "1"
    }
    "#;

    let parsed = parse_vdf(input).expect("Commented VDF failed");
    let lib = parsed.get("Library").expect("Library block missing");
    assert_eq!(lib.get_str("path"), Some("/mnt/storage/games"));
    assert_eq!(lib.get_bool("enabled"), Some(true));
}

#[test]
fn test_vdf_escaped_characters() {
    let input = r#"
    "Key" "Value with \"quotes\" and \\slash and \t tab"
    "#;

    let parsed = parse_vdf(input).expect("Escaped VDF failed");
    assert_eq!(
        parsed.get_str("Key"),
        Some("Value with \"quotes\" and \\slash and \t tab")
    );
}

#[test]
fn test_vdf_unquoted_tokens() {
    let input = r#"
    AppState
    {
        appid 570
        Universe 1
        name Dota2
    }
    "#;

    let parsed = parse_vdf(input).expect("Unquoted VDF parsing failed");
    let state = parsed.get("AppState").expect("AppState missing");
    assert_eq!(state.get_u32("appid"), Some(570));
    assert_eq!(state.get_str("name"), Some("Dota2"));
}

#[test]
fn test_vdf_appmanifest_parser() {
    let input = r#"
    "AppState"
    {
        "appid" "1172470"
        "Universe" "1"
        "name" "Apex Legends"
        "installdir" "Apex Legends"
        "LastUpdated" "1690000000"
        "SizeOnDisk" "64424509440"
        "buildid" "12345678"
        "BytesToDownload" "0"
        "BytesDownloaded" "0"
        "CompatToolMapping"
        {
            "0"
            {
                "name" "proton_experimental"
                "config" ""
                "priority" "250"
            }
        }
    }
    "#;

    let manifest = parse_app_manifest(input).expect("Manifest parse failed");
    assert_eq!(manifest.appid, 1172470);
    assert_eq!(manifest.name, "Apex Legends");
    assert_eq!(manifest.installdir, "Apex Legends");
    assert_eq!(manifest.size_on_disk, 64424509440);
    assert_eq!(manifest.build_id, 12345678);
    assert_eq!(manifest.compat_tool, Some("proton_experimental".to_string()));
}

#[test]
fn test_vdf_libraryfolders_parser() {
    let input = r#"
    "libraryfolders"
    {
        "0"
        {
            "path" "/data/steam_main"
            "label" "NVMe Fast"
            "totalsize" "1000204886016"
            "apps"
            {
                "730" "35000000000"
                "570" "45000000000"
            }
        }
        "1"
        {
            "path" "/data/steam_secondary"
            "label" "HDD Storage"
            "totalsize" "4000787030016"
            "apps"
            {
                "1172470" "65000000000"
            }
        }
    }
    "#;

    let libs = parse_library_folders(input).expect("Library folders parse failed");
    assert_eq!(libs.len(), 2);
    assert_eq!(libs[0].path.to_str(), Some("/data/steam_main"));
    assert_eq!(libs[0].label, "NVMe Fast");
    assert!(libs[0].apps.contains(&730));
    assert!(libs[0].apps.contains(&570));

    assert_eq!(libs[1].path.to_str(), Some("/data/steam_secondary"));
    assert_eq!(libs[1].apps, vec![1172470]);
}

#[test]
fn test_vdf_malformed_syntax_error() {
    let input = r#"
    "AppState"
    {
        "appid" "730"
        // missing closing brace
    "#;

    let err = parse_vdf(input).unwrap_err();
    match err {
        VdfError::UnexpectedEof { .. } => {}
        other => panic!("Expected UnexpectedEof, got {:?}", other),
    }
}

#[test]
fn test_vdf_unterminated_string_error() {
    let input = r#"
    "AppState"
    {
        "appid" "730
    }
    "#;

    let err = parse_vdf(input).unwrap_err();
    match err {
        VdfError::UnterminatedString { .. } => {}
        other => panic!("Expected UnterminatedString, got {:?}", other),
    }
}

#[test]
fn test_vdf_recursion_depth_limit() {
    // Generate pathological nesting > 64 levels
    let mut pathological = String::new();
    for i in 0..70 {
        pathological.push_str(&format!("\"node{}\" {{ ", i));
    }
    pathological.push_str("\"leaf\" \"val\"");
    for _ in 0..70 {
        pathological.push_str(" }");
    }

    let err = parse_vdf(&pathological).unwrap_err();
    match err {
        VdfError::RecursionLimitExceeded(limit) => assert_eq!(limit, 64),
        other => panic!("Expected RecursionLimitExceeded, got {:?}", other),
    }
}

#[test]
fn test_profile_builtin_loading() {
    let comp = get_builtin_profile("competitive-fps").expect("competitive-fps missing");
    assert_eq!(comp.profile.name, "competitive-fps");
    assert!(comp.wrapper.gamemode);
    assert!(comp.wrapper.mangohud);
    assert_eq!(comp.env.get("MESA_VK_WSI_PRESENT_MODE").map(|s| s.as_str()), Some("immediate"));

    let ue5 = get_builtin_profile("unreal-engine-5").expect("unreal-engine-5 missing");
    assert_eq!(ue5.profile.name, "unreal-engine-5");
    assert_eq!(ue5.env.get("VKD3D_CONFIG").map(|s| s.as_str()), Some("dxr11,dxr"));

    let wayland = get_builtin_profile("low-latency-wayland").expect("low-latency-wayland missing");
    assert_eq!(wayland.profile.name, "low-latency-wayland");
    assert!(wayland.gamescope.enabled);
    assert_eq!(wayland.gamescope.upscale_filter, Some(UpscaleFilter::Fsr));
}

#[test]
fn test_profile_toml_roundtrip() {
    let comp = get_builtin_profile("competitive-fps").unwrap();
    let toml_str = comp.to_toml().expect("Serialization failed");
    let deserialized = Profile::from_toml(&toml_str).expect("Deserialization failed");
    assert_eq!(comp, deserialized);
}

#[test]
fn test_profile_merging_and_overrides() {
    let base = get_builtin_profile("competitive-fps").unwrap();

    let child_toml = r#"
    [profile]
    name = "cs2-custom"
    description = "Customized CS2 competitive profile"
    target_gpu = "amd"

    [env]
    CUSTOM_FLAG = "1"
    RADV_PERFTEST = "gpl,sam"

    [gamescope]
    enabled = true
    output_width = 1920
    output_height = 1080
    framerate_limit = 360
    "#;

    let child = Profile::from_toml(child_toml).expect("Child profile invalid");
    let merged = base.merge(&child);

    assert_eq!(merged.profile.name, "cs2-custom");
    assert_eq!(merged.profile.target_gpu, GpuVendor::Amd);
    assert_eq!(merged.env.get("CUSTOM_FLAG").map(|s| s.as_str()), Some("1"));
    assert_eq!(
        merged.env.get("RADV_PERFTEST").map(|s| s.as_str()),
        Some("gpl,sam")
    );
    assert_eq!(
        merged.env.get("MESA_VK_WSI_PRESENT_MODE").map(|s| s.as_str()),
        Some("immediate")
    );
    assert!(merged.gamescope.enabled);
    assert_eq!(merged.gamescope.output_width, Some(1920));
    assert_eq!(merged.gamescope.framerate_limit, Some(360));
}

#[test]
fn test_profile_bounds_checking_nice_priority() {
    let invalid_toml = r#"
    [cpu]
    nice_priority = -25
    "#;

    let res = Profile::from_toml(invalid_toml);
    assert!(res.is_err());
    match res.unwrap_err() {
        ProfileError::ValidationError { field, .. } => {
            assert_eq!(field, "cpu.nice_priority");
        }
        other => panic!("Expected ValidationError, got {:?}", other),
    }
}

#[test]
fn test_profile_bounds_checking_fsr_sharpness() {
    let invalid_toml = r#"
    [gamescope]
    fsr_sharpness = 25
    "#;

    let res = Profile::from_toml(invalid_toml);
    assert!(res.is_err());
    match res.unwrap_err() {
        ProfileError::ValidationError { field, .. } => {
            assert_eq!(field, "gamescope.fsr_sharpness");
        }
        other => panic!("Expected ValidationError, got {:?}", other),
    }
}

#[test]
fn test_profile_bounds_checking_resolution() {
    let invalid_toml = r#"
    [gamescope]
    output_width = 0
    "#;

    let res = Profile::from_toml(invalid_toml);
    assert!(res.is_err());
    match res.unwrap_err() {
        ProfileError::ValidationError { field, .. } => {
            assert_eq!(field, "gamescope.output_width");
        }
        other => panic!("Expected ValidationError, got {:?}", other),
    }
}

#[test]
fn test_profile_bounds_checking_refresh_rate() {
    let invalid_toml = r#"
    [gamescope]
    refresh_rate = 1200
    "#;

    let res = Profile::from_toml(invalid_toml);
    assert!(res.is_err());
    match res.unwrap_err() {
        ProfileError::ValidationError { field, .. } => {
            assert_eq!(field, "gamescope.refresh_rate");
        }
        other => panic!("Expected ValidationError, got {:?}", other),
    }
}

#[test]
fn test_chain_synthesis_full_wrapper() {
    let mut profile = get_builtin_profile("low-latency-wayland").unwrap();
    profile.cpu.cpu_cores = Some("0-7".to_string());
    profile.cpu.nice_priority = Some(-10);

    let cmd = vec![
        "/usr/games/counterstrike".to_string(),
        "-novid".to_string(),
        "-freq".to_string(),
        "144".to_string(),
    ];

    let synth = LaunchChainBuilder::from_profile(&profile, &cmd).build();

    let argv = synth.full_argv();
    assert_eq!(argv[0], "taskset");
    assert_eq!(argv[1], "-c");
    assert_eq!(argv[2], "0-7");
    assert_eq!(argv[3], "nice");
    assert_eq!(argv[4], "-n");
    assert_eq!(argv[5], "-10");
    assert_eq!(argv[6], "gamemoderun");
    assert_eq!(argv[7], "mangohud");
    assert_eq!(argv[8], "gamescope");

    // Check gamescope arguments
    assert!(argv.contains(&"-w".to_string()));
    assert!(argv.contains(&"1920".to_string()));
    assert!(argv.contains(&"-W".to_string()));
    assert!(argv.contains(&"2560".to_string()));
    assert!(argv.contains(&"-r".to_string()));
    assert!(argv.contains(&"144".to_string()));
    assert!(argv.contains(&"-F".to_string()));
    assert!(argv.contains(&"fsr".to_string()));
    assert!(argv.contains(&"--".to_string()));

    // Target command at the end
    let separator_idx = argv.iter().position(|a| a == "--").unwrap();
    assert_eq!(argv[separator_idx + 1], "/usr/games/counterstrike");
    assert_eq!(argv[separator_idx + 2], "-novid");
    assert_eq!(argv[separator_idx + 3], "-freq");
    assert_eq!(argv[separator_idx + 4], "144");
}

#[test]
fn test_chain_gamescope_flags() {
    let mut profile = Profile::default();
    profile.gamescope = GamescopeConfig {
        enabled: true,
        nested_width: Some(1280),
        nested_height: Some(720),
        output_width: Some(1920),
        output_height: Some(1080),
        refresh_rate: Some(240),
        framerate_limit: Some(240),
        upscale_filter: Some(UpscaleFilter::Nis),
        fsr_sharpness: Some(10),
        fullscreen: true,
        borderless: true,
        force_grab_cursor: true,
        hdr: true,
        tearing: true,
    };

    let synth = LaunchChainBuilder::from_profile(&profile, &["mygame".to_string()]).build();
    let argv = synth.full_argv();

    assert_eq!(argv[0], "gamescope");
    assert!(argv.contains(&"-w".to_string()));
    assert!(argv.contains(&"1280".to_string()));
    assert!(argv.contains(&"-F".to_string()));
    assert!(argv.contains(&"nis".to_string()));
    assert!(argv.contains(&"--fsr-sharpness".to_string()));
    assert!(argv.contains(&"10".to_string()));
    assert!(argv.contains(&"-f".to_string()));
    assert!(argv.contains(&"-b".to_string()));
    assert!(argv.contains(&"--force-grab-cursor".to_string()));
    assert!(argv.contains(&"--hdr-enabled".to_string()));
    assert!(argv.contains(&"--immediate-flips".to_string()));
    assert!(argv.contains(&"--framerate-limit".to_string()));
    assert!(argv.contains(&"240".to_string()));
}

#[test]
fn test_chain_steam_launch_string_formatting() {
    let profile = get_builtin_profile("competitive-fps").unwrap();
    let synth = LaunchChainBuilder::from_profile(&profile, &["%command%".to_string()]).build();

    let steam_str = synth.to_steam_launch_string();
    assert!(steam_str.contains("RADV_PERFTEST=gpl,nggc"));
    assert!(steam_str.contains("gamemoderun"));
    assert!(steam_str.contains("mangohud"));
    assert!(steam_str.ends_with("%command%"));
}

#[test]
fn test_chain_shell_escaping() {
    assert_eq!(shell_escape("normal_arg"), "normal_arg");
    assert_eq!(shell_escape("path/with-dash.bin"), "path/with-dash.bin");
    assert_eq!(shell_escape("spaced argument"), "'spaced argument'");
    assert_eq!(shell_escape("it's cool"), "'it'\\''s cool'");
    assert_eq!(shell_escape(""), "''");
}

#[test]
fn test_chain_command_expansion() {
    let profile = Profile::default();
    let cmd = vec!["custom_runner".to_string(), "arg1".to_string(), "arg2".to_string()];
    let synth = LaunchChainBuilder::from_profile(&profile, &cmd).build();

    assert_eq!(synth.program, "custom_runner");
    assert_eq!(synth.args, vec!["arg1", "arg2"]);
}

#[test]
fn test_telemetry_mangohud_csv_ns() {
    let csv = r#"
    gpu_time,cpu_time,frametime_ns
    1200000,1400000,6944444
    1250000,1350000,7000000
    1210000,1390000,6850000
    1220000,1380000,7100000
    "#;

    let fts = parse_frametimes_csv(csv).expect("Failed to parse MangoHud CSV");
    assert_eq!(fts.len(), 4);
    // Nanoseconds 6944444 -> 6.944444 ms
    assert!((fts[0] - 6.944444).abs() < 1e-4);
    assert!((fts[1] - 7.000000).abs() < 1e-4);
}

#[test]
fn test_telemetry_csv_ms_column() {
    let csv = r#"
    # Benchmark capture
    Frametime (ms),FPS
    16.667,60.0
    16.700,59.88
    16.650,60.06
    16.680,59.95
    "#;

    let fts = parse_frametimes_csv(csv).expect("Failed to parse ms CSV");
    assert_eq!(fts.len(), 4);
    assert!((fts[0] - 16.667).abs() < 1e-3);
}

#[test]
fn test_telemetry_average_fps_math() {
    // 4 frames of 10.0ms each: Total duration 40ms = 0.04s. Average FPS = 4 / 0.04 = 100 FPS
    let fts = vec![10.0, 10.0, 10.0, 10.0];
    let metrics = calculate_metrics(&fts).expect("Calculation failed");

    assert_eq!(metrics.total_frames, 4);
    assert!((metrics.duration_seconds - 0.04).abs() < 1e-6);
    assert!((metrics.avg_fps - 100.0).abs() < 1e-6);
    assert!((metrics.mean_frametime_ms - 10.0).abs() < 1e-6);
    assert!((metrics.std_dev_ms - 0.0).abs() < 1e-6);
    assert!((metrics.rmssd_jitter_ms - 0.0).abs() < 1e-6);
    assert_eq!(metrics.stutter_frames_count, 0);
}

#[test]
fn test_telemetry_percentiles_math() {
    // 100 frames: 90 frames of 10ms (100 FPS), 10 frames of 50ms (20 FPS)
    let mut fts = vec![10.0; 90];
    fts.extend(vec![50.0; 10]);

    let metrics = calculate_metrics(&fts).expect("Calculation failed");
    assert_eq!(metrics.total_frames, 100);
    // 99th percentile will be 50ms (20 FPS)
    assert!((metrics.low_1pct_fps - 20.0).abs() < 1e-4);
    assert!((metrics.min_fps - 20.0).abs() < 1e-6);
    assert!((metrics.max_fps - 100.0).abs() < 1e-6);
}

#[test]
fn test_telemetry_duration_weighted_lows() {
    // 1000 frames total: 990 frames of 10ms (total 9900ms), 10 frames of 100ms (total 1000ms).
    // Total duration = 10900ms.
    // 1% duration = 109ms.
    // Worst frames are 100ms each. First 2 worst frames accumulate 200ms >= 109ms.
    // Average of these worst frames = 100ms -> 10.0 FPS.
    let mut fts = vec![10.0; 990];
    fts.extend(vec![100.0; 10]);

    let metrics = calculate_metrics(&fts).expect("Calculation failed");
    assert!((metrics.dur_weighted_1pct_fps - 10.0).abs() < 1e-3);
}

#[test]
fn test_telemetry_frame_pacing_jitter_rmssd() {
    // Alternating frametimes: 10.0, 20.0, 10.0, 20.0, 10.0
    // Successive deltas: 10, -10, 10, -10
    // Squared deltas: 100, 100, 100, 100
    // Mean of squared deltas = 400 / 4 = 100
    // RMSSD = sqrt(100) = 10.0
    let fts = vec![10.0, 20.0, 10.0, 20.0, 10.0];
    let metrics = calculate_metrics(&fts).expect("Calculation failed");
    assert!((metrics.rmssd_jitter_ms - 10.0).abs() < 1e-6);
}

#[test]
fn test_telemetry_stutter_index() {
    // 90 frames of 10ms (900ms total, median is 10ms).
    // 10 frames of 20ms (200ms total). Threshold = 1.5 * 10 = 15ms.
    // The 10 frames of 20ms exceed 15ms.
    // Total time in stutter = 200ms. Total benchmark duration = 1100ms.
    // Stutter index = (200 / 1100) * 100% = 18.1818%
    let mut fts = vec![10.0; 90];
    fts.extend(vec![20.0; 10]);

    let metrics = calculate_metrics(&fts).expect("Calculation failed");
    assert_eq!(metrics.stutter_frames_count, 10);
    assert!((metrics.stutter_index_pct - 18.1818).abs() < 0.1);
}

#[test]
fn test_telemetry_insufficient_samples_error() {
    let empty: Vec<f64> = vec![];
    assert_eq!(
        calculate_metrics(&empty).unwrap_err(),
        TelemetryError::InsufficientSamples
    );

    let single = vec![16.6];
    assert_eq!(
        calculate_metrics(&single).unwrap_err(),
        TelemetryError::InsufficientSamples
    );
}

#[test]
fn test_doctor_report_generation() {
    let report = run_diagnostics();
    assert!(
        report.overall_status == CheckStatus::Pass
            || report.overall_status == CheckStatus::Warn
            || report.overall_status == CheckStatus::Fail
    );
    assert!(!report.checks.is_empty());
    let ascii = report.to_ascii_report();
    assert!(ascii.contains("OVERDRIVE LINUX GAMING ENVIRONMENT DOCTOR"));
    assert!(ascii.contains("DIAGNOSTIC SYSTEM CHECKS"));
}

#[test]
fn test_security_zero_leak() {
    // Verify that the codebase contains no hardcoded absolute user home paths
    // Dynamically retrieve user to avoid hardcoding any username in the test source!
    if let Ok(user) = std::env::var("USER") {
        if !user.is_empty() && user != "root" && user != "runner" {
            let user_leak = format!("/home/{}", user);
            // Verify our own built binary string and profiles do not embed this path
            let profile_content = overdrive::profile::BUILTIN_COMPETITIVE_FPS;
            assert!(
                !profile_content.contains(&user_leak),
                "Built-in profile leaked user path"
            );
        }
    }
}
