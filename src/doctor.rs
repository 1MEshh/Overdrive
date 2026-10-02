use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
    Info,
}

impl CheckStatus {
    pub fn badge(&self) -> &'static str {
        match self {
            CheckStatus::Pass => "[PASS]",
            CheckStatus::Warn => "[WARN]",
            CheckStatus::Fail => "[FAIL]",
            CheckStatus::Info => "[INFO]",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticCheck {
    pub category: String,
    pub name: String,
    pub status: CheckStatus,
    pub current_value: String,
    pub recommended_value: String,
    pub details: String,
    pub recommendation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DoctorReport {
    pub overall_status: CheckStatus,
    pub checks: Vec<DiagnosticCheck>,
    pub vulkan_icds: Vec<String>,
    pub detected_gpus: Vec<String>,
    pub display_server: String,
    pub session_compositor: String,
}

impl DoctorReport {
    pub fn is_critical(&self) -> bool {
        self.overall_status == CheckStatus::Fail
    }

    pub fn to_ascii_report(&self) -> String {
        let mut out = String::new();
        out.push_str("+------------------------------------------------------------------------------+\n");
        out.push_str("|                  OVERDRIVE LINUX GAMING ENVIRONMENT DOCTOR                  |\n");
        out.push_str("+------------------------------------------------------------------------------+\n");
        out.push_str(&format!(
            "| Overall System Status : {:<52} |\n",
            self.overall_status.badge()
        ));
        out.push_str(&format!(
            "| Display Server Type   : {:<52} |\n",
            self.display_server
        ));
        out.push_str(&format!(
            "| Window Compositor     : {:<52} |\n",
            self.session_compositor
        ));
        out.push_str("+------------------------------------------------------------------------------+\n");
        out.push_str("| DIAGNOSTIC SYSTEM CHECKS                                                     |\n");
        out.push_str("+--------+--------------------------+------------------------------------------+\n");
        out.push_str("| STATUS | CHECK NAME               | CURRENT VALUE                            |\n");
        out.push_str("+--------+--------------------------+------------------------------------------+\n");

        for check in &self.checks {
            out.push_str(&format!(
                "| {:<6} | {:<24} | {:<40} |\n",
                check.status.badge(),
                truncate_str(&check.name, 24),
                truncate_str(&check.current_value, 40)
            ));
        }

        out.push_str("+--------+--------------------------+------------------------------------------+\n");

        // Actionable Recommendations
        let recommendations: Vec<&DiagnosticCheck> = self
            .checks
            .iter()
            .filter(|c| c.status == CheckStatus::Warn || c.status == CheckStatus::Fail)
            .collect();

        if !recommendations.is_empty() {
            out.push_str("| ACTIONABLE REMEDIATIONS & TUNING INSTRUCTIONS                                |\n");
            out.push_str("+------------------------------------------------------------------------------+\n");
            for rec in recommendations {
                out.push_str(&format!(
                    "| [*] {}: {}\n",
                    rec.name,
                    rec.details
                ));
                if let Some(cmd) = &rec.recommendation {
                    out.push_str(&format!("|     Remedy: {}\n", cmd));
                }
            }
            out.push_str("+------------------------------------------------------------------------------+\n");
        }

        // Vulkan ICD list
        out.push_str("| DETECTED VULKAN DRIVER ICDS                                                  |\n");
        out.push_str("+------------------------------------------------------------------------------+\n");
        if self.vulkan_icds.is_empty() {
            out.push_str("| [!] No Vulkan ICD manifest files discovered in standard paths.               |\n");
        } else {
            for icd in &self.vulkan_icds {
                out.push_str(&format!("| - {:<74} |\n", truncate_str(icd, 74)));
            }
        }
        out.push_str("+------------------------------------------------------------------------------+\n");

        out
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

pub fn run_diagnostics() -> DoctorReport {
    let mut checks = Vec::new();
    let mut overall = CheckStatus::Pass;

    // 1. vm.max_map_count check
    let max_map_count = read_proc_sys("/proc/sys/vm/max_map_count");
    match max_map_count {
        Some(val_str) => {
            if let Ok(count) = val_str.trim().parse::<u64>() {
                if count >= 1_048_576 {
                    checks.push(DiagnosticCheck {
                        category: "Kernel Memory".to_string(),
                        name: "vm.max_map_count".to_string(),
                        status: CheckStatus::Pass,
                        current_value: count.to_string(),
                        recommended_value: ">= 1048576 (ideal: 2147483648)".to_string(),
                        details: "Sufficient virtual memory allocations for modern game engines."
                            .to_string(),
                        recommendation: None,
                    });
                } else if count >= 262_144 {
                    checks.push(DiagnosticCheck {
                        category: "Kernel Memory".to_string(),
                        name: "vm.max_map_count".to_string(),
                        status: CheckStatus::Warn,
                        current_value: count.to_string(),
                        recommended_value: ">= 1048576".to_string(),
                        details: "Moderate limit. Modern titles (CS2, Star Citizen, UE5) may crash or stutter."
                            .to_string(),
                        recommendation: Some(
                            "echo 'vm.max_map_count=2147483648' | sudo tee /etc/sysctl.d/80-gaming.conf && sudo sysctl --system"
                                .to_string(),
                        ),
                    });
                    if overall == CheckStatus::Pass {
                        overall = CheckStatus::Warn;
                    }
                } else {
                    checks.push(DiagnosticCheck {
                        category: "Kernel Memory".to_string(),
                        name: "vm.max_map_count".to_string(),
                        status: CheckStatus::Fail,
                        current_value: count.to_string(),
                        recommended_value: ">= 1048576".to_string(),
                        details: "Dangerously low limit! High probability of crashes in DirectX 12/Vulkan titles."
                            .to_string(),
                        recommendation: Some(
                            "sudo sysctl -w vm.max_map_count=2147483648".to_string(),
                        ),
                    });
                    overall = CheckStatus::Fail;
                }
            } else {
                checks.push(DiagnosticCheck {
                    category: "Kernel Memory".to_string(),
                    name: "vm.max_map_count".to_string(),
                    status: CheckStatus::Warn,
                    current_value: val_str,
                    recommended_value: ">= 1048576".to_string(),
                    details: "Unparseable value".to_string(),
                    recommendation: None,
                });
            }
        }
        None => {
            checks.push(DiagnosticCheck {
                category: "Kernel Memory".to_string(),
                name: "vm.max_map_count".to_string(),
                status: CheckStatus::Warn,
                current_value: "Not accessible".to_string(),
                recommended_value: ">= 1048576".to_string(),
                details: "Cannot access /proc/sys/vm/max_map_count".to_string(),
                recommendation: None,
            });
        }
    }

    // 2. kernel.split_lock_mitigate check
    let split_lock = read_proc_sys("/proc/sys/kernel/split_lock_mitigate");
    if let Some(val_str) = split_lock {
        let trimmed = val_str.trim();
        if trimmed == "0" {
            checks.push(DiagnosticCheck {
                category: "Kernel CPU".to_string(),
                name: "split_lock_mitigate".to_string(),
                status: CheckStatus::Pass,
                current_value: "0 (Disabled)".to_string(),
                recommended_value: "0".to_string(),
                details: "Split-lock bus penalty mitigation is disabled. Prevents severe gaming microstutters."
                    .to_string(),
                recommendation: None,
            });
        } else if trimmed == "1" {
            checks.push(DiagnosticCheck {
                category: "Kernel CPU".to_string(),
                name: "split_lock_mitigate".to_string(),
                status: CheckStatus::Warn,
                current_value: "1 (Enabled)".to_string(),
                recommended_value: "0".to_string(),
                details: "Active split-lock mitigation induces massive 50-80% frame dips during bus locks."
                    .to_string(),
                recommendation: Some(
                    "sudo sysctl -w kernel.split_lock_mitigate=0".to_string(),
                ),
            });
            if overall == CheckStatus::Pass {
                overall = CheckStatus::Warn;
            }
        }
    } else {
        checks.push(DiagnosticCheck {
            category: "Kernel CPU".to_string(),
            name: "split_lock_mitigate".to_string(),
            status: CheckStatus::Info,
            current_value: "Unsupported / Non-x86".to_string(),
            recommended_value: "N/A".to_string(),
            details: "Kernel does not expose split_lock_mitigate sysctl parameter.".to_string(),
            recommendation: None,
        });
    }

    // 3. fs.file-max check
    if let Some(file_max_str) = read_proc_sys("/proc/sys/fs/file-max") {
        if let Ok(limit) = file_max_str.trim().parse::<u64>() {
            if limit >= 524_288 {
                checks.push(DiagnosticCheck {
                    category: "Kernel Filesystem".to_string(),
                    name: "fs.file-max".to_string(),
                    status: CheckStatus::Pass,
                    current_value: limit.to_string(),
                    recommended_value: ">= 524288".to_string(),
                    details: "Adequate maximum open file descriptors ceiling for asset-heavy titles."
                        .to_string(),
                    recommendation: None,
                });
            } else {
                checks.push(DiagnosticCheck {
                    category: "Kernel Filesystem".to_string(),
                    name: "fs.file-max".to_string(),
                    status: CheckStatus::Warn,
                    current_value: limit.to_string(),
                    recommended_value: ">= 524288".to_string(),
                    details: "Low maximum open files limit.".to_string(),
                    recommendation: Some("sudo sysctl -w fs.file-max=2097152".to_string()),
                });
            }
        }
    }

    // 4. Vulkan ICD discovery & conflict detection
    let (vulkan_icds, icd_checks) = scan_vulkan_icds();
    checks.extend(icd_checks);

    // 5. CPU Frequency scaling governor
    let cpu_gov = read_proc_sys("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor");
    if let Some(gov) = cpu_gov {
        let trimmed = gov.trim();
        if trimmed == "performance" {
            checks.push(DiagnosticCheck {
                category: "CPU Power".to_string(),
                name: "scaling_governor".to_string(),
                status: CheckStatus::Pass,
                current_value: trimmed.to_string(),
                recommended_value: "performance".to_string(),
                details: "CPU governor set to static peak frequency mode.".to_string(),
                recommendation: None,
            });
        } else if trimmed == "schedutil" || trimmed == "ondemand" {
            checks.push(DiagnosticCheck {
                category: "CPU Power".to_string(),
                name: "scaling_governor".to_string(),
                status: CheckStatus::Warn,
                current_value: trimmed.to_string(),
                recommended_value: "performance".to_string(),
                details: "Dynamic governor introduces frequency ramp latency during frame spikes."
                    .to_string(),
                recommendation: Some(
                    "echo performance | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor"
                        .to_string(),
                ),
            });
            if overall == CheckStatus::Pass {
                overall = CheckStatus::Warn;
            }
        } else {
            checks.push(DiagnosticCheck {
                category: "CPU Power".to_string(),
                name: "scaling_governor".to_string(),
                status: CheckStatus::Fail,
                current_value: trimmed.to_string(),
                recommended_value: "performance".to_string(),
                details: "Powersave or low-power governor active. Severe frame rate degradation."
                    .to_string(),
                recommendation: Some(
                    "echo performance | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor"
                        .to_string(),
                ),
            });
            overall = CheckStatus::Fail;
        }
    }

    // 6. Gaming Utilities Check
    let check_binary = |bin_name: &'static str| -> DiagnosticCheck {
        let found = which_binary(bin_name);
        DiagnosticCheck {
            category: "Gaming Utilities".to_string(),
            name: bin_name.to_string(),
            status: if found {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            current_value: if found {
                "Installed".to_string()
            } else {
                "Not Found".to_string()
            },
            recommended_value: "Installed".to_string(),
            details: format!("Utility '{}' availability in PATH.", bin_name),
            recommendation: if found {
                None
            } else {
                Some(format!("Install '{}' via system package manager", bin_name))
            },
        }
    };

    checks.push(check_binary("gamemoderun"));
    checks.push(check_binary("gamescope"));
    checks.push(check_binary("mangohud"));

    // 7. Display Server & Compositor
    let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "unknown".to_string());
    let display_server = if std::env::var("WAYLAND_DISPLAY").is_ok() || session_type == "wayland" {
        "Wayland".to_string()
    } else if std::env::var("DISPLAY").is_ok() || session_type == "x11" {
        "X11".to_string()
    } else {
        session_type
    };

    let session_compositor = std::env::var("XDG_CURRENT_DESKTOP")
        .or_else(|_| std::env::var("DESKTOP_SESSION"))
        .unwrap_or_else(|_| "unknown".to_string());

    DoctorReport {
        overall_status: overall,
        checks,
        vulkan_icds,
        detected_gpus: Vec::new(),
        display_server,
        session_compositor,
    }
}

fn scan_vulkan_icds() -> (Vec<String>, Vec<DiagnosticCheck>) {
    let mut icd_paths = Vec::new();
    let mut checks = Vec::new();

    let search_dirs = [
        "/usr/share/vulkan/icd.d",
        "/etc/vulkan/icd.d",
        "/usr/local/share/vulkan/icd.d",
    ];

    let mut found_radv = false;
    let mut found_amdvlk = false;
    let mut found_nvidia = false;
    let mut found_intel = false;

    for dir in &search_dirs {
        let p = Path::new(dir);
        if let Ok(entries) = fs::read_dir(p) {
            for entry in entries.flatten() {
                let file_path = entry.path();
                if let Some(ext) = file_path.extension() {
                    if ext == "json" {
                        let filename = file_path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();

                        let lower = filename.to_ascii_lowercase();
                        if lower.contains("radeon") {
                            found_radv = true;
                        }
                        if lower.contains("amd_icd") {
                            found_amdvlk = true;
                        }
                        if lower.contains("nvidia") {
                            found_nvidia = true;
                        }
                        if lower.contains("intel") {
                            found_intel = true;
                        }

                        icd_paths.push(file_path.display().to_string());
                    }
                }
            }
        }
    }

    if found_radv && found_amdvlk {
        checks.push(DiagnosticCheck {
            category: "Vulkan Driver".to_string(),
            name: "AMD Driver Conflict".to_string(),
            status: CheckStatus::Warn,
            current_value: "RADV + AMDVLK Coexisting".to_string(),
            recommended_value: "RADV Primary".to_string(),
            details: "Both RADV (Mesa) and AMDVLK ICDs are installed. AMDVLK may default over RADV causing shader compile stutters."
                .to_string(),
            recommendation: Some(
                "Export VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/radeon_icd.x86_64.json or uninstall amdvlk"
                    .to_string(),
            ),
        });
    } else if found_radv {
        checks.push(DiagnosticCheck {
            category: "Vulkan Driver".to_string(),
            name: "AMD RADV Driver".to_string(),
            status: CheckStatus::Pass,
            current_value: "RADV (Mesa ACO)".to_string(),
            recommended_value: "RADV".to_string(),
            details: "Optimal AMD Vulkan driver with ACO shader compiler identified.".to_string(),
            recommendation: None,
        });
    }

    if found_nvidia {
        checks.push(DiagnosticCheck {
            category: "Vulkan Driver".to_string(),
            name: "NVIDIA Proprietary ICD".to_string(),
            status: CheckStatus::Pass,
            current_value: "Detected".to_string(),
            recommended_value: "Detected".to_string(),
            details: "NVIDIA Vulkan ICD driver manifest present.".to_string(),
            recommendation: None,
        });
    }

    if found_intel {
        checks.push(DiagnosticCheck {
            category: "Vulkan Driver".to_string(),
            name: "Intel ANV Driver".to_string(),
            status: CheckStatus::Pass,
            current_value: "Detected".to_string(),
            recommended_value: "Detected".to_string(),
            details: "Intel ANV Mesa Vulkan driver manifest present.".to_string(),
            recommendation: None,
        });
    }

    if icd_paths.is_empty() {
        checks.push(DiagnosticCheck {
            category: "Vulkan Driver".to_string(),
            name: "Vulkan ICD Manifests".to_string(),
            status: CheckStatus::Fail,
            current_value: "0 ICDs Found".to_string(),
            recommended_value: ">= 1 ICD".to_string(),
            details: "No Vulkan driver manifest found in system search paths. Vulkan games will fail to launch."
                .to_string(),
            recommendation: Some("Install mesa-vulkan-drivers or nvidia-driver".to_string()),
        });
    }

    (icd_paths, checks)
}

fn read_proc_sys(path: &str) -> Option<String> {
    fs::read_to_string(path).ok()
}

fn which_binary(name: &str) -> bool {
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in path_var.split(':') {
            let p = Path::new(dir).join(name);
            if p.is_file() {
                return true;
            }
        }
    }
    false
}
