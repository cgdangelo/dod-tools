#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use analysis::{Analysis, Player, PlayerGlobalId, Round, SteamId, Team};
use egui::{
    Align, CentralPanel, CollapsingHeader, Color32, Context, Frame, Grid, Label, Layout,
    ProgressBar, ScrollArea, SidePanel, Sides, TopBottomPanel, Ui, Window, panel::Side,
};
use egui_extras::{Column, TableBody, TableBuilder};
use egui_file_dialog::FileDialog;
use egui_plot::{Corner, Legend, Line, Plot, PlotPoints};
use humantime::{format_duration, format_rfc3339_seconds};
use native::{FileInfo, run_analyzer};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, mpsc};
use std::time::Duration;

#[tokio::main]
async fn main() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_maximized(true),
        ..Default::default()
    };

    eframe::run_native(
        "dod-tools",
        options,
        Box::new(|_cc| Ok(Box::<Gui>::default())),
    )
    .expect("Could not run the GUI");
}

pub struct Gui {
    analyses: Vec<(FileInfo, Analysis)>,
    batch_progress: Option<(usize, usize)>,
    file_picker: FileDialog,
    open_windows: HashSet<String>,
    player_highlight: PlayerHighlighting,

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
        analysis: Box<Analysis>,
        file_info: FileInfo,
        progress: (usize, usize),
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
            open_windows: Default::default(),
            analyses: Default::default(),
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

            Ok(GuiMessage::AnalyzerProgress {
                file_info,
                progress,
                analysis,
            }) => {
                self.batch_progress = Some(progress);

                self.open_windows.insert(file_info.path.clone());

                self.analyses.push((file_info, *analysis));
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

            let demo_paths =
                Vec::from_iter(from_picker.into_iter().chain(from_drop).filter(|path| {
                    if let Some(path) = path.to_str().and_then(|str| String::from_str(str).ok()) {
                        !self
                            .analyses
                            .iter()
                            .any(|(file_info, _)| file_info.path == path)
                    } else {
                        false
                    }
                }));

            if !demo_paths.is_empty() {
                analyze_files_async(ctx.clone(), self.tx.clone(), demo_paths);
            }
        });

        TopBottomPanel::top("controls")
            .frame(Frame::side_top_panel(&ctx.style()).inner_margin(6.))
            .show(ctx, |ui| {
                Sides::default().show(
                    ui,
                    |ui| {
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            ui.menu_button("File ⏷", |ui| {
                                if ui.button("Open").clicked() {
                                    self.file_picker.pick_multiple();
                                }

                                ui.separator();

                                if ui.button("Quit").clicked() {
                                    std::process::exit(0);
                                }
                            });

                            if !self.analyses.is_empty() {
                                ui.separator();

                                if ui.button("Clear memory").clicked() {
                                    self.open_windows.clear();
                                    self.analyses.clear();
                                }

                                if ui.button("Organize windows").clicked() {
                                    ctx.memory_mut(|mem| mem.reset_areas());
                                }
                            };
                        });
                    },
                    |ui| {
                        egui::widgets::global_theme_preference_buttons(ui);
                    },
                );
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

        if !self.analyses.is_empty() {
            SidePanel::new(Side::Left, "open_reports")
                .frame(Frame::side_top_panel(&ctx.style()).inner_margin(6.))
                .show(ctx, |ui| {
                    ScrollArea::vertical().show(ui, |ui| {
                        ui.with_layout(Layout::top_down_justified(Align::LEFT), |ui| {
                            let mut reports = self.analyses.iter().peekable();

                            while let Some((file_info, _)) = reports.next() {
                                let title = file_info.path.clone();
                                let mut is_open = self.open_windows.contains(&title);

                                ui.toggle_value(&mut is_open, &title);

                                if !is_open {
                                    self.open_windows.remove(&title);
                                } else {
                                    self.open_windows.insert(title);
                                }

                                if reports.peek().is_some() {
                                    ui.separator();
                                }
                            }
                        });
                    });
                });
        }

        CentralPanel::default().show(ctx, |ui| {
            ui.centered_and_justified(|ui| {
                if self.analyses.is_empty() {
                    ui.heading("To start, drag and drop demos here or open with the File > Open menu.");
                } else if self.open_windows.is_empty() {
                    ui.heading(
                        "You still have demos open. Select one from the list on the left to re-open an existing demo, or you can add a new demo.",
                    );
                }
            });

            for (file_info, analysis) in &self.analyses {
                let demo_path = &file_info.path;
                let mut is_open = self.open_windows.contains(demo_path);

                Window::new(&file_info.name)
                    .id(demo_path.clone().into())
                    .default_height(600.)
                    .open(&mut is_open)
                    .show(ctx, |ui| {
                        report_ui(file_info, analysis, &mut self.player_highlight, ui);
                    });

                if !is_open {
                    self.open_windows.remove(demo_path);
                } else {
                    self.open_windows.insert(demo_path.clone());
                }
            }
        });
    }
}

