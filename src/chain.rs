use crate::profile::{GamescopeConfig, Profile, UpscaleFilter};
use std::collections::BTreeMap;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynthesizedCommand {
    pub env: BTreeMap<String, String>,
    pub program: String,
    pub args: Vec<String>,
}

impl SynthesizedCommand {
    pub fn full_argv(&self) -> Vec<String> {
        let mut argv = Vec::with_capacity(1 + self.args.len());
        argv.push(self.program.clone());
        argv.extend(self.args.iter().cloned());
        argv
    }

    pub fn to_command(&self) -> Command {
        let mut cmd = Command::new(&self.program);
        cmd.args(&self.args);
        for (k, v) in &self.env {
            cmd.env(k, v);
        }
        cmd
    }

    pub fn to_steam_launch_string(&self) -> String {
        let mut parts = Vec::new();

        // Environment variables
        for (k, v) in &self.env {
            parts.push(format!("{}={}", k, shell_escape(v)));
        }

        // Program and args
        parts.push(shell_escape(&self.program));
        for arg in &self.args {
            if arg == "%command%" {
                parts.push("%command%".to_string());
            } else {
                parts.push(shell_escape(arg));
            }
        }

        parts.join(" ")
    }

    pub fn to_shell_string(&self) -> String {
        let mut parts = Vec::new();

        for (k, v) in &self.env {
            parts.push(format!("{}={}", k, shell_escape(v)));
        }

        parts.push(shell_escape(&self.program));
        for arg in &self.args {
            parts.push(shell_escape(arg));
        }

        parts.join(" ")
    }
}

pub fn shell_escape(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }

    let is_safe = s.chars().all(|c| {
        c.is_ascii_alphanumeric()
            || c == '_'
            || c == '-'
            || c == '.'
            || c == '/'
            || c == ':'
            || c == '='
            || c == ','
            || c == '+'
    });

    if is_safe {
        s.to_string()
    } else {
        // Enclose in single quotes and escape any single quotes: ' -> '\''
        let mut escaped = String::from("'");
        for c in s.chars() {
            if c == '\'' {
                escaped.push_str("'\\''");
            } else {
                escaped.push(c);
            }
        }
        escaped.push('\'');
        escaped
    }
}

#[derive(Debug, Clone, Default)]
pub struct LaunchChainBuilder {
    env: BTreeMap<String, String>,
    use_gamemode: bool,
    use_mangohud: bool,
    gamescope_cfg: GamescopeConfig,
    nice_priority: Option<i8>,
    cpu_cores: Option<String>,
    target_command: Vec<String>,
}

impl LaunchChainBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_profile(profile: &Profile, target_cmd: &[String]) -> Self {
        let mut builder = Self::new();

        // 1. Ingest profile env
        for (k, v) in &profile.env {
            builder.env.insert(k.clone(), v.clone());
        }

        // 2. Compatibility flags injection
        if profile.compat.dxvk_async {
            builder.env.insert("DXVK_ASYNC".to_string(), "1".to_string());
        }
        if profile.compat.dxvk_nvapi {
            builder
                .env
                .insert("PROTON_ENABLE_NVAPI".to_string(), "1".to_string());
            builder
                .env
                .insert("DXVK_ENABLE_NVAPI".to_string(), "1".to_string());
        }
        if profile.compat.wine_sync {
            builder
                .env
                .insert("WINEFSYNC".to_string(), "1".to_string());
            builder
                .env
                .insert("WINEESYNC".to_string(), "1".to_string());
        }
        if let Some(vkd3d) = &profile.compat.vkd3d_config {
            builder
                .env
                .insert("VKD3D_CONFIG".to_string(), vkd3d.clone());
        }
        if let Some(radv) = &profile.compat.radv_perftest {
            builder
                .env
                .insert("RADV_PERFTEST".to_string(), radv.clone());
        }

        // 3. Wrappers
        builder.use_gamemode = profile.wrapper.gamemode;
        builder.use_mangohud = profile.wrapper.mangohud;
        builder.gamescope_cfg = profile.gamescope.clone();
        if profile.wrapper.gamescope {
            builder.gamescope_cfg.enabled = true;
        }

        // 4. CPU
        builder.nice_priority = profile.cpu.nice_priority;
        builder.cpu_cores = profile.cpu.cpu_cores.clone();

        // 5. Target command
        if target_cmd.is_empty() {
            builder.target_command = vec!["%command%".to_string()];
        } else {
            builder.target_command = target_cmd.to_vec();
        }

