use crate::run_analyzer;
use eframe::Frame;
use egui::Context;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use egui_file_dialog::FileDialog;
use std::fmt::Write;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc};

pub struct Gui {
    batch_progress: Option<(usize, usize)>,
    demo_picker: FileDialog,
    markdown_cache: CommonMarkCache,
    output_picker: FileDialog,
    textbox_contents: Option<String>,
    rx: Receiver<GuiMessage>,
    tx: Sender<GuiMessage>,
}

enum GuiMessage {
    Idle,

    AnalyzerStart {
        files_count: usize,
    },

    AnalyzerProgress {
        progress: (usize, usize),
        report: String,
    },
}

impl Default for Gui {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();

        Self {
            batch_progress: None,
            demo_picker: FileDialog::new()
                .add_file_filter(
                    "Demo files (*.dem)",
                    Arc::new(|file| file.extension().is_some_and(|ext| ext == "dem")),
                )
                .default_file_filter("Demo files (*.dem)"),
            output_picker: FileDialog::new(),
            markdown_cache: CommonMarkCache::default(),
            textbox_contents: None,
            rx,
            tx,
        }
    }
}

impl eframe::App for Gui {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        match self.rx.try_recv() {
            Ok(GuiMessage::Idle) => {
                self.batch_progress = None;
            }
            Ok(GuiMessage::AnalyzerStart { files_count }) => {
                self.textbox_contents = None;
                self.batch_progress = Some((0, files_count));
            }
            Ok(GuiMessage::AnalyzerProgress { progress, report }) => {
                self.batch_progress = Some(progress);
                self.textbox_contents = if let Some(t) = &self.textbox_contents {
                    Some(format!("{}\n{}", t, report))
                } else {
                    Some(report)
                };
            }
            _ => {}
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Open file(s)").clicked() {
                    self.demo_picker.pick_multiple();
                }

                if let Some((current, total)) = &self.batch_progress {
                    ui.label(format!("Finished: {} of {}", current, total));
                }

                if let Some(text) = &self.textbox_contents {
                    if ui.button("Copy to Clipboard").clicked() {
                        ctx.copy_text(text.clone());
                    }

                    if ui.button("Save File").clicked() {
                        self.output_picker.save_file();
                    }
                }
            });

            self.demo_picker.update(ctx);
            self.output_picker.update(ctx);

            let analyze_files = |demo_paths: Vec<PathBuf>| {
                analyze_files_async(ctx.clone(), self.tx.clone(), demo_paths);
            };

            if let Some(demo_paths) = self.demo_picker.take_picked_multiple() {
                analyze_files(demo_paths);
            }

            ctx.input(|i| {
                if !i.raw.dropped_files.is_empty() {
                    let demo_paths = i
                        .raw
                        .dropped_files
                        .iter()
                        .filter_map(|dropped_file| dropped_file.path.clone())
                        .collect::<Vec<PathBuf>>();

                    analyze_files(demo_paths);
                }
            });

            if let (Some(text), Some(output_path)) =
                (&self.textbox_contents, self.output_picker.take_picked())
            {
                let _ = fs::write(output_path, text);
            }

            if let Some(ref mut report_text) = &mut self.textbox_contents {
                CommonMarkViewer::new().show_scrollable(
                    &report_text,
                    ui,
                    &mut self.markdown_cache,
                    report_text,
                );
            }
        });
    }
}

fn analyze_files_async(ctx: Context, tx: Sender<GuiMessage>, paths: Vec<PathBuf>) {
    tokio::spawn(async move {
        tx.send(GuiMessage::AnalyzerStart {
            files_count: paths.len(),
        })
        .unwrap();

        for (index, file) in paths.iter().enumerate() {
            let mut report = String::new();

            let _ = write!(report, "{}", run_analyzer(file));

            tx.send(GuiMessage::AnalyzerProgress {
                progress: (index + 1, paths.len()),
                report,
            })
            .unwrap();

            ctx.request_repaint();
        }

        tx.send(GuiMessage::Idle).unwrap();
    });
}
