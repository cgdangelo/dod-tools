use crate::analysis::{Player, PlayerGlobalId, Round};
use crate::dod::Team;
use crate::reporting::Report;
use crate::run_analyzer;
use egui::{
    panel::Side, Align, CentralPanel, CollapsingHeader, Context, Frame, Grid, Label, Layout,
    ProgressBar, ScrollArea, SidePanel, TopBottomPanel, Ui, Window,
};
use egui_extras::{Column, TableBody, TableBuilder};
use egui_file_dialog::FileDialog;
use humantime::{format_duration, format_rfc3339_seconds};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{mpsc, Arc};
use std::time::Duration;

pub struct Gui {
    batch_progress: Option<(usize, usize)>,
    file_picker: FileDialog,
    open_reports: HashSet<String>,
    player_highlight: PlayerHighlighting,
    reports: Vec<Report>,

    rx: mpsc::Receiver<GuiMessage>,
    tx: mpsc::Sender<GuiMessage>,
}

#[derive(Default)]
struct PlayerHighlighting {
    highlighted: HashSet<PlayerGlobalId>,
}

enum GuiMessage {
    Idle,

    AnalyzerStart {
        files: usize,
    },

    AnalyzerProgress {
        progress: (usize, usize),
        report: Box<Report>,
    },
}

impl Default for Gui {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();

        Self {
            batch_progress: Default::default(),

            file_picker: FileDialog::default()
                .add_file_filter(
                    "Demo files (*.dem)",
                    Arc::new(|path| path.extension().unwrap_or_default() == "dem"),
                )
                .default_file_filter("Demo files (*.dem)"),

            player_highlight: Default::default(),
            open_reports: Default::default(),
            reports: Default::default(),
            rx,
            tx,
        }
    }
}

impl eframe::App for Gui {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        match self.rx.try_recv() {
            Ok(GuiMessage::Idle) => {
                self.batch_progress = None;
            }
            Ok(GuiMessage::AnalyzerStart { files }) => {
                self.batch_progress = Some((0, files));
            }
            Ok(GuiMessage::AnalyzerProgress { progress, report }) => {
                self.batch_progress = Some(progress);

                let title = get_report_title(&report);
                self.open_reports.insert(title);

                self.reports.push(*report);
            }
            _ => {}
        }

        self.file_picker.update(ctx);

        ctx.input(|i| {
            let from_picker = self.file_picker.take_picked_multiple().unwrap_or_default();

            let from_drop = i
                .raw
                .dropped_files
                .iter()
                .filter_map(|dropped_file| dropped_file.path.clone())
                .collect::<Vec<PathBuf>>();

            let demo_paths = Vec::from_iter(from_picker.into_iter().chain(from_drop));

            if !demo_paths.is_empty() {
                analyze_files_async(ctx.clone(), self.tx.clone(), demo_paths);
            }
        });

        TopBottomPanel::top("controls")
            .frame(Frame::side_top_panel(&ctx.style()).inner_margin(6.))
            .show(ctx, |ui| {
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("Open").clicked() {
                            self.file_picker.pick_multiple();
                        }

                        ui.separator();

                        if ui.button("Quit").clicked() {
                            std::process::exit(0);
                        }
                    });

                    if !self.reports.is_empty() {
                        ui.separator();

                        if ui.button("Clear Memory").clicked() {
                            self.open_reports.clear();
                            self.reports.clear();
                        }

                        if ui.button("Organize Windows").clicked() {
                            ctx.memory_mut(|mem| mem.reset_areas());
                        }
                    };
                });
            });

        if let Some(batch_progress) = self.batch_progress {
            TopBottomPanel::bottom("status")
                .frame(Frame::side_top_panel(&ctx.style()).inner_margin(6.))
                .show(ctx, |ui| {
                    let bar_progress = (batch_progress.0 + 1) as f32 / batch_progress.1 as f32;
                    let bar_label = format!(
                        "Analyzing: {} of {}",
                        batch_progress.0 + 1,
                        batch_progress.1
                    );

                    ui.add(
                        ProgressBar::new(bar_progress)
                            .show_percentage()
                            .text(bar_label),
                    );
                });
        }

        SidePanel::new(Side::Left, "open_reports")
            .frame(Frame::side_top_panel(&ctx.style()).inner_margin(6.))
            .show(ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    ui.with_layout(Layout::top_down_justified(Align::LEFT), |ui| {
                        let mut reports = self.reports.iter().peekable();

                        while let Some(r) = reports.next() {
                            let title = get_report_title(r);
                            let mut is_open = self.open_reports.contains(&title);

                            ui.toggle_value(&mut is_open, &title);

                            if !is_open {
                                self.open_reports.remove(&title);
                            } else {
                                self.open_reports.insert(title);
                            }

                            if reports.peek().is_some() {
                                ui.separator();
                            }
                        }
                    });
                });
            });

        CentralPanel::default().show(ctx, |ui| {
            ui.centered_and_justified(|ui| {
                if self.reports.is_empty() {
                    ui.heading("To start, drag and drop demos here or open with the File > Open menu.");
                } else if self.open_reports.is_empty() {
                    ui.heading(
                        "You still have demos open. Select one from the list on the left to re-open an existing demo, or you can add a new demo.",
                    );
                }
            });

            for r in &self.reports {
                let title = get_report_title(r);
                let mut is_open = self.open_reports.contains(&title);

                Window::new(&title)
                    .default_height(600.)
                    .open(&mut is_open)
                    .show(ctx, |ui| {
                        report_ui(r, &mut self.player_highlight, ui);
                    });

                if !is_open {
                    self.open_reports.remove(&title);
                } else {
                    self.open_reports.insert(title);
                }
            }
        });
    }
}