const TABLE_ROW_HEIGHT: f32 = 18.;

const ALLIES_COLOR: Color32 = Color32::DARK_GREEN;

const AXIS_COLOR: Color32 = Color32::DARK_RED;

const NEUTRAL_COLOR: Color32 = Color32::WHITE;

fn report_ui(
    file_info: &FileInfo,
    r: &Analysis,
    player_highlighting: &mut PlayerHighlighting,
    ui: &mut Ui,
) {
    header_ui(file_info, r, ui);

    ui.separator();

    scoreboard_ui(r, player_highlighting, ui);

    ui.separator();

    team_score_timeline_ui(r, ui);

    ui.separator();

    rounds_ui(r, ui);

    ui.separator();

    player_summaries_ui(r, player_highlighting, ui);
}

fn header_ui(file_info: &FileInfo, analysis: &Analysis, ui: &mut Ui) {
    CollapsingHeader::new("Summary")
        .default_open(true)
        .show(ui, |ui| {
            Grid::new("header").show(ui, |ui| {
                ui.strong("File path");
                ui.monospace(&file_info.path);
                ui.end_row();

                ui.strong("File created at");
                ui.label(format_rfc3339_seconds(file_info.created_at).to_string());
                ui.end_row();

                ui.strong("Map name");
                ui.label(&analysis.demo_info.map_name);
                ui.end_row();

                ui.strong("Demo protocol");
                ui.label(analysis.demo_info.demo_protocol.to_string());
                ui.end_row();

                ui.strong("Network protocol");
                ui.label(analysis.demo_info.network_protocol.to_string());
                ui.end_row();

                ui.strong("Analyzer version");
                ui.label(env!("CARGO_PKG_VERSION"));
                ui.end_row();
            });
        });
}

