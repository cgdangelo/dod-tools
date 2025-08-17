//! Demo analyzer that runs in a terminal and produces text output.

use analysis::{Analysis, Round, SteamId, Team};
use clap::{Parser, ValueEnum};
use humantime::{format_duration, format_rfc3339_seconds};
use native::{FileInfo, run_analyzer};
use serde_json::{Value, json};
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use tabled::{builder::Builder, settings::Style};

fn main() {
    let args = Args::parse();

    let analyses = args.demo_paths.iter().map(run_analyzer);

    match args.output_format {
        OutputFormat::Json => println!("{}", Json::from_iter(analyses)),

        OutputFormat::Markdown => analyses.map(Markdown::from).for_each(|output| {
            println!("{output}");
        }),
    };
}

#[derive(Debug, Parser)]
#[command(version)]
struct Args {
    /// List of paths to demo files
    demo_paths: Vec<PathBuf>,

    /// The kind of string output to produce from an analysis
    #[arg(long, value_enum, default_value_t = OutputFormat::Markdown)]
    output_format: OutputFormat,
}

#[derive(Clone, Debug, ValueEnum)]
enum OutputFormat {
    /// Markdown document best used in combination with a Markdown renderer
    Markdown,

    /// JSON string for automated tools or custom visualization
    Json,
}

type AnalyzerOutput = (FileInfo, Analysis);

struct Json(Value);

impl FromIterator<AnalyzerOutput> for Json {
    fn from_iter<T: IntoIterator<Item = AnalyzerOutput>>(iter: T) -> Self {
        let analyses = iter.into_iter();

        let json = analyses.fold(vec![], |mut acc, (file, analysis)| {
            let players = analysis
                .state
                .players
                .iter()
                .map(|player| {
                    let id = SteamId::try_from(&player.id)
                        .map(|steam_id| steam_id.to_string())
                        .ok()
                        .unwrap_or(player.id.to_string());

                    json!({
                        "id": id,
                        "name": player.name,
                        "team": player.team.clone().map(|t| format!("{t:?}").to_lowercase()),
                        "score": player.stats.0,
                        "kills": player.stats.1,
                        "deaths": player.stats.2,
                    })
                })
                .collect::<Vec<_>>();

            acc.push(json!({
                "file": file.path,

                "teams": {
                    "allies": analysis.state.team_scores.get_team_score(Team::Allies),
                    "axis": analysis.state.team_scores.get_team_score(Team::Axis),
                },

                "players": players,
            }));

            acc
        });

        json!(json).into()
    }
}

impl From<Value> for Json {
    fn from(value: Value) -> Self {
        Self(value)
    }
}

impl Display for Json {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str = serde_json::to_string_pretty(&self.0).map_err(|_| std::fmt::Error)?;

        f.write_str(&str)
    }
}

struct Markdown(FileInfo, Analysis);

impl From<AnalyzerOutput> for Markdown {
    fn from(value: AnalyzerOutput) -> Self {
        Self(value.0, value.1)
    }
}

impl Markdown {
    fn md_escape(str: &str) -> String {
        str.replace("|", r"\|")
            .replace("_", r"\_")
            .replace("*", r"\*")
            .replace("[", r"\[")
            .replace("]", r"\]")
    }
}

impl Display for Markdown {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // Header section
        {
            let file_name = &self.0.name;
            let map_name = &self.1.demo_info.map_name;
            writeln!(f, "# Summary: {file_name} on {map_name}\n")?;

            let file_path = &self.0.path;
            writeln!(f, "- File path: `{file_path}`")?;
            let file_created_at = format_rfc3339_seconds(self.0.created_at);
            writeln!(f, "- File created at: {file_created_at}")?;
            let demo_protocol = &self.1.demo_info.demo_protocol;
            writeln!(f, "- Demo protocol: {demo_protocol}")?;
            let network_protocol = &self.1.demo_info.network_protocol;
            writeln!(f, "- Network protocol: {network_protocol}")?;
            let app_version = env!("CARGO_PKG_VERSION");
            writeln!(f, "- Analyzer version: {app_version}")?;
            let report_created_at = format_rfc3339_seconds(SystemTime::now());
            writeln!(f, "- Report created at: {report_created_at}")?;
        }

        writeln!(f)?;