        builder
    }

    pub fn set_gamemode(&mut self, enabled: bool) -> &mut Self {
        self.use_gamemode = enabled;
        self
    }

    pub fn set_mangohud(&mut self, enabled: bool) -> &mut Self {
        self.use_mangohud = enabled;
        self
    }

    pub fn set_gamescope(&mut self, enabled: bool) -> &mut Self {
        self.gamescope_cfg.enabled = enabled;
        self
    }

    pub fn set_gamescope_resolution(
        &mut self,
        width: Option<u32>,
        height: Option<u32>,
    ) -> &mut Self {
        if let (Some(w), Some(h)) = (width, height) {
            self.gamescope_cfg.output_width = Some(w);
            self.gamescope_cfg.output_height = Some(h);
        }
        self
    }

    pub fn set_fps_limit(&mut self, fps: Option<u32>) -> &mut Self {
        self.gamescope_cfg.framerate_limit = fps;
        self
    }

    pub fn set_env(&mut self, key: &str, value: &str) -> &mut Self {
        self.env.insert(key.to_string(), value.to_string());
        self
    }

    pub fn set_target_command(&mut self, cmd: Vec<String>) -> &mut Self {
        self.target_command = cmd;
        self
    }

    pub fn build(self) -> SynthesizedCommand {
        let mut tokens: Vec<String> = Vec::new();

        // 1. CPU Core Pinning: taskset
        if let Some(cores) = &self.cpu_cores {
            tokens.push("taskset".to_string());
            tokens.push("-c".to_string());
            tokens.push(cores.clone());
        }

        // 2. CPU Scheduling Priority: nice
        if let Some(nice) = self.nice_priority {
            tokens.push("nice".to_string());
            tokens.push("-n".to_string());
            tokens.push(nice.to_string());
        }

        // 3. Feral GameMode: gamemoderun
        if self.use_gamemode {
            tokens.push("gamemoderun".to_string());
        }

        // 4. MangoHud Vulkan / OpenGL Overlay: mangohud
        if self.use_mangohud {
            tokens.push("mangohud".to_string());
        }

        // 5. Valve Gamescope Compositor
        if self.gamescope_cfg.enabled {
            tokens.push("gamescope".to_string());

            if let Some(w) = self.gamescope_cfg.nested_width {
                tokens.push("-w".to_string());
                tokens.push(w.to_string());
            }
            if let Some(h) = self.gamescope_cfg.nested_height {
                tokens.push("-h".to_string());
                tokens.push(h.to_string());
            }
            if let Some(w) = self.gamescope_cfg.output_width {
                tokens.push("-W".to_string());
                tokens.push(w.to_string());
            }
            if let Some(h) = self.gamescope_cfg.output_height {
                tokens.push("-H".to_string());
                tokens.push(h.to_string());
            }
            if let Some(r) = self.gamescope_cfg.refresh_rate {
                tokens.push("-r".to_string());
                tokens.push(r.to_string());
            }
            if let Some(limit) = self.gamescope_cfg.framerate_limit {
                tokens.push("--framerate-limit".to_string());
                tokens.push(limit.to_string());
            }
            if let Some(filter) = self.gamescope_cfg.upscale_filter {
                tokens.push("-F".to_string());
                let filter_str = match filter {
                    UpscaleFilter::Linear => "linear",
                    UpscaleFilter::Nearest => "nearest",
                    UpscaleFilter::Fsr => "fsr",
                    UpscaleFilter::Nis => "nis",
                    UpscaleFilter::Bicubic => "bicubic",
                };
                tokens.push(filter_str.to_string());
            }
            if let Some(sharpness) = self.gamescope_cfg.fsr_sharpness {
                tokens.push("--fsr-sharpness".to_string());
                tokens.push(sharpness.to_string());
            }
            if self.gamescope_cfg.fullscreen {
                tokens.push("-f".to_string());
            }
            if self.gamescope_cfg.borderless {
                tokens.push("-b".to_string());
            }
            if self.gamescope_cfg.force_grab_cursor {
                tokens.push("--force-grab-cursor".to_string());
            }
            if self.gamescope_cfg.hdr {
                tokens.push("--hdr-enabled".to_string());
            }
            if self.gamescope_cfg.tearing {
                tokens.push("--immediate-flips".to_string());
            }

            // Gamescope delimiter
            tokens.push("--".to_string());
        }

        // 6. Target command
        if self.target_command.is_empty() {
            tokens.push("%command%".to_string());
        } else {
            tokens.extend(self.target_command);
        }

        let program = tokens.remove(0);
        SynthesizedCommand {
            env: self.env,
            program,
            args: tokens,
        }
    }
}