fn scoreboard_ui(r: &Analysis, player_highlighting: &mut PlayerHighlighting, ui: &mut Ui) {
    let (allies_score, axis_score) = (
        r.state.team_scores.get_team_score(Team::Allies),
        r.state.team_scores.get_team_score(Team::Axis),
    );

    let match_result_fragment = format!(
        ": Allies ({}) {} Axis ({})",
        allies_score,
        if allies_score > axis_score { ">" } else { "<" },
        axis_score
    );

    CollapsingHeader::new(format!("Scoreboard: {match_result_fragment}"))
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
                    let mut players = Vec::from_iter(&r.state.players);

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

        row.col(|ui| match SteamId::try_from(&p.id) {
            Ok(steam_id) => {
                let link_text = steam_id.to_string();
                let link_url = format!("https://steamcommunity.com/profiles/{}", p.id);

                ui.hyperlink_to(link_text, link_url);
            }
            _ => {
                ui.label(p.id.to_string());
            }
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
                Some(x) => format!("{x:?}"),
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

fn team_score_timeline_ui(r: &Analysis, ui: &mut Ui) {
    CollapsingHeader::new("Timeline")
        .default_open(true)
        .show(ui, |ui| {
            let plot = Plot::new("timeline_plot")
                .allow_scroll(false)
                .height(200.)
                .width(ui.max_rect().width())
                .legend(Legend::default().position(Corner::LeftTop))
                .custom_x_axes(vec![]) // Remove the x-axis
                .custom_y_axes(vec![]) // Remove the y-axis
                .label_formatter(|team, point| {
                    if !team.is_empty() {
                        let duration = Duration::from_secs_f64(point.x);
                        let duration = Duration::new(duration.as_secs(), 0);

                        format!("{}\n{}: {}", format_duration(duration), team, point.y)
                    } else {
                        String::default()
                    }
                });

            plot.show(ui, |plot_ui| {
                let team_line_points = |team: Team| {
                    r.state
                        .team_scores
                        .iter()
                        .filter_map(move |(time, t, score)| {
                            if *t == team {
                                Some([time.offset.as_secs_f64(), *score as f64])
                            } else {
                                None
                            }
                        })
                };

                let points = team_line_points(Team::Allies);
                let line = Line::new(PlotPoints::from_iter(points))
                    .color(ALLIES_COLOR)
                    .name("Allies");

                plot_ui.line(line);

                let points = team_line_points(Team::Axis);
                let line = Line::new(PlotPoints::from_iter(points))
                    .color(AXIS_COLOR)
                    .name("Axis");

                plot_ui.line(line);
            });
        });
}

fn rounds_ui(r: &Analysis, ui: &mut Ui) {
    CollapsingHeader::new("Rounds").show(ui, |ui| {
        let table = TableBuilder::new(ui)
            .striped(true)
            .cell_layout(Layout::left_to_right(Align::Center))
            .columns(Column::auto(), 6);

        table
            .header(TABLE_ROW_HEIGHT, |mut ui| {
                ui.col(|ui| {
                    ui.add_space(ui.style().spacing.indent);
                });
                ui.col(|ui| {
                    ui.strong("#");
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
                let mut match_duration = Duration::default();

                for (i, round) in r.state.rounds.iter().enumerate() {
                    if let Round::Completed {
                        start_time,
                        end_time,
                        winner_stats,
                    } = round
                    {
                        match_duration += end_time.offset - start_time.offset;

                        ui.row(TABLE_ROW_HEIGHT, |mut row| {
                            row.col(|ui| {
                                ui.painter().rect_filled(
                                    ui.max_rect(),
                                    0.0,
                                    match winner_stats {
                                        Some((Team::Allies, _)) => ALLIES_COLOR,
                                        Some((Team::Axis, _)) => AXIS_COLOR,
                                        _ => NEUTRAL_COLOR,
                                    },
                                );
                            });

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

fn player_summaries_ui(r: &Analysis, player_highlighting: &PlayerHighlighting, ui: &mut Ui) {
    let mut players = Vec::from_iter(&r.state.players);

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
        .columns(Column::auto(), 5)
        .header(TABLE_ROW_HEIGHT, |mut row| {
            row.col(|ui| {
                ui.strong("Weapon");
            });
            row.col(|ui| {
                ui.strong("Kills");
            });
            row.col(|ui| {
                ui.strong("% of Total");
            });
            row.col(|ui| {
                ui.strong("Team Kills");
            });
            row.col(|ui| {
                ui.strong("% of Total");
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
                        ui.label(format!("{weapon:?}"));
                    });

                    row.col(|ui| {
                        ui.label(format!("{kills}"));
                    });

                    row.col(|ui| {
                        let pct_of_total = if kills + total_kills > 0 {
                            ((*kills as f32 / total_kills as f32) * 100.).floor()
                        } else {
                            0.
                        };

                        ui.label(format!("{pct_of_total}%"));
                    });

                    row.col(|ui| {
                        ui.label(format!("{teamkills}",));
                    });

                    row.col(|ui| {
                        let pct_of_total = if teamkills + total_teamkills > 0 {
                            ((*teamkills as f32 / total_teamkills as f32) * 100.).floor()
                        } else {
                            0.
                        };

                        ui.label(format!("{pct_of_total}%"));
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
                                .map(|(_, weapon)| format!("{weapon:?}"))
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

        for (index, demo_path) in paths.iter().enumerate() {
            let (file_info, analysis) = run_analyzer(demo_path);

            tx.send(GuiMessage::AnalyzerProgress {
                file_info,
                progress: (index + 1, paths.len()),
                analysis: Box::new(analysis),
            })
            .unwrap();

            ctx.request_repaint();
        }

        tx.send(GuiMessage::Idle).unwrap();
    });
}
