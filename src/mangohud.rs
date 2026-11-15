use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum MangoHudLayout {
    #[default]
    CompactHorizontal,
    MinimalFps,
    FullTelemetry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum MangoHudPosition {
    TopLeft,
    #[default]
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl MangoHudPosition {
    pub fn as_str(&self) -> &'static str {
        match self {
            MangoHudPosition::TopLeft => "top-left",
            MangoHudPosition::TopCenter => "top-center",
            MangoHudPosition::TopRight => "top-right",
            MangoHudPosition::BottomLeft => "bottom-left",
            MangoHudPosition::BottomCenter => "bottom-center",
            MangoHudPosition::BottomRight => "bottom-right",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MangoHudPreset {
    pub layout: MangoHudLayout,
    pub horizontal: bool,
    pub horizontal_stretch: bool,
    pub position: MangoHudPosition,
    pub background_alpha: f32,
    pub font_size: u16,
    pub fps_limit: Option<u32>,
    pub show_fps: bool,
    pub show_frametime: bool,
    pub show_frametime_graph: bool,
    pub show_cpu_stats: bool,
    pub show_gpu_stats: bool,
    pub show_vram: bool,
    pub show_ram: bool,
    pub show_gamemode: bool,
    pub show_wine: bool,
    pub no_display: bool,
    pub toggle_hud_key: String,
}

impl Default for MangoHudPreset {
    fn default() -> Self {
        Self::compact_hyprland()
    }
}

impl MangoHudPreset {
    /// Compact single-row horizontal bar preset suited for Hyprland + Waybar layouts.
    pub fn compact_hyprland() -> Self {
        Self {
            layout: MangoHudLayout::CompactHorizontal,
            horizontal: true,
            horizontal_stretch: false,
            position: MangoHudPosition::TopCenter,
            background_alpha: 0.35,
            font_size: 18,
            fps_limit: None,
            show_fps: true,
            show_frametime: true,
            show_frametime_graph: false,
            show_cpu_stats: true,
            show_gpu_stats: true,
            show_vram: true,
            show_ram: false,
            show_gamemode: true,
            show_wine: false,
            no_display: false,
            toggle_hud_key: "Shift_R+F12".to_string(),
        }
    }

    /// Minimal FPS and frametime readout for distraction-free competitive play.
    pub fn minimal_fps() -> Self {
        Self {
            layout: MangoHudLayout::MinimalFps,
            horizontal: true,
            horizontal_stretch: false,
            position: MangoHudPosition::TopLeft,
            background_alpha: 0.25,
            font_size: 16,
            fps_limit: None,
            show_fps: true,
            show_frametime: true,
            show_frametime_graph: false,
            show_cpu_stats: false,
            show_gpu_stats: false,
            show_vram: false,
            show_ram: false,
            show_gamemode: false,
            show_wine: false,
            no_display: false,
            toggle_hud_key: "Shift_R+F12".to_string(),
        }
    }

    /// Full diagnostic overlay including frametime graph, CPU/GPU/VRAM/RAM, and Wine/Proton tags.
    pub fn full_telemetry() -> Self {
        Self {
            layout: MangoHudLayout::FullTelemetry,
            horizontal: false,
            horizontal_stretch: false,
            position: MangoHudPosition::TopLeft,
            background_alpha: 0.50,
            font_size: 20,
            fps_limit: None,
            show_fps: true,
            show_frametime: true,
            show_frametime_graph: true,
            show_cpu_stats: true,
            show_gpu_stats: true,
            show_vram: true,
            show_ram: true,
            show_gamemode: true,
            show_wine: true,
            no_display: false,
            toggle_hud_key: "Shift_R+F12".to_string(),
        }
    }

    pub fn from_layout(layout: MangoHudLayout) -> Self {
        match layout {
            MangoHudLayout::CompactHorizontal => Self::compact_hyprland(),
            MangoHudLayout::MinimalFps => Self::minimal_fps(),
            MangoHudLayout::FullTelemetry => Self::full_telemetry(),
        }
    }

    pub fn with_fps_limit(mut self, fps_limit: Option<u32>) -> Self {
        self.fps_limit = fps_limit;
        self
    }

    pub fn with_position(mut self, position: MangoHudPosition) -> Self {
        self.position = position;
        self
    }

    /// Generates ordered MangoHud configuration directives.
    pub fn to_config_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push("legacy_layout=0".to_string());

        if self.horizontal {
            lines.push("horizontal".to_string());
            if !self.horizontal_stretch {
                lines.push("horizontal_stretch=0".to_string());
            }
        }

        lines.push(format!("position={}", self.position.as_str()));
        let alpha = self.background_alpha.clamp(0.0, 1.0);
        lines.push(format!("background_alpha={:.2}", alpha));
        lines.push(format!("font_size={}", self.font_size.max(8)));

        if self.show_fps {
            lines.push("fps".to_string());
        }
        if self.show_frametime {
            lines.push("frametime".to_string());
        }
        if self.show_frametime_graph {
            lines.push("frame_timing=1".to_string());
        } else {
            lines.push("frame_timing=0".to_string());
        }
        if self.show_cpu_stats {
            lines.push("cpu_stats".to_string());
        }
        if self.show_gpu_stats {
            lines.push("gpu_stats".to_string());
        }
        if self.show_vram {
            lines.push("vram".to_string());
        }
        if self.show_ram {
            lines.push("ram".to_string());
        }
        if self.show_gamemode {
            lines.push("gamemode".to_string());
        }
        if self.show_wine {
            lines.push("wine".to_string());
        }
        if let Some(limit) = self.fps_limit {
            if limit > 0 {
                lines.push(format!("fps_limit={}", limit));
            }
        }
        if self.no_display {
            lines.push("no_display".to_string());
        }
        if !self.toggle_hud_key.is_empty() {
            lines.push(format!("toggle_hud={}", self.toggle_hud_key));
        }

        lines
    }

    /// Serializes the preset to a newline-delimited MangoHud.conf payload.
    pub fn to_config_file(&self) -> String {
        let mut out = String::from("# Overdrive MangoHud preset\n");
        out.push_str(&self.to_config_lines().join("\n"));
        out.push('\n');
        out
    }

    /// Serializes the preset as a comma-separated MANGOHUD_CONFIG environment string.
    pub fn to_mangohud_config_env(&self) -> String {
        self.to_config_lines().join(",")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::LaunchChainBuilder;

    #[test]
    fn compact_hyprland_config_serialization() {
        let preset = MangoHudPreset::compact_hyprland();
        let conf = preset.to_config_file();

        assert!(conf.starts_with("# Overdrive MangoHud preset\n"));
        assert!(conf.contains("\nlegacy_layout=0\n"));
        assert!(conf.contains("\nhorizontal\n"));
        assert!(conf.contains("\nhorizontal_stretch=0\n"));
        assert!(conf.contains("\nposition=top-center\n"));
        assert!(conf.contains("\nbackground_alpha=0.35\n"));
        assert!(conf.contains("\nframe_timing=0\n"));
        assert!(conf.contains("\ncpu_stats\n"));
        assert!(conf.contains("\ngpu_stats\n"));
        assert!(conf.contains("\ngamemode\n"));
        assert!(!conf.contains("\nno_display\n"));
    }

    #[test]
    fn mangohud_env_string_and_bounds_clamping() {
        let mut preset = MangoHudPreset::minimal_fps()
            .with_position(MangoHudPosition::BottomRight)
            .with_fps_limit(Some(144));
        preset.background_alpha = 1.8;
        preset.font_size = 4;
        preset.no_display = true;

        let env_str = preset.to_mangohud_config_env();
        let parts: Vec<&str> = env_str.split(',').collect();

        assert!(parts.contains(&"horizontal"));
        assert!(parts.contains(&"position=bottom-right"));
        assert!(parts.contains(&"background_alpha=1.00"));
        assert!(parts.contains(&"font_size=8"));
        assert!(parts.contains(&"fps_limit=144"));
        assert!(parts.contains(&"no_display"));

        let mut builder = LaunchChainBuilder::new();
        builder
            .set_mangohud_preset(&preset)
            .set_target_command(vec!["./game".to_string()]);
        let synth = builder.build();

        assert_eq!(synth.program, "mangohud");
        assert_eq!(
            synth.env.get("MANGOHUD_CONFIG").map(String::as_str),
            Some(env_str.as_str())
        );
    }
}