const TABLE_ROW_HEIGHT: f32 = 18.;

fn get_report_title(r: &Report) -> String {
    format!("{} ({})", &r.file_info.name, &r.demo_info.map_name)
}

fn report_ui(r: &Report, player_highlighting: &mut PlayerHighlighting, ui: &mut Ui) {
    header_ui(r, ui);

    ui.separator();

    scoreboard_ui(r, player_highlighting, ui);

    ui.separator();

    rounds_ui(r, ui);

    ui.separator();

    player_summaries_ui(r, player_highlighting, ui);
}

fn header_ui(r: &Report, ui: &mut Ui) {
    CollapsingHeader::new("Summary")
        .default_open(true)
        .show(ui, |ui| {
            Grid::new("header").show(ui, |ui| {
                ui.strong("File path");
                ui.monospace(&r.file_info.path);
                ui.end_row();

                ui.strong("File created at");
                ui.label(format_rfc3339_seconds(r.file_info.created_at).to_string());
                ui.end_row();

                ui.strong("Demo protocol");
                ui.label(r.demo_info.demo_protocol.to_string());
                ui.end_row();

                ui.strong("Network protocol");
                ui.label(r.demo_info.network_protocol.to_string());
                ui.end_row();

                ui.strong("Analyzer version");
                ui.label(env!("CARGO_PKG_VERSION"));
                ui.end_row();
            });
        });
}

fn scoreboard_ui(r: &Report, player_highlighting: &mut PlayerHighlighting, ui: &mut Ui) {
    let match_result_fragment = match (
        r.analysis.team_scores.get(&Team::Allies),
        r.analysis.team_scores.get(&Team::Axis),
    ) {
        (Some(allies_score), Some(axis_score)) => {
            format!(
                "Allies ({}) {} Axis ({})",
                allies_score,
                if allies_score > axis_score { ">" } else { "<" },
                axis_score
            )
        }
        _ => String::new(),
    };

    CollapsingHeader::new(format!("Scoreboard: {}", match_result_fragment))
        .default_open(true)
        .show(ui, |ui| {
            let table = TableBuilder::new(ui)
                .striped(true)
                .cell_layout(Layout::left_to_right(Align::Center))
                .max_scroll_height(260.)
                .column(Column::auto())
                .column(Column::auto_with_initial_suggestion(150.))
                .columns(Column::auto(), 6);

            table
                .header(TABLE_ROW_HEIGHT, |mut header| {
                    let columns = [
                        "", "ID", "Name", "Team", "Class", "Score", "Kills", "Deaths",
                    ];

                    for column in columns {
                        header.col(|ui| {
                            ui.strong(column);
                        });
                    }
                })
                .body(|ref mut body| {
                    // Players sorted by team > score > kills
                    let mut players = Vec::from_iter(&r.analysis.players);

                    players.sort_by(|left, right| match (&left.team, &right.team) {
                        (Some(left_team), Some(right_team)) if left_team == right_team => {
                            if left.stats.0 == right.stats.0 {
                                left.stats.1.cmp(&right.stats.1).reverse()
                            } else {
                                left.stats.0.cmp(&right.stats.0).reverse()
                            }
                        }

                        (Some(Team::Allies), _) => Ordering::Less,
                        (Some(Team::Axis), Some(Team::Spectators)) => Ordering::Less,
                        (Some(Team::Spectators) | None, _) => Ordering::Greater,

                        _ => Ordering::Equal,
                    });

                    for p in players {
                        scoreboard_row_ui(p, player_highlighting, body);
                    }
                });
        });
}

