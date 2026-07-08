use crate::simulation::{SimulationSettings, SimulationStats};

#[derive(Debug, Clone)]
pub struct UiState {
    pub show_trails: bool,
    pub show_neighbor_radius: bool,
    pub show_bounds: bool,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            show_trails: true,
            show_neighbor_radius: false,
            show_bounds: true,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct UiAction {
    pub reset: bool,
    pub randomize: bool,
}

pub fn draw(
    ctx: &egui::Context,
    settings: &mut SimulationSettings,
    stats: SimulationStats,
    ui_state: &mut UiState,
) -> UiAction {
    let mut action = UiAction::default();

    egui::TopBottomPanel::top("top_status")
        .frame(
            egui::Frame::none()
                .fill(egui::Color32::from_rgba_unmultiplied(8, 11, 18, 196))
                .inner_margin(egui::Margin::symmetric(14.0, 8.0)),
        )
        .show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("Flock Lab");
                ui.separator();
                ui.label(format!("{} agents", settings.agent_count));
                ui.label(format!("{:.1} avg speed", stats.average_speed));
                ui.label(format!("{} neighbour samples", stats.neighbor_samples));
                ui.separator();
                if ui
                    .button(if settings.pause { "Resume" } else { "Pause" })
                    .clicked()
                {
                    settings.pause = !settings.pause;
                }
                if ui.button("Reset").clicked() {
                    action.reset = true;
                }
                if ui.button("Randomise").clicked() {
                    action.randomize = true;
                }
            });
        });

    egui::SidePanel::right("controls")
        .resizable(false)
        .default_width(300.0)
        .frame(
            egui::Frame::none()
                .fill(egui::Color32::from_rgba_unmultiplied(9, 13, 22, 224))
                .inner_margin(egui::Margin::same(16.0)),
        )
        .show(ctx, |ui| {
            ui.heading("Simulation");
            ui.add_space(8.0);
            ui.add(egui::Slider::new(&mut settings.agent_count, 100..=20_000).text("Agent count"));
            ui.add(egui::Slider::new(&mut settings.max_speed, 1.0..=60.0).text("Speed"));
            ui.add(egui::Slider::new(&mut settings.bounds, 30.0..=220.0).text("Bounds"));
            ui.add(egui::Slider::new(&mut settings.trail_length, 0..=64).text("Trail length"));

            ui.add_space(12.0);
            ui.heading("Behaviour");
            ui.add(egui::Slider::new(&mut settings.separation_strength, 0.0..=8.0).text("Separation"));
            ui.add(egui::Slider::new(&mut settings.alignment_strength, 0.0..=6.0).text("Alignment"));
            ui.add(egui::Slider::new(&mut settings.cohesion_strength, 0.0..=6.0).text("Cohesion"));
            ui.add(egui::Slider::new(&mut settings.goal_strength, 0.0..=2.0).text("Goal seeking"));
            ui.add(
                egui::Slider::new(&mut settings.predator_avoidance_strength, 0.0..=12.0)
                    .text("Predator avoidance"),
            );
            ui.add(
                egui::Slider::new(&mut settings.obstacle_avoidance_strength, 0.0..=12.0)
                    .text("Obstacle avoidance"),
            );

            ui.add_space(12.0);
            ui.heading("World");
            ui.add(egui::Slider::new(&mut settings.predator_count, 0..=12).text("Predators"));
            ui.add(egui::Slider::new(&mut settings.obstacle_count, 0..=48).text("Obstacles"));
            ui.add(egui::Slider::new(&mut settings.neighbor_radius, 2.0..=24.0).text("Neighbour radius"));
            settings.separation_radius = settings.separation_radius.min(settings.neighbor_radius);
            ui.add(
                egui::Slider::new(&mut settings.separation_radius, 0.5..=settings.neighbor_radius)
                    .text("Separation radius"),
            );

            ui.add_space(12.0);
            ui.heading("View");
            ui.checkbox(&mut ui_state.show_trails, "Trails");
            ui.checkbox(&mut ui_state.show_bounds, "Bounds");
            ui.checkbox(&mut ui_state.show_neighbor_radius, "Neighbour radius debug");
        });

    action
}
