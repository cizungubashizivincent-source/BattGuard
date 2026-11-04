//! Application des profils d'alimentation via `powercfg`.
//! Si l'operation est refusee (droits admin), une elevation UAC est tentee.

use std::process::Command;

pub const GUID_BALANCED: &str = "381b4222-f694-41f0-9685-ff5bb260df2e";
pub const GUID_SAVER: &str = "a1841308-3541-4fab-bc81-f71556f20b4a";
pub const GUID_PERF: &str = "8c5e7fda-e8c7-40ce-8a41-8df5e51cfc9a";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Balanced,
    Saver,
    Performance,
    /// Profil personnalise : processeur plafonne sur batterie + economie.
    Extended,
}

fn run_powercfg(args: &[&str]) -> Result<(), String> {
    let out = Command::new("powercfg")
        .args(args)
        .output()
        .map_err(|e| format!("Impossible de lancer powercfg : {e}"))?;

    if out.status.success() {
        return Ok(());
    }

    // Reessai en tant qu'administrateur (declenche une demande UAC).
    let joined = args.join(" ");
    let ps = format!(
        "Start-Process powercfg -ArgumentList '{joined}' -Verb RunAs -Wait -WindowStyle Hidden"
    );
    let out2 = Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps])
        .output()
        .map_err(|e| format!("Elevation impossible : {e}"))?;

    if out2.status.success() {
        Ok(())
    } else {
        Err("Action annulee ou refusee (droits administrateur requis)".to_string())
    }
}

/// Applique un profil. Retourne un message de confirmation.
pub fn apply(profile: Profile) -> Result<String, String> {
    let (msg, cmds): (String, Vec<Vec<&str>>) = match profile {
        Profile::Balanced => (
            "Profil Equilibre active.".to_string(),
            vec![vec!["/setactive", GUID_BALANCED]],
        ),
        Profile::Saver => (
            "Profil Econome active.".to_string(),
            vec![vec!["/setactive", GUID_SAVER]],
        ),
        Profile::Performance => (
            "Profil Performances active.".to_string(),
            vec![vec!["/setactive", GUID_PERF]],
        ),
        Profile::Extended => (
            "Batterie prolongee active : processeur plafonne a 70% sur batterie, \
             profil Econome, min CPU 5%."
                .to_string(),
            vec![
                vec![
                    "/setdcvalueindex",
                    "scheme_current",
                    "sub_processor",
                    "PROCTHROTTLEMAX",
                    "70",
                ],
                vec![
                    "/setdcvalueindex",
                    "scheme_current",
                    "sub_processor",
                    "PROCTHROTTLEMIN",
                    "5",
                ],
                vec!["/setactive", GUID_SAVER],
            ],
        ),
    };

    for c in cmds {
        run_powercfg(&c)?;
    }
    Ok(msg)
}

/// Nom du plan actif (avec marqueur si le mode "Batterie prolongee" est applique).
pub fn active_scheme(extended: bool) -> String {
    let guid = active_guid().unwrap_or_default();
    let mut name = if guid.starts_with("381b4222") {
        "Equilibre"
    } else if guid.starts_with("a1841308") {
        "Econome"
    } else if guid.starts_with("8c5e7fda") {
        "Performances"
    } else if guid.starts_with("e9a42b02") {
        "Ultimate"
    } else if guid.is_empty() {
        "Inconnu"
    } else {
        "Personnalise"
    }
    .to_string();

    if extended {
        name.push_str(" + Batterie prolongee");
    }
    name
}

fn active_guid() -> Option<String> {
    let out = Command::new("powercfg")
        .args(["/getactivescheme"])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout).to_lowercase();
    let open = s.find('(')?;
    let close = s[open..].find(')')? + open;
    let inner = &s[open + 1..close];
    let guid: String = inner
        .chars()
        .filter(|c| c.is_ascii_hexdigit() || *c == '-')
        .collect();
    if guid.len() >= 36 {
        Some(guid[..36].to_string())
    } else {
        None
    }
}