fn scoreboard_row_ui(
    p: &Player,
    player_highlighting: &mut PlayerHighlighting,
    body: &mut TableBody,
) {
    let row_label = |ui: &mut Ui, str: &str| {
        ui.add(Label::new(str).extend());
    };

    body.row(TABLE_ROW_HEIGHT, |mut row| {
        let mut is_checked = player_highlighting.highlighted.contains(&p.id);

        row.set_selected(is_checked);

        row.col(|ui| {
            if ui.checkbox(&mut is_checked, "").changed() {
                if is_checked {
                    player_highlighting.highlighted.insert(p.id.clone());
                } else {
                    player_highlighting.highlighted.remove(&p.id);
                }
            }
        });

        row.col(|ui| {
            let link_text = p.id.as_steam_id().unwrap_or(p.id.to_string());
            let link_url = format!("https://steamcommunity.com/profiles/{}", p.id);

            ui.hyperlink_to(link_text, link_url);
        });

        row.col(|ui| {
            row_label(ui, &p.name);
        });

        row.col(|ui| {
            ui.label(match &p.team {
                None => "Unknown",
                Some(Team::Allies) => "Allies",
                Some(Team::Axis) => "Axis",
                Some(Team::Spectators) => "Spectators",
            });
        });

        row.col(|ui| {
            ui.label(match &p.class {
                None => "Unknown".to_string(),
                Some(x) => format!("{:?}", x),
            });
        });

        row.col(|ui| {
            ui.label(p.stats.0.to_string());
        });

        row.col(|ui| {
            ui.label(p.stats.1.to_string());
        });

        row.col(|ui| {
            ui.label(p.stats.2.to_string());
        });
    });
}

fn rounds_ui(r: &Report, ui: &mut Ui) {
    CollapsingHeader::new("Rounds").show(ui, |ui| {
        let table = TableBuilder::new(ui)
            .striped(true)
            .cell_layout(Layout::left_to_right(Align::Center))
            .columns(Column::auto(), 5);

        table
            .header(TABLE_ROW_HEIGHT, |mut ui| {
                ui.col(|ui| {
                    ui.strong("Round");
                });
                ui.col(|ui| {
                    ui.strong("Start Time");
                });
                ui.col(|ui| {
                    ui.strong("Duration");
                });
                ui.col(|ui| {
                    ui.strong("Winner");
                });
                ui.col(|ui| {
                    ui.strong("Kills by Winner");
                });
            })
            .body(|mut ui| {
                let mut rounds = r.analysis.rounds.iter().enumerate();

                let mut match_duration = Duration::default();

                while let Some((
                    i,
                    Round::Completed {
                        start_time,
                        end_time,
                        winner_stats,
                    },
                )) = rounds.next()
                {
                    match_duration += end_time.offset - start_time.offset;

                    ui.row(TABLE_ROW_HEIGHT, |mut row| {
                        row.col(|ui| {
                            ui.label((i + 1).to_string());
                        });

                        row.col(|ui| {
                            let start_time =
                                Duration::from_millis(start_time.offset.as_millis() as u64);

                            ui.label(format_duration(start_time).to_string());
                        });

                        row.col(|ui| {
                            let duration = Duration::from_millis(
                                (end_time.offset - start_time.offset).as_millis() as u64,
                            );

                            ui.label(format_duration(duration).to_string());
                        });

                        if let Some((winner, kills)) = winner_stats {
                            row.col(|ui| {
                                ui.label(if matches!(winner, Team::Allies) {
                                    "Allies"
                                } else {
                                    "Axis"
                                });
                            });

                            row.col(|ui| {
                                ui.label(kills.to_string());
                            });
                        } else {
                            row.col(|_ui| {});
                            row.col(|_ui| {});
                        }
                    });
                }

                ui.row(TABLE_ROW_HEIGHT, |mut row| {
                    row.col(|_| {});
                    row.col(|_| {});
                    row.col(|ui| {
                        ui.label(format_duration(match_duration).to_string());
                    });
                    row.col(|_| {});
                });
            });
    });
}

fn player_summaries_ui(r: &Report, player_highlighting: &PlayerHighlighting, ui: &mut Ui) {
    let mut players = Vec::from_iter(&r.analysis.players);

    players.sort_by(|l, r| l.name.cmp(&r.name));

    ScrollArea::vertical()
        .auto_shrink(false)
        .min_scrolled_height(260.)
        .show(ui, |ui| {
            for p in players {
                if !player_highlighting.highlighted.is_empty()
                    && !player_highlighting.highlighted.contains(&p.id)
                {
                    continue;
                }

                CollapsingHeader::new(&p.name)
                    .default_open(false)
                    .show(ui, |ui| {
                        weapon_breakdown_ui(p, ui);
                        kill_streaks_ui(p, ui);
                    });
            }
        });
}

