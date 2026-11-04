use eframe::egui;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::battery::{self, BatteryHealth, BatteryLive};
use crate::power::{self, Profile};

const REFRESH_SECS: f32 = 2.0;
const HISTORY_SECS: f64 = 900.0; // 15 min d'historique
const HEALTH_REFRESH_SECS: u64 = 300;

pub struct BattApp {
    live: BatteryLive,
    health: Option<BatteryHealth>,
    health_msg: String,
    history: VecDeque<(f64, f32)>,
    start: Instant,
    last_live: Instant,
    last_health: Instant,
    active_scheme: String,
    extended_active: bool,
    status_msg: String,
    status_ok: bool,
}

impl BattApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let live = battery::read_live();
        let (health, health_msg) = match battery::read_health() {
            Ok(h) => (Some(h), String::new()),
            Err(e) => (None, e),
        };

        let mut app = Self {
            live,
            health,
            health_msg,
            history: VecDeque::new(),
            start: Instant::now(),
            last_live: Instant::now(),
            last_health: Instant::now(),
            active_scheme: power::active_scheme(false),
            extended_active: false,
            status_msg: String::new(),
            status_ok: true,
        };
        app.push_sample();
        app
    }

    fn push_sample(&mut self) {
        if !self.live.has_battery {
            return;
        }
        let t = self.start.elapsed().as_secs_f64();
        self.history.push_back((t, self.live.percent));
        while let Some(&(tt, _)) = self.history.front() {
            if t - tt > HISTORY_SECS {
                self.history.pop_front();
            } else {
                break;
            }
        }
    }

    /// Tendance de la charge en %/minute (negatif = decharge).
    /// Regression lineaire sur les 6 dernieres minutes.
    fn rate_pct_per_min(&self) -> Option<f64> {
        if self.history.len() < 4 {
            return None;
        }
        let t_end = self.history.back()?.0;
        let pts: Vec<(f64, f64)> = self
            .history
            .iter()
            .filter(|(t, _)| t_end - t <= 360.0)
            .map(|&(t, p)| (t, p as f64))
            .collect();
        if pts.len() < 4 {
            return None;
        }
        let n = pts.len() as f64;
        let sum_t: f64 = pts.iter().map(|p| p.0).sum();
        let sum_p: f64 = pts.iter().map(|p| p.1).sum();
        let sum_tt: f64 = pts.iter().map(|p| p.0 * p.0).sum();
        let sum_tp: f64 = pts.iter().map(|p| p.0 * p.1).sum();
        let denom = n * sum_tt - sum_t * sum_t;
        if denom.abs() < 1e-9 {
            return None;
        }
        let slope = (n * sum_tp - sum_t * sum_p) / denom;
        Some(slope * 60.0)
    }

    /// Decharge estimee en watts.
    fn discharge_watts(&self) -> Option<f64> {
        let rate = self.rate_pct_per_min()?;
        let full = self.health.as_ref()?.full_mwh as f64;
        if rate >= -0.01 || full <= 0.0 {
            return None;
        }
        Some((-rate / 100.0) * full * 60.0 / 1000.0)
    }

    fn apply(&mut self, profile: Profile) {
        match power::apply(profile) {
            Ok(msg) => {
                self.status_msg = msg;
                self.status_ok = true;
            }
            Err(e) => {
                self.status_msg = e;
                self.status_ok = false;
            }
        }
        self.extended_active = profile == Profile::Extended;
        self.active_scheme = power::active_scheme(self.extended_active);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.last_live.elapsed().as_secs_f32() >= REFRESH_SECS {
            self.live = battery::read_live();
            self.push_sample();
            self.last_live = Instant::now();
        }
        if self.last_health.elapsed().as_secs() >= HEALTH_REFRESH_SECS {
            if let Ok(h) = battery::read_health() {
                self.health = Some(h);
            }
            self.last_health = Instant::now();
        }
        ctx.request_repaint_after(Duration::from_secs_f32(REFRESH_SECS));

        // ---------- En-tete ----------
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.heading("BattGuard");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("v0.1.0").weak());
                });
            });
            ui.label(
                egui::RichText::new("Prolongez l'autonomie et la duree de vie de votre batterie")
                    .weak(),
            );
            ui.add_space(8.0);
        });

        // ---------- Contenu ----------
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(6.0);

                // ---- Carte batterie ----
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.heading("Batterie");
                    if self.live.has_battery {
                        ui.add_space(6.0);
                        ui.add(
                            egui::ProgressBar::new(self.live.percent / 100.0)
                                .show_percentage()
                                .animate(true),
                        );
                        ui.add_space(6.0);
                        let state = if self.live.charging {
                            ("En charge", [90, 170, 240])
                        } else if self.live.plugged {
                            ("Branche sur secteur", [150, 150, 150])
                        } else {
                            ("Sur batterie", [240, 200, 80])
                        };
                        ui.horizontal(|ui| {
                            ui.colored_label(
                                egui::Color32::from_rgb(state.1[0], state.1[1], state.1[2]),
                                format!("● {}", state.0),
                            );
                            if !self.live.plugged {
                                if let Some(rate) = self.rate_pct_per_min() {
                                    if rate < -0.01 {
                                        ui.label(format!(
                                            "  ({:.1} %/min)",
                                            -rate
                                        ));
                                    }
                                }
                            }
                        });
                        if self.live.plugged && self.live.percent >= 80.0 {
                            ui.add_space(4.0);
                            ui.colored_label(
                                egui::Color32::from_rgb(230, 175, 50),
                                "Conseil : au-dessus de 80%, debranchez si vous n'avez pas \
                                 besoin du secteur (cela preserve la sante a long terme).",
                            );
                        }
                    } else {
                        ui.colored_label(
                            egui::Color32::from_rgb(200, 90, 80),
                            "Aucune batterie detectee (PC de bureau ?).",
                        );
                    }
                });

                ui.add_space(8.0);

                // ---- Carte autonomie ----
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.heading("Autonomie restante");
                    ui.add_space(4.0);
                    if !self.live.has_battery {
                        ui.label("--");
                    } else if self.live.life_time_secs > 0 {
                        ui.label(
                            egui::RichText::new(fmt_duration(self.live.life_time_secs as u64))
                                .size(24.0),
                        );
                        ui.label("Estimation fournie par Windows");
                    } else if let Some(rate) = self.rate_pct_per_min() {
                        if rate < -0.01 {
                            let secs = (self.live.percent / (-rate) * 60.0) as u64;
                            ui.label(egui::RichText::new(fmt_duration(secs)).size(24.0));
                            ui.label("Estime d'apres la tendance de decharge observee");
                        } else {
                            ui.label(egui::RichText::new("Stable / en charge").size(20.0));
                            ui.label("Pas de decharge en cours");
                        }
                    } else {
                        ui.label("Mesure en cours...");
                        ui.label("L'estimation apparaitra apres quelques secondes d'observation");
                    }
                    if let Some(w) = self.discharge_watts() {
                        ui.add_space(4.0);
                        ui.label(format!("Debit estime : {:.1} W", w));
                    }
                });

                ui.add_space(8.0);

                // ---- Carte sante ----
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.heading("Sante de la batterie");
                    ui.add_space(4.0);
                    if let Some(h) = &self.health {
                        let pct = h.health_pct();
                        let color = if pct >= 80.0 {
                            [80, 170, 90]
                        } else if pct >= 60.0 {
                            [230, 175, 50]
                        } else {
                            [225, 85, 75]
                        };
                        ui.add(
                            egui::ProgressBar::new(pct / 100.0)
                                .show_percentage()
                                .filled_color(egui::Color32::from_rgb(
                                    color[0], color[1], color[2],
                                )),
                        );
                        ui.add_space(4.0);
                        ui.label(format!(
                            "Capacite pleine : {} mWh  |  Capacite d'origine : {} mWh",
                            h.full_mwh, h.design_mwh
                        ));
                        let (text, c) = h.verdict();
                        ui.colored_label(egui::Color32::from_rgb(c[0], c[1], c[2]), text);
                    } else if self.health_msg.is_empty() {
                        ui.label("Lecture en cours...");
                    } else {
                        ui.colored_label(
                            egui::Color32::from_rgb(200, 90, 80),
                            &self.health_msg,
                        );
                    }
                });

                ui.add_space(8.0);

                // ---- Carte profils ----
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.heading("Profil d'alimentation");
                    ui.add_space(4.0);
                    ui.label(format!("Profil actif : {}", self.active_scheme));
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("Equilibre").clicked() {
                            self.apply(Profile::Balanced);
                        }
                        if ui.button("Econome").clicked() {
                            self.apply(Profile::Saver);
                        }
                        if ui.button("Performances").clicked() {
                            self.apply(Profile::Performance);
                        }
                    });
                    ui.add_space(4.0);
                    let wide = ui.available_width();
                    if ui
                        .add(egui::Button::new(
                            "Batterie prolongee (recommande sur batterie)",
                        )
                        .min_size(egui::vec2(wide, 0.0)))
                        .clicked()
                    {
                        self.apply(Profile::Extended);
                    }
                    if !self.status_msg.is_empty() {
                        ui.add_space(4.0);
                        let c = if self.status_ok { [80, 170, 90] } else { [225, 85, 75] };
                        ui.colored_label(egui::Color32::from_rgb(c[0], c[1], c[2]), &self.status_msg);
                    }
                });

                ui.add_space(8.0);

                // ---- Conseils ----
                egui::CollapsingHeader::new("Conseils pour durer plus longtemps")
                    .default_open(true)
                    .show(ui, |ui| {
                        for tip in [
                            "Reduisez la luminosite de l'ecran (le plus gros poste de conso).",
                            "Laissez le PC se refroidir : la chaleur degrade la batterie.",
                            "Limitez les applications lourdes en arriere-plan sur batterie.",
                            "Utilisez le Wi-Fi en 5 GHz proche, le signal faible consomme plus.",
                            "Ne laissez pas la batterie a 0% ni chargee a 100% inutilement.",
                            "Activez le mode Avion quand vous n'avez pas besoin du reseau.",
                        ] {
                            ui.label(format!("- {tip}"));
                        }
                    });

                ui.add_space(8.0);
                ui.separator();
                ui.label(
                    egui::RichText::new("Actualisation toutes les 2 s  |  BattGuard v0.1.0")
                        .weak()
                        .small(),
                );
                ui.add_space(4.0);
            });
        });
    }
}

impl eframe::App for BattApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        BattApp::update(self, ctx, frame)
    }
}

fn fmt_duration(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    if h > 0 {
        format!("{h} h {m:02} min")
    } else {
        format!("{m} min")
    }
}
