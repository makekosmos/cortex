use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::Path;
use sysinfo::{DiskKind, Disks, System};
use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::{
    EnumDisplayDevicesW, EnumDisplaySettingsW, DEVMODEW, DISPLAY_DEVICEW,
    DISPLAY_DEVICE_ATTACHED_TO_DESKTOP, DISPLAY_DEVICE_MIRRORING_DRIVER,
    DISPLAY_DEVICE_PRIMARY_DEVICE, ENUM_CURRENT_SETTINGS,
};
use wmi::{COMLibrary, WMIConnection, WMIError};

#[derive(Serialize)]
pub struct SystemInfo {
    pub hostname: Option<String>,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub kernel_version: Option<String>,
    pub uptime_seconds: u64,
    pub boot_time: u64,
    pub arch: String,
    pub cpu: CpuInfo,
    pub memory: MemoryInfo,
    pub disks: Vec<DiskInfo>,
    pub gpus: Vec<GpuInfo>,
    pub monitors: Vec<MonitorInfo>,
}

#[derive(Serialize)]
pub struct CpuInfo {
    pub brand: String,
    pub vendor_id: String,
    pub frequency_mhz: u64,
    pub physical_cores: Option<usize>,
    pub logical_cores: usize,
}

#[derive(Serialize)]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub free_bytes: u64,
    pub available_bytes: u64,
    pub total_swap_bytes: u64,
    pub used_swap_bytes: u64,
}

#[derive(Serialize)]
pub struct DiskInfo {
    pub name: String,
    pub mount_point: String,
    pub file_system: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub kind: String,
    pub is_removable: bool,
    pub model: Option<String>,
    pub media_type: Option<String>,
}

#[derive(Serialize)]
pub struct GpuInfo {
    pub name: String,
    pub device_name: String,
    pub is_primary: bool,
}

#[derive(Serialize)]
pub struct MonitorInfo {
    pub name: String,
    pub device_name: String,
    pub width: u32,
    pub height: u32,
    pub refresh_rate: u32,
    pub is_primary: bool,
}

fn disk_kind_label(kind: DiskKind) -> String {
    match kind {
        DiskKind::HDD => "HDD".to_string(),
        DiskKind::SSD => "SSD".to_string(),
        DiskKind::Unknown(_) => "Unknown".to_string(),
    }
}

fn utf16_to_string(buffer: &[u16]) -> String {
    let len = buffer
        .iter()
        .position(|&value| value == 0)
        .unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..len]).trim().to_string()
}

#[derive(Debug, Deserialize)]
struct WmiMonitorId {
    #[serde(rename = "InstanceName")]
    instance_name: String,
    #[serde(rename = "UserFriendlyName")]
    user_friendly_name: Option<Vec<u16>>,
}

#[derive(Debug, Deserialize)]
struct WmiVideoMode {
    #[serde(rename = "HorizontalActivePixels")]
    horizontal_active_pixels: u32,
    #[serde(rename = "VerticalActivePixels")]
    vertical_active_pixels: u32,
    #[serde(rename = "VerticalRefreshRateNumerator")]
    vertical_refresh_rate_numerator: u32,
    #[serde(rename = "VerticalRefreshRateDenominator")]
    vertical_refresh_rate_denominator: u32,
}

#[derive(Debug, Deserialize)]
struct WmiMonitorModes {
    #[serde(rename = "InstanceName")]
    instance_name: String,
    #[serde(rename = "PreferredMonitorSourceModeIndex")]
    preferred_monitor_source_mode_index: u32,
    #[serde(rename = "MonitorSourceModes")]
    monitor_source_modes: Vec<WmiVideoMode>,
}

