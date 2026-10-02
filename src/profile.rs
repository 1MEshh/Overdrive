use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProfileError {
    #[error("Failed to parse profile TOML: {0}")]
    TomlParse(String),

    #[error("Profile validation failed for '{field}': {reason}")]
    ValidationError {
        field: &'static str,
        reason: String,
    },

    #[error("Profile '{0}' not found in standard paths")]
    NotFound(String),

    #[error("I/O error loading profile: {0}")]
    Io(String),

    #[error("Circular inheritance detected when resolving profile '{0}'")]
    CircularInheritance(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum GpuVendor {
    #[default]
    Any,
    Amd,
    Nvidia,
    Intel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileMeta {
    #[serde(default = "default_profile_name")]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub target_gpu: GpuVendor,
    #[serde(default)]
    pub inherits: Option<String>,
}

fn default_profile_name() -> String {
    "default".to_string()
}

impl Default for ProfileMeta {
    fn default() -> Self {
        Self {
            name: default_profile_name(),
            description: String::new(),
            target_gpu: GpuVendor::Any,
            inherits: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpscaleFilter {
    Linear,
    Nearest,
    Fsr,
    Nis,
    Bicubic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WrapperConfig {
    #[serde(default)]
    pub gamemode: bool,
    #[serde(default)]
    pub mangohud: bool,
    #[serde(default)]
    pub gamescope: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GamescopeConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub nested_width: Option<u32>,
    #[serde(default)]
    pub nested_height: Option<u32>,
    #[serde(default)]
    pub output_width: Option<u32>,
    #[serde(default)]
    pub output_height: Option<u32>,
    #[serde(default)]
    pub refresh_rate: Option<u32>,
    #[serde(default)]
    pub framerate_limit: Option<u32>,
    #[serde(default)]
    pub upscale_filter: Option<UpscaleFilter>,
    #[serde(default)]
    pub fsr_sharpness: Option<u8>,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default)]
    pub borderless: bool,
    #[serde(default)]
    pub force_grab_cursor: bool,
    #[serde(default)]
    pub hdr: bool,
    #[serde(default)]
    pub tearing: bool,
}

impl Default for GamescopeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            nested_width: None,
            nested_height: None,
            output_width: None,
            output_height: None,
            refresh_rate: None,
            framerate_limit: None,
            upscale_filter: None,
            fsr_sharpness: None,
            fullscreen: false,
            borderless: false,
            force_grab_cursor: false,
            hdr: false,
            tearing: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CpuConfig {
    #[serde(default)]
    pub governor: Option<String>,
    #[serde(default)]
    pub energy_performance_preference: Option<String>,
    #[serde(default)]
    pub nice_priority: Option<i8>,
    #[serde(default)]
    pub cpu_cores: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CompatConfig {
    #[serde(default)]
    pub proton_dist: Option<String>,
    #[serde(default)]
    pub dxvk_nvapi: bool,
    #[serde(default)]
    pub dxvk_async: bool,
    #[serde(default)]
    pub wine_sync: bool,
    #[serde(default)]
    pub vkd3d_config: Option<String>,
    #[serde(default)]
    pub radv_perftest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Profile {
    #[serde(default)]
    pub profile: ProfileMeta,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub wrapper: WrapperConfig,
    #[serde(default)]
    pub gamescope: GamescopeConfig,
    #[serde(default)]
    pub cpu: CpuConfig,
    #[serde(default)]
    pub compat: CompatConfig,
}

impl Profile {
    pub fn from_toml(content: &str) -> Result<Self, ProfileError> {
        let p: Profile = toml::from_str(content).map_err(|e| ProfileError::TomlParse(e.to_string()))?;
        p.validate()?;
        Ok(p)
    }

    pub fn to_toml(&self) -> Result<String, ProfileError> {
        toml::to_string_pretty(self).map_err(|e| ProfileError::TomlParse(e.to_string()))
    }

    pub fn validate(&self) -> Result<(), ProfileError> {
        // Bounds checking on nice priority
        if let Some(nice) = self.cpu.nice_priority {
            if !(-20..=19).contains(&nice) {
                return Err(ProfileError::ValidationError {
                    field: "cpu.nice_priority",
                    reason: format!("Value {} outside allowed Linux nice range [-20, 19]", nice),
                });
            }
        }

        // Bounds checking on FSR sharpness
        if let Some(sharp) = self.gamescope.fsr_sharpness {
            if sharp > 20 {
                return Err(ProfileError::ValidationError {
                    field: "gamescope.fsr_sharpness",
                    reason: format!("Value {} exceeds maximum FSR sharpness of 20", sharp),
                });
            }
        }

        // Bounds checking on resolutions
        let check_res = |dim: Option<u32>, name: &'static str| -> Result<(), ProfileError> {
            if let Some(val) = dim {
                if val == 0 || val > 16384 {
                    return Err(ProfileError::ValidationError {
                        field: name,
                        reason: format!("Resolution {} outside realistic boundaries [1, 16384]", val),
                    });
                }
            }
            Ok(())
        };

        check_res(self.gamescope.nested_width, "gamescope.nested_width")?;
        check_res(self.gamescope.nested_height, "gamescope.nested_height")?;
        check_res(self.gamescope.output_width, "gamescope.output_width")?;
        check_res(self.gamescope.output_height, "gamescope.output_height")?;

        // Bounds checking on refresh rate
        if let Some(hz) = self.gamescope.refresh_rate {
            if hz == 0 || hz > 1000 {
                return Err(ProfileError::ValidationError {
                    field: "gamescope.refresh_rate",
                    reason: format!("Refresh rate {} outside realistic boundaries [1, 1000] Hz", hz),
                });
            }
        }

        // Bounds checking on framerate limit
        if let Some(limit) = self.gamescope.framerate_limit {
            if limit == 0 || limit > 1000 {
                return Err(ProfileError::ValidationError {
                    field: "gamescope.framerate_limit",
                    reason: format!("Framerate limit {} outside realistic boundaries [1, 1000] FPS", limit),
                });
            }
        }

        Ok(())
    }

    pub fn merge(&self, child: &Profile) -> Profile {
        let mut merged = self.clone();

        if !child.profile.name.is_empty() && child.profile.name != "default" {
            merged.profile.name = child.profile.name.clone();
        }
        if !child.profile.description.is_empty() {
            merged.profile.description = child.profile.description.clone();
        }
        if child.profile.target_gpu != GpuVendor::Any {
            merged.profile.target_gpu = child.profile.target_gpu;
        }

        // Merge environment variables: child overrides or adds to base
        for (k, v) in &child.env {
            merged.env.insert(k.clone(), v.clone());
        }

        // Wrappers: if true in either, activate (or override)
        if child.wrapper.gamemode {
            merged.wrapper.gamemode = true;
        }
        if child.wrapper.mangohud {
            merged.wrapper.mangohud = true;
        }
        if child.wrapper.gamescope {
            merged.wrapper.gamescope = true;
        }

        // Gamescope
        if child.gamescope.enabled {
            merged.gamescope.enabled = true;
        }
        if child.gamescope.nested_width.is_some() {
            merged.gamescope.nested_width = child.gamescope.nested_width;
        }
        if child.gamescope.nested_height.is_some() {
            merged.gamescope.nested_height = child.gamescope.nested_height;
        }
        if child.gamescope.output_width.is_some() {
            merged.gamescope.output_width = child.gamescope.output_width;
        }
        if child.gamescope.output_height.is_some() {
            merged.gamescope.output_height = child.gamescope.output_height;
        }
        if child.gamescope.refresh_rate.is_some() {
            merged.gamescope.refresh_rate = child.gamescope.refresh_rate;
        }
        if child.gamescope.framerate_limit.is_some() {
            merged.gamescope.framerate_limit = child.gamescope.framerate_limit;
        }
        if child.gamescope.upscale_filter.is_some() {
            merged.gamescope.upscale_filter = child.gamescope.upscale_filter;
        }
        if child.gamescope.fsr_sharpness.is_some() {
            merged.gamescope.fsr_sharpness = child.gamescope.fsr_sharpness;
        }
        if child.gamescope.fullscreen {
            merged.gamescope.fullscreen = true;
        }
        if child.gamescope.borderless {
            merged.gamescope.borderless = true;
        }
        if child.gamescope.force_grab_cursor {
            merged.gamescope.force_grab_cursor = true;
        }
        if child.gamescope.hdr {
            merged.gamescope.hdr = true;
        }
        if child.gamescope.tearing {
            merged.gamescope.tearing = true;
        }

        // CPU
        if child.cpu.governor.is_some() {
            merged.cpu.governor = child.cpu.governor.clone();
        }
        if child.cpu.energy_performance_preference.is_some() {
            merged.cpu.energy_performance_preference =
                child.cpu.energy_performance_preference.clone();
        }
        if child.cpu.nice_priority.is_some() {
            merged.cpu.nice_priority = child.cpu.nice_priority;
        }
        if child.cpu.cpu_cores.is_some() {
            merged.cpu.cpu_cores = child.cpu.cpu_cores.clone();
        }

        // Compat
        if child.compat.proton_dist.is_some() {
            merged.compat.proton_dist = child.compat.proton_dist.clone();
        }
        if child.compat.dxvk_nvapi {
            merged.compat.dxvk_nvapi = true;
        }
        if child.compat.dxvk_async {
            merged.compat.dxvk_async = true;
        }
        if child.compat.wine_sync {
            merged.compat.wine_sync = true;
        }
        if child.compat.vkd3d_config.is_some() {
            merged.compat.vkd3d_config = child.compat.vkd3d_config.clone();
        }
        if child.compat.radv_perftest.is_some() {
            merged.compat.radv_perftest = child.compat.radv_perftest.clone();
        }

        merged
    }
}

pub const BUILTIN_COMPETITIVE_FPS: &str = r#"[profile]
name = "competitive-fps"
description = "Maximized frame rates, ultra-low input latency, and immediate presentation for competitive esports"
target_gpu = "any"

[env]
RADV_PERFTEST = "gpl,nggc"
vblank_mode = "0"
MESA_VK_WSI_PRESENT_MODE = "immediate"
DXVK_ASYNC = "1"
PROTON_NO_ESYNC = "0"
PROTON_NO_FSYNC = "0"
__GL_GSYNC_ALLOWED = "1"
__GL_VRR_ALLOWED = "1"
PROTON_ENABLE_NVAPI = "1"
MESA_GL_THREAD = "true"
WINE_FULLSCREEN_FSR = "0"

[wrapper]
gamemode = true
gamescope = false
mangohud = true

[gamescope]
enabled = false
fullscreen = true
tearing = true
framerate_limit = 240

[cpu]
governor = "performance"
energy_performance_preference = "performance"
nice_priority = -10

[compat]
dxvk_async = true
wine_sync = true
radv_perftest = "gpl,nggc"
"#;

pub const BUILTIN_UNREAL_ENGINE_5: &str = r#"[profile]
name = "unreal-engine-5"
description = "DirectX 12 and Vulkan pipeline pre-compilation, VRAM defragmentation, and stutter mitigation for UE5"
target_gpu = "any"

[env]
VKD3D_CONFIG = "dxr11,dxr"
RADV_PERFTEST = "gpl"
DXVK_ASYNC = "1"
PROTON_ENABLE_NVAPI = "1"
VKD3D_SHADER_CACHE = "1"
WINE_LARGE_ADDRESS_AWARE = "1"
PROTON_HIDE_NVIDIA_GPU = "0"
PROTON_FORCE_LARGE_ADDRESS_AWARE = "1"

[wrapper]
gamemode = true
gamescope = false
mangohud = true

[gamescope]
enabled = false
fullscreen = true
borderless = true

[cpu]
governor = "performance"
energy_performance_preference = "performance"
nice_priority = -5

[compat]
dxvk_nvapi = true
dxvk_async = true
wine_sync = true
vkd3d_config = "dxr11,dxr"
radv_perftest = "gpl"
"#;

pub const BUILTIN_LOW_LATENCY_WAYLAND: &str = r#"[profile]
name = "low-latency-wayland"
description = "Native Wayland nested Gamescope compositor with FSR spatial upscaling and asynchronous flip support"
target_gpu = "any"

[env]
SDL_VIDEODRIVER = "wayland,x11"
GDK_BACKEND = "wayland,x11"
CLUTTER_BACKEND = "wayland"
QT_QPA_PLATFORM = "wayland;xcb"
MESA_VK_WSI_PRESENT_MODE = "immediate"
vblank_mode = "0"

[wrapper]
gamemode = true
gamescope = true
mangohud = true

[gamescope]
enabled = true
nested_width = 1920
nested_height = 1080
output_width = 2560
output_height = 1440
refresh_rate = 144
framerate_limit = 144
upscale_filter = "fsr"
fsr_sharpness = 5
fullscreen = true
borderless = false
force_grab_cursor = true
hdr = false
tearing = true

[cpu]
governor = "performance"
energy_performance_preference = "balance_performance"
nice_priority = -8

[compat]
wine_sync = true
"#;

pub fn get_builtin_profile(name: &str) -> Option<Profile> {
    match name {
        "competitive-fps" | "competitive" => Profile::from_toml(BUILTIN_COMPETITIVE_FPS).ok(),
        "unreal-engine-5" | "ue5" => Profile::from_toml(BUILTIN_UNREAL_ENGINE_5).ok(),
        "low-latency-wayland" | "wayland" => {
            Profile::from_toml(BUILTIN_LOW_LATENCY_WAYLAND).ok()
        }
        _ => None,
    }
}

pub fn list_builtin_profiles() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "competitive-fps",
            "Maximized frame rates, ultra-low input latency, and immediate presentation",
        ),
        (
            "unreal-engine-5",
            "DirectX 12/VKD3D pipeline pre-compilation and stutter mitigation",
        ),
        (
            "low-latency-wayland",
            "Native Wayland Gamescope compositor with FSR upscaling and direct scanout",
        ),
    ]
}

pub fn resolve_profile(
    name_or_appid: &str,
    custom_search_dirs: &[PathBuf],
) -> Result<Profile, ProfileError> {
    // 1. Check custom search directories
    let filename = if name_or_appid.ends_with(".toml") {
        name_or_appid.to_string()
    } else {
        format!("{}.toml", name_or_appid)
    };

    for dir in custom_search_dirs {
        let p = dir.join(&filename);
        if p.exists() {
            let content = std::fs::read_to_string(&p)
                .map_err(|e| ProfileError::Io(format!("Failed to read {}: {}", p.display(), e)))?;
            let loaded = Profile::from_toml(&content)?;
            return resolve_inheritance(loaded, custom_search_dirs, 0);
        }
    }

    // 2. Check XDG config directory / standard paths
    if let Ok(config_home) = std::env::var("OVERDRIVE_CONFIG_DIR") {
        let p = PathBuf::from(config_home).join("profiles").join(&filename);
        if p.exists() {
            let content = std::fs::read_to_string(&p)
                .map_err(|e| ProfileError::Io(format!("Failed to read {}: {}", p.display(), e)))?;
            let loaded = Profile::from_toml(&content)?;
            return resolve_inheritance(loaded, custom_search_dirs, 0);
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home)
            .join(".config")
            .join("overdrive")
            .join("profiles")
            .join(&filename);
        if p.exists() {
            let content = std::fs::read_to_string(&p)
                .map_err(|e| ProfileError::Io(format!("Failed to read {}: {}", p.display(), e)))?;
            let loaded = Profile::from_toml(&content)?;
            return resolve_inheritance(loaded, custom_search_dirs, 0);
        }
    }

    // 3. Check system directory
    let sys_path = PathBuf::from("/etc/overdrive/profiles").join(&filename);
    if sys_path.exists() {
        let content = std::fs::read_to_string(&sys_path).map_err(|e| {
            ProfileError::Io(format!("Failed to read {}: {}", sys_path.display(), e))
        })?;
        let loaded = Profile::from_toml(&content)?;
        return resolve_inheritance(loaded, custom_search_dirs, 0);
    }

    // 4. Check builtins
    let clean_name = name_or_appid.trim_end_matches(".toml");
    if let Some(builtin) = get_builtin_profile(clean_name) {
        return resolve_inheritance(builtin, custom_search_dirs, 0);
    }

    Err(ProfileError::NotFound(name_or_appid.to_string()))
}

fn resolve_inheritance(
    profile: Profile,
    search_dirs: &[PathBuf],
    depth: usize,
) -> Result<Profile, ProfileError> {
    if depth > 8 {
        return Err(ProfileError::CircularInheritance(profile.profile.name));
    }

    if let Some(parent_name) = &profile.profile.inherits {
        let parent = resolve_profile(parent_name, search_dirs)?;
        let resolved_parent = resolve_inheritance(parent, search_dirs, depth + 1)?;
        Ok(resolved_parent.merge(&profile))
    } else {
        Ok(profile)
    }
}