fn weapon_breakdown_ui(p: &Player, ui: &mut Ui) {
    CollapsingHeader::new("Weapon Breakdown")
        .default_open(false)
        .show(ui, |ui| {
            weapon_breakdown_table_ui(p, ui);
        });
}

fn weapon_breakdown_table_ui(p: &Player, ui: &mut Ui) {
    let mut weapon_breakdown = Vec::from_iter(&p.weapon_breakdown);

    weapon_breakdown.sort_by(|(_, l), (_, r)| l.cmp(r).reverse());

    TableBuilder::new(ui)
        .striped(true)
        .cell_layout(Layout::left_to_right(Align::Center))
        .columns(Column::auto(), 3)
        .header(TABLE_ROW_HEIGHT, |mut row| {
            row.col(|ui| {
                ui.strong("Weapon");
            });
            row.col(|ui| {
                ui.strong("Kills");
            });
            row.col(|ui| {
                ui.strong("Team Kills");
            });
        })
        .body(|mut body| {
            let (total_kills, total_teamkills) = weapon_breakdown
                .iter()
                .fold((0, 0), |(k_sum, tk_sum), (_, (k, tk))| {
                    (k_sum + k, tk_sum + tk)
                });

            for (weapon, (kills, teamkills)) in weapon_breakdown {
                body.row(TABLE_ROW_HEIGHT, |mut row| {
                    row.col(|ui| {
                        ui.label(format!("{:?}", weapon));
                    });

                    row.col(|ui| {
                        ui.label(format!(
                            "{} ({}%)",
                            kills,
                            if kills + total_kills > 0 {
                                ((*kills as f32 / total_kills as f32) * 100.).floor()
                            } else {
                                0.
                            }
                        ));
                    });

                    row.col(|ui| {
                        ui.label(format!(
                            "{} ({}%)",
                            teamkills,
                            if teamkills + total_teamkills > 0 {
                                ((*teamkills as f32 / total_teamkills as f32) * 100.).floor()
                            } else {
                                0.
                            }
                        ));
                    });
                });
            }
        });
}

fn kill_streaks_ui(p: &Player, ui: &mut Ui) {
    CollapsingHeader::new("Kill Streaks")
        .default_open(false)
        .show(ui, |ui| {
            kill_streaks_table_ui(p, ui);
        });
}

fn kill_streaks_table_ui(p: &Player, ui: &mut Ui) {
    TableBuilder::new(ui)
        .striped(true)
        .cell_layout(Layout::left_to_right(Align::Center))
        .columns(Column::auto(), 5)
        .header(TABLE_ROW_HEIGHT, |mut row| {
            row.col(|ui| {
                ui.strong("Wave");
            });
            row.col(|ui| {
                ui.strong("Total Kills");
            });
            row.col(|ui| {
                ui.strong("Start Time");
            });
            row.col(|ui| {
                ui.strong("Duration");
            });
            row.col(|ui| {
                ui.strong("Weapons Used");
            });
        })
        .body(|mut body| {
            for (wave, streak) in p.kill_streaks.iter().enumerate() {
                if let (Some((start, _)), Some((end, _))) =
                    (streak.kills.first(), streak.kills.last())
                {
                    body.row(TABLE_ROW_HEIGHT, |mut row| {
                        row.col(|ui| {
                            ui.label((wave + 1).to_string());
                        });

                        row.col(|ui| {
                            ui.label(streak.kills.len().to_string());
                        });

                        row.col(|ui| {
                            let start = Duration::new(start.offset.as_secs(), 0);

                            ui.label(format_duration(start).to_string());
                        });

                        row.col(|ui| {
                            let duration = Duration::new((end.offset - start.offset).as_secs(), 0);

                            ui.label(format_duration(duration).to_string());
                        });

                        row.col(|ui| {
                            let weapons = streak
                                .kills
                                .iter()
                                .map(|(_, weapon)| format!("{:?}", weapon))
                                .collect::<Vec<_>>()
                                .join(", ");

                            ui.label(weapons);
                        });
                    });
                }
            }
        });
}

fn analyze_files_async(ctx: Context, tx: mpsc::Sender<GuiMessage>, paths: Vec<PathBuf>) {
    tokio::spawn(async move {
        tx.send(GuiMessage::AnalyzerStart { files: paths.len() })
            .unwrap();

        for (index, file) in paths.iter().enumerate() {
            let report = run_analyzer(file);

            tx.send(GuiMessage::AnalyzerProgress {
                progress: (index + 1, paths.len()),
                report: Box::new(report),
            })
            .unwrap();

            ctx.request_repaint();
        }

        tx.send(GuiMessage::Idle).unwrap();
    });
}