#[derive(Debug, Clone)]
struct WmiMonitorInfo {
    name: String,
    preferred_width: u32,
    preferred_height: u32,
    preferred_refresh: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WmiDiskDrive {
    model: Option<String>,
    media_type: Option<String>,
    index: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WmiDiskPartition {
    device_id: String,
    disk_index: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WmiLogicalDiskToPartition {
    antecedent: String,
    dependent: String,
}

fn parse_refresh_rate(numerator: u32, denominator: u32) -> u32 {
    if numerator == 0 || denominator == 0 {
        return 0;
    }
    ((numerator as f64 / denominator as f64).round()) as u32
}

fn extract_pnp_id_from_instance(instance_name: &str) -> Option<String> {
    instance_name
        .split('\\')
        .nth(1)
        .map(|value| value.trim().to_uppercase())
        .filter(|value| !value.is_empty())
}

fn extract_wmi_quoted_value(value: &str) -> Option<String> {
    let start = value.find('\"')?;
    let end = value[start + 1..].find('\"')?;
    Some(value[start + 1..start + 1 + end].to_string())
}

fn normalize_media_type(model: &str, media_type: Option<&str>) -> String {
    let model_upper = model.to_uppercase();
    if model_upper.contains("SSD") || model_upper.contains("NVME") || model_upper.contains("NVM") {
        return "SSD".to_string();
    }

    if let Some(media) = media_type {
        let media_upper = media.to_uppercase();
        if media_upper.contains("SSD") || media_upper.contains("SOLID") {
            return "SSD".to_string();
        }
    }

    "HDD".to_string()
}

fn collect_wmi_monitor_info() -> HashMap<String, WmiMonitorInfo> {
    const RPC_E_CHANGED_MODE: i32 = -2147417850;
    let mut map = HashMap::new();
    let com = match COMLibrary::new() {
        Ok(com) => com,
        Err(WMIError::HResultError { hres }) if hres == RPC_E_CHANGED_MODE => unsafe {
            COMLibrary::assume_initialized()
        },
        Err(err) => {
            eprintln!("WMI COM init failed: {:?}", err);
            return map;
        }
    };
    let wmi = match WMIConnection::with_namespace_path("ROOT\\WMI", com) {
        Ok(wmi) => wmi,
        Err(err) => {
            eprintln!("WMI connection failed (ROOT\\\\WMI): {:?}", err);
            return map;
        }
    };

    let query = "SELECT InstanceName, UserFriendlyName FROM WmiMonitorID";
    let results: Vec<WmiMonitorId> = match wmi.raw_query(query) {
        Ok(results) => results,
        Err(err) => {
            eprintln!("WMI query failed (WmiMonitorID): {:?}", err);
            return map;
        }
    };

    for entry in results {
        let name = match entry.user_friendly_name {
            Some(ref raw) => utf16_to_string(raw),
            None => String::new(),
        };
        if name.is_empty() {
            continue;
        }

        if let Some(pnp_id) = extract_pnp_id_from_instance(&entry.instance_name) {
            map.insert(
                pnp_id,
                WmiMonitorInfo {
                    name,
                    preferred_width: 0,
                    preferred_height: 0,
                    preferred_refresh: 0,
                },
            );
        }
    }

    let modes_query = "SELECT InstanceName, PreferredMonitorSourceModeIndex, MonitorSourceModes FROM WmiMonitorListedSupportedSourceModes";
    let modes: Vec<WmiMonitorModes> = match wmi.raw_query(modes_query) {
        Ok(modes) => modes,
        Err(err) => {
            eprintln!(
                "WMI query failed (WmiMonitorListedSupportedSourceModes): {:?}",
                err
            );
            return map;
        }
    };

    for entry in modes {
        let pnp_id = match extract_pnp_id_from_instance(&entry.instance_name) {
            Some(pnp_id) => pnp_id,
            None => continue,
        };

        let preferred_index = entry.preferred_monitor_source_mode_index as usize;
        let preferred_mode = entry.monitor_source_modes.get(preferred_index);

        let (width, height, refresh) = if let Some(mode) = preferred_mode {
            (
                mode.horizontal_active_pixels,
                mode.vertical_active_pixels,
                parse_refresh_rate(
                    mode.vertical_refresh_rate_numerator,
                    mode.vertical_refresh_rate_denominator,
                ),
            )
        } else {
            let mut best = (0, 0, 0);
            for mode in entry.monitor_source_modes {
                let rate = parse_refresh_rate(
                    mode.vertical_refresh_rate_numerator,
                    mode.vertical_refresh_rate_denominator,
                );
                if rate > best.2 {
                    best = (
                        mode.horizontal_active_pixels,
                        mode.vertical_active_pixels,
                        rate,
                    );
                }
            }
            best
        };

        if width == 0 || height == 0 {
            continue;
        }

        map.entry(pnp_id)
            .and_modify(|info| {
                info.preferred_width = width;
                info.preferred_height = height;
                info.preferred_refresh = refresh;
            })
            .or_insert(WmiMonitorInfo {
                name: String::new(),
                preferred_width: width,
                preferred_height: height,
                preferred_refresh: refresh,
            });
    }

    map
}

fn collect_wmi_disk_models() -> HashMap<String, (String, String)> {
    let mut map = HashMap::new();
    let com = match COMLibrary::new() {
        Ok(com) => com,
        Err(_) => return map,
    };
    let wmi = match WMIConnection::new(com) {
        Ok(wmi) => wmi,
        Err(_) => return map,
    };

    let drives: Vec<WmiDiskDrive> =
        match wmi.raw_query("SELECT Model, MediaType, Index FROM Win32_DiskDrive") {
            Ok(drives) => drives,
            Err(_) => return map,
        };
    let partitions: Vec<WmiDiskPartition> =
        match wmi.raw_query("SELECT DeviceID, DiskIndex FROM Win32_DiskPartition") {
            Ok(parts) => parts,
            Err(_) => return map,
        };
    let links: Vec<WmiLogicalDiskToPartition> =
        match wmi.raw_query("SELECT Antecedent, Dependent FROM Win32_LogicalDiskToPartition") {
            Ok(links) => links,
            Err(_) => return map,
        };

    let disk_index_to_drive = drives
        .into_iter()
        .map(|drive| {
            let model = drive.model.unwrap_or_else(|| "Unknown".to_string());
            let media = normalize_media_type(&model, drive.media_type.as_deref());
            (drive.index, (model, media))
        })
        .collect::<HashMap<_, _>>();

    let partition_to_index = partitions
        .into_iter()
        .map(|partition| (partition.device_id, partition.disk_index))
        .collect::<HashMap<_, _>>();

    for link in links {
        let partition_id = match extract_wmi_quoted_value(&link.antecedent) {
            Some(value) => value,
            None => continue,
        };
        let logical_id = match extract_wmi_quoted_value(&link.dependent) {
            Some(value) => value,
            None => continue,
        };

        let disk_index = match partition_to_index.get(&partition_id) {
            Some(index) => *index,
            None => continue,
        };
        let (model, media) = match disk_index_to_drive.get(&disk_index) {
            Some(data) => data.clone(),
            None => continue,
        };

        map.insert(logical_id.to_uppercase(), (model, media));
    }

    map
}

fn collect_display_info() -> (Vec<GpuInfo>, Vec<MonitorInfo>) {
    let mut gpus = Vec::new();
    let mut monitors = Vec::new();
    let mut seen_monitors = HashSet::new();
    let wmi_info = collect_wmi_monitor_info();

    let mut adapter_index = 0;
    loop {
        let mut adapter = DISPLAY_DEVICEW {
            cb: std::mem::size_of::<DISPLAY_DEVICEW>() as u32,
            ..Default::default()
        };
        let adapter_ok = unsafe {
            EnumDisplayDevicesW(PCWSTR::null(), adapter_index, &mut adapter, 0).as_bool()
        };
        if !adapter_ok {
            break;
        }

        let adapter_name = utf16_to_string(&adapter.DeviceString);
        let adapter_device = utf16_to_string(&adapter.DeviceName);
        let adapter_flags = adapter.StateFlags;
        let adapter_attached = (adapter_flags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP) != 0;
        let adapter_primary = (adapter_flags & DISPLAY_DEVICE_PRIMARY_DEVICE) != 0;

        if !adapter_name.is_empty() && (adapter_attached || adapter_primary) {
            gpus.push(GpuInfo {
                name: adapter_name,
                device_name: adapter_device.clone(),
                is_primary: adapter_primary,
            });
        }

        let mut monitor_index = 0;
        loop {
            let mut monitor = DISPLAY_DEVICEW {
                cb: std::mem::size_of::<DISPLAY_DEVICEW>() as u32,
                ..Default::default()
            };
            let monitor_ok = unsafe {
                EnumDisplayDevicesW(
                    PCWSTR::from_raw(adapter.DeviceName.as_ptr()),
                    monitor_index,
                    &mut monitor,
                    0,
                )
                .as_bool()
            };
            if !monitor_ok {
                break;
            }

            let monitor_flags = monitor.StateFlags;
            if (monitor_flags & DISPLAY_DEVICE_MIRRORING_DRIVER) != 0 {
                monitor_index += 1;
                continue;
            }

            let mut monitor_name = utf16_to_string(&monitor.DeviceString);
            let monitor_device = utf16_to_string(&monitor.DeviceName);
            let monitor_device_id = utf16_to_string(&monitor.DeviceID);

            if monitor_name.is_empty() && monitor_device.is_empty() {
                monitor_index += 1;
                continue;
            }

            if !monitor_device.is_empty() && !seen_monitors.insert(monitor_device.clone()) {
                monitor_index += 1;
                continue;
            }

            let mut preferred_width = 0;
            let mut preferred_height = 0;
            let mut preferred_refresh = 0;

            if let Some(pnp_id) = extract_pnp_id_from_instance(&monitor_device_id) {
                if let Some(info) = wmi_info.get(&pnp_id) {
                    if !info.name.is_empty() {
                        monitor_name = info.name.clone();
                    }
                    preferred_width = info.preferred_width;
                    preferred_height = info.preferred_height;
                    preferred_refresh = info.preferred_refresh;
                }
            }

            let mut width = 0;
            let mut height = 0;
            let mut refresh_rate = 0;
            let mut devmode = DEVMODEW {
                dmSize: std::mem::size_of::<DEVMODEW>() as u16,
                ..Default::default()
            };
            let settings_ok = unsafe {
                EnumDisplaySettingsW(
                    if monitor.DeviceName[0] != 0 {
                        PCWSTR::from_raw(monitor.DeviceName.as_ptr())
                    } else {
                        PCWSTR::from_raw(adapter.DeviceName.as_ptr())
                    },
                    ENUM_CURRENT_SETTINGS,
                    &mut devmode,
                )
                .as_bool()
            };
            if settings_ok {
                width = devmode.dmPelsWidth;
                height = devmode.dmPelsHeight;
                refresh_rate = devmode.dmDisplayFrequency;
            }
            if width == 0 && preferred_width > 0 {
                width = preferred_width;
            }
            if height == 0 && preferred_height > 0 {
                height = preferred_height;
            }
            if refresh_rate == 0 && preferred_refresh > 0 {
                refresh_rate = preferred_refresh;
            }

            let is_primary = (monitor_flags & DISPLAY_DEVICE_PRIMARY_DEVICE) != 0;

            monitors.push(MonitorInfo {
                name: monitor_name,
                device_name: monitor_device,
                width,
                height,
                refresh_rate,
                is_primary,
            });

            monitor_index += 1;
        }

        adapter_index += 1;
    }

    if monitors.is_empty() && !wmi_info.is_empty() {
        monitors = wmi_info
            .values()
            .enumerate()
            .map(|(index, info)| MonitorInfo {
                name: if info.name.is_empty() {
                    format!("Monitor {}", index + 1)
                } else {
                    info.name.clone()
                },
                device_name: String::new(),
                width: info.preferred_width,
                height: info.preferred_height,
                refresh_rate: info.preferred_refresh,
                is_primary: index == 0,
            })
            .collect();
    }

    monitors.sort_by(|a, b| a.name.cmp(&b.name));

    (gpus, monitors)
}

#[tauri::command]
pub fn get_system_info() -> SystemInfo {
    let mut sys = System::new_all();
    sys.refresh_all();

    let mut disks = Disks::new_with_refreshed_list();
    disks.refresh(true);
    let disk_models = collect_wmi_disk_models();

    let (cpu_brand, cpu_vendor, cpu_frequency) = sys
        .cpus()
        .first()
        .map(|cpu| {
            (
                cpu.brand().to_string(),
                cpu.vendor_id().to_string(),
                cpu.frequency(),
            )
        })
        .unwrap_or_else(|| ("Unknown".to_string(), "".to_string(), 0));

    let mut disk_infos: Vec<DiskInfo> = disks
        .list()
        .iter()
        .map(|disk| {
            let mount_point = disk.mount_point().to_string_lossy().to_string();
            let drive_key = mount_point.trim_end_matches('\\').to_uppercase();
            let model_info = disk_models.get(&drive_key);

            DiskInfo {
                name: disk.name().to_string_lossy().to_string(),
                mount_point,
                file_system: disk.file_system().to_string_lossy().to_string(),
                total_bytes: disk.total_space(),
                available_bytes: disk.available_space(),
                kind: disk_kind_label(disk.kind()),
                is_removable: disk.is_removable(),
                model: model_info.map(|value| value.0.clone()),
                media_type: model_info.map(|value| value.1.clone()),
            }
        })
        .collect();

    disk_infos.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));
    let (gpus, monitors) = collect_display_info();

    SystemInfo {
        hostname: System::host_name(),
        os_name: System::name(),
        os_version: System::os_version(),
        kernel_version: System::kernel_version(),
        uptime_seconds: System::uptime(),
        boot_time: System::boot_time(),
        arch: std::env::consts::ARCH.to_string(),
        cpu: CpuInfo {
            brand: cpu_brand,
            vendor_id: cpu_vendor,
            frequency_mhz: cpu_frequency,
            physical_cores: System::physical_core_count(),
            logical_cores: sys.cpus().len(),
        },
        memory: MemoryInfo {
            total_bytes: sys.total_memory(),
            used_bytes: sys.used_memory(),
            free_bytes: sys.free_memory(),
            available_bytes: sys.available_memory(),
            total_swap_bytes: sys.total_swap(),
            used_swap_bytes: sys.used_swap(),
        },
        disks: disk_infos,
        gpus,
        monitors,
    }
}

#[derive(Serialize)]
pub struct DiskSpeedResult {
    pub mount_point: String,
    pub size_bytes: u64,
    pub write_mbps: f64,
    pub read_mbps: f64,
    pub elapsed_write_ms: u128,
    pub elapsed_read_ms: u128,
}

#[tauri::command]
pub fn test_disk_speed(mount_point: String) -> Result<DiskSpeedResult, String> {
    if mount_point.trim().is_empty() {
        return Err("Empty mount point".to_string());
    }

    let base_path = Path::new(&mount_point);
    let test_dir = base_path.join("arrancador_speedtest");
    let test_file = test_dir.join("speedtest.bin");
    std::fs::create_dir_all(&test_dir).map_err(|err| err.to_string())?;

    let size_bytes: u64 = 128 * 1024 * 1024;
    let chunk_size: usize = 4 * 1024 * 1024;
    let buffer = vec![0xA5u8; chunk_size];

    let write_start = std::time::Instant::now();
    {
        let mut file = std::fs::File::create(&test_file).map_err(|err| err.to_string())?;
        let mut remaining = size_bytes;
        while remaining > 0 {
            let to_write = std::cmp::min(remaining as usize, chunk_size);
            file.write_all(&buffer[..to_write])
                .map_err(|err| err.to_string())?;
            remaining -= to_write as u64;
        }
        file.sync_all().map_err(|err| err.to_string())?;
    }
    let elapsed_write_ms = write_start.elapsed().as_millis();

    let read_start = std::time::Instant::now();
    {
        let mut file = std::fs::File::open(&test_file).map_err(|err| err.to_string())?;
        let mut read_buffer = vec![0u8; chunk_size];
        loop {
            let read = file.read(&mut read_buffer).map_err(|err| err.to_string())?;
            if read == 0 {
                break;
            }
        }
    }
    let elapsed_read_ms = read_start.elapsed().as_millis();

    let _ = std::fs::remove_file(&test_file);
    let _ = std::fs::remove_dir(&test_dir);

    let write_seconds = (elapsed_write_ms as f64) / 1000.0;
    let read_seconds = (elapsed_read_ms as f64) / 1000.0;
    let size_mb = (size_bytes as f64) / 1_048_576.0;

    let write_mbps = if write_seconds > 0.0 {
        size_mb / write_seconds
    } else {
        0.0
    };
    let read_mbps = if read_seconds > 0.0 {
        size_mb / read_seconds
    } else {
        0.0
    };

    Ok(DiskSpeedResult {
        mount_point,
        size_bytes,
        write_mbps,
        read_mbps,
        elapsed_write_ms,
        elapsed_read_ms,
    })
}