        // Player scoreboard section
        {
            let mut table_builder = Builder::default();
            table_builder.push_record(["ID", "Name", "Team", "Class", "Score", "Kills", "Deaths"]);

            for player in &self.1.state.players {
                table_builder.push_record([
                    player.id.to_string(),
                    Self::md_escape(&player.name),
                    match &player.team {
                        None => "Unknown",
                        Some(Team::Allies) => "Allies",
                        Some(Team::Axis) => "Axis",
                        Some(Team::Spectators) => "Spectators",
                    }
                    .to_string(),
                    match &player.class {
                        None => "Unknown".to_string(),
                        Some(x) => format!("{x:?}"),
                    },
                    player.stats.0.to_string(),
                    player.stats.1.to_string(),
                    player.stats.2.to_string(),
                ]);
            }

            let (allies_score, axis_score) = (
                self.1.state.team_scores.get_team_score(Team::Allies),
                self.1.state.team_scores.get_team_score(Team::Axis),
            );

            let match_result_fragment = format!(
                ": Allies ({}) {} Axis ({})",
                allies_score,
                if allies_score > axis_score { ">" } else { "<" },
                axis_score
            );

            writeln!(f, "## Scoreboard{match_result_fragment}\n")?;

            let mut table = table_builder.build();
            table.with(Style::markdown());

            writeln!(f, "{table}")?;
        }

        writeln!(f)?;

        // Rounds section
        {
            let mut table_builder = Builder::default();
            table_builder.push_record([
                "Round",
                "Start Time",
                "Duration",
                "Winner",
                "Kills by Winner",
            ]);

            let mut rounds = self.1.state.rounds.iter().enumerate();

            while let Some((
                i,
                Round::Completed {
                    start_time,
                    end_time,
                    winner_stats,
                },
            )) = rounds.next()
            {
                let duration = Duration::new((end_time - start_time).as_secs(), 0);
                let start_time = Duration::new(start_time.viewdemo_offset.as_secs(), 0);

                table_builder.push_record([
                    (i + 1).to_string(),
                    format_duration(start_time).to_string(),
                    format_duration(duration).to_string(),
                    if let Some((winner, _)) = winner_stats {
                        format!("{winner:?}")
                    } else {
                        String::new()
                    },
                    if let Some((_, kills)) = winner_stats {
                        kills.to_string()
                    } else {
                        String::new()
                    },
                ]);
            }

            writeln!(f, "## Rounds\n")?;

            let mut table = table_builder.build();
            table.with(Style::markdown());

            writeln!(f, "{table}")?;
        }

        writeln!(f)?;

        // Individual player summaries
        {
            writeln!(f, "## Player Summaries\n")?;

            for player in &self.1.state.players {
                writeln!(f, "### {}\n", Self::md_escape(&player.name))?;

                // Kills per weapon section
                writeln!(f, "#### Weapon Breakdown\n")?;

                let mut table_builder = Builder::default();
                table_builder.push_record(["Weapon", "Kills", "Team Kills"]);

                for (weapon, (kills, teamkills)) in player.weapon_breakdown.iter() {
                    table_builder.push_record([
                        format!("{weapon:?}"),
                        kills.to_string(),
                        teamkills.to_string(),
                    ]);
                }

                let mut table = table_builder.build();
                table.with(Style::markdown());

                writeln!(f, "{table}\n")?;

                // Kill streaks section
                writeln!(f, "#### Kill Streaks\n")?;

                let mut table_builder = Builder::default();
                table_builder.push_record([
                    "Wave",
                    "Total Kills",
                    "Start Time",
                    "Duration",
                    "Weapons Used",
                ]);

                for (wave, kill_streak) in player.kill_streaks.iter().enumerate() {
                    if let (Some((start_time, _)), Some((end_time, _))) =
                        (kill_streak.kills.first(), kill_streak.kills.last())
                    {
                        let start_time_offset =
                            Duration::new(start_time.viewdemo_offset.as_secs(), 0);
                        let streak_duration = Duration::new((end_time - start_time).as_secs(), 0);

                        let weapons_used = kill_streak
                            .kills
                            .iter()
                            .map(|(_, weapon)| format!("{weapon:?}"))
                            .collect::<Vec<_>>()
                            .join(", ");

                        table_builder.push_record([
                            (wave + 1).to_string(),
                            kill_streak.kills.len().to_string(),
                            format_duration(start_time_offset).to_string(),
                            format_duration(streak_duration).to_string(),
                            weapons_used,
                        ]);
                    }
                }

                let mut table = table_builder.build();
                table.with(Style::markdown());

                writeln!(f, "{table}\n")?;
            }
        }

        Ok(())
    }
}
