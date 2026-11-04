//! Lecture des informations batterie sous Windows :
//! - etat en temps reel via GetSystemPowerStatus (Win32, sans dependance)
//! - sante (capacite conception / pleine charge) via le rapport `powercfg /batteryreport`

use std::fs;
use std::process::Command;

#[repr(C)]
#[allow(dead_code)]
struct SystemPowerStatus {
    ac_line_status: u8,
    battery_flag: u8,
    battery_life_percent: u8,
    system_status_flag: u8,
    battery_life_time: i32,
    battery_full_life_time: i32,
}

#[link(name = "kernel32")]
extern "system" {
    fn GetSystemPowerStatus(lp_system_power_status: *mut SystemPowerStatus) -> i32;
}

/// Etat instantane de la batterie.
#[derive(Clone, Debug)]
pub struct BatteryLive {
    pub has_battery: bool,
    pub percent: f32,
    pub plugged: bool,
    pub charging: bool,
    /// Autonomie estimee par Windows en secondes (-1 = inconnu).
    pub life_time_secs: i64,
}

pub fn read_live() -> BatteryLive {
    let mut s = SystemPowerStatus {
        ac_line_status: 0,
        battery_flag: 0,
        battery_life_percent: 0,
        system_status_flag: 0,
        battery_life_time: -1,
        battery_full_life_time: -1,
    };

    let ok = unsafe { GetSystemPowerStatus(&mut s) };
    if ok == 0 {
        return BatteryLive {
            has_battery: false,
            percent: 0.0,
            plugged: false,
            charging: false,
            life_time_secs: -1,
        };
    }

    let has_battery = (s.battery_flag & 128) == 0 && s.battery_flag != 255;
    let percent = if s.battery_life_percent == 255 {
        0.0
    } else {
        s.battery_life_percent as f32
    };

    BatteryLive {
        has_battery,
        percent,
        plugged: s.ac_line_status == 1,
        charging: (s.battery_flag & 8) != 0,
        life_time_secs: if s.battery_life_time < 0 {
            -1
        } else {
            s.battery_life_time as i64
        },
    }
}

/// Sante chimique de la batterie (mWh).
#[derive(Clone, Debug)]
pub struct BatteryHealth {
    pub design_mwh: u64,
    pub full_mwh: u64,
}

impl BatteryHealth {
    pub fn health_pct(&self) -> f32 {
        if self.design_mwh == 0 {
            return 0.0;
        }
        (self.full_mwh as f32 / self.design_mwh as f32 * 100.0).min(100.0)
    }

    pub fn verdict(&self) -> (&'static str, [u8; 3]) {
        match self.health_pct() {
            p if p >= 80.0 => ("Bonne sante - usage normal", [80, 170, 90]),
            p if p >= 60.0 => (
                "Sante moyenne - privilegiez les charges a 80% pour durer",
                [230, 175, 50],
            ),
            _ => ("Sante faible - envisagez le remplacement de la batterie", [225, 85, 75]),
        }
    }
}

/// Genere le rapport batterie et en extrait les capacites.
pub fn read_health() -> Result<BatteryHealth, String> {
    let xml_path = std::env::temp_dir().join("battguard_report.xml");
    let p = xml_path.to_string_lossy().to_string();

    let out = Command::new("powercfg")
        .args(["/batteryreport", "/xml", "/output", &p])
        .output()
        .map_err(|e| format!("Impossible de lancer powercfg : {e}"))?;

    let content = if out.status.success() && xml_path.exists() {
        fs::read_to_string(&xml_path).unwrap_or_default()
    } else {
        // Repli : rapport HTML (certains systemes n'acceptent plus /xml)
        let html_path = std::env::temp_dir().join("battguard_report.html");
        let hp = html_path.to_string_lossy().to_string();
        let out2 = Command::new("powercfg")
            .args(["/batteryreport", "/output", &hp])
            .output()
            .map_err(|e| format!("Impossible de lancer powercfg : {e}"))?;

        if !out2.status.success() {
            let err = String::from_utf8_lossy(&out2.stderr);
            let err = err.trim();
            if err.is_empty() {
                return Err(
                    "Rapport batterie indisponible (aucune batterie detectee ?)".to_string(),
                );
            }
            return Err(format!("Rapport batterie indisponible : {err}"));
        }
        fs::read_to_string(&html_path).unwrap_or_default()
    };

    let _ = fs::remove_file(&xml_path);

    let design = find_capacity(&content, &[
        "designcapacity",
        "design_capacity",
        "design capacity",
    ])?;
    let full = find_capacity(&content, &[
        "fullchargecapacity",
        "full_charge_capacity",
        "full charge capacity",
    ])?;

    if design == 0 || full == 0 {
        return Err("Capacites introuvables dans le rapport batterie".to_string());
    }

    Ok(BatteryHealth {
        design_mwh: design,
        full_mwh: full,
    })
}

/// Cherche un label (insensible a la casse) puis extrait le nombre qui suit,
/// avec detection de l'unite (mWh / Wh). Valide pour le XML comme pour le HTML.
fn find_capacity(content: &str, needles: &[&str]) -> Result<u64, String> {
    let lower = content.to_lowercase();

    for needle in needles {
        let mut from = 0usize;
        while let Some(pos) = lower[from..].find(needle) {
            let abs = from + pos + needle.len();
            // On ne cherche que dans les ~100 caracteres qui suivent le label.
            let end = (abs + 100).min(lower.len());
            if let Some((value, unit_mwh)) = parse_number_with_unit(&lower[abs..end]) {
                return Ok(if unit_mwh { value } else { value * 1000 });
            }
            from = abs;
        }
    }
    Err("Capacite introuvable dans le rapport batterie".to_string())
}

/// Extrait le premier nombre ("45,000" ou "45000") suivi d'une unite.
fn parse_number_with_unit(s: &str) -> Option<(u64, bool)> {
    let bytes: Vec<char> = s.chars().collect();
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            let mut digits = String::new();
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == ',') {
                digits.push(bytes[i]);
                i += 1;
            }
            let cleaned: String = digits.chars().filter(|c| c.is_ascii_digit()).collect();
            if cleaned.len() >= 3 {
                if let Ok(v) = cleaned.parse::<u64>() {
                    let tail: String = bytes[i..(i + 6).min(bytes.len())]
                        .iter()
                        .collect::<String>()
                        .to_lowercase();
                    let unit_mwh = if tail.contains("mwh") {
                        true
                    } else if tail.contains("wh") {
                        false
                    } else {
                        true // defaut Windows : mWh
                    };
                    return Some((v, unit_mwh));
                }
            }
            continue;
        }
        i += 1;
    }
    None
}
