use crate::analysis::{AnalyzerState, Round};
use crate::dod::Team;
use humantime::{format_duration, format_rfc3339_seconds};
use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use std::time::{Duration, SystemTime};
use tabled::{builder::Builder, settings::Style};

pub struct DemoInfo {
    pub demo_protocol: i32,
    pub network_protocol: i32,
    pub map_name: String,
}

pub struct FileInfo {
    pub created_at: SystemTime,
    pub name: String,
    pub path: String,
}

pub struct Report {
    pub analysis: AnalyzerState,
    pub demo_info: DemoInfo,
    pub file_info: FileInfo,
}

impl Display for Report {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // Players sorted by team then kills
        let mut ordered_players = Vec::from_iter(&self.analysis.players);

        ordered_players.sort_by(|left, right| match (&left.team, &right.team) {
            (Some(left_team), Some(right_team)) if left_team == right_team => {
                left.stats.0.cmp(&right.stats.0).reverse()
            }

            (Some(Team::Allies), _) => Ordering::Less,
            (Some(Team::Axis), Some(Team::Spectators)) => Ordering::Less,
            (Some(Team::Spectators) | None, _) => Ordering::Greater,

            _ => Ordering::Equal,
        });

        // Header section
        {
            let file_name = &self.file_info.name;
            let map_name = &self.demo_info.map_name;
            writeln!(f, "# Summary: {} on {}\n", file_name, map_name)?;

            let file_path = &self.file_info.path;
            writeln!(f, "- File path: `{}`", file_path)?;
            let file_created_at = format_rfc3339_seconds(self.file_info.created_at);
            writeln!(f, "- File created at: {}", file_created_at)?;
            let demo_protocol = &self.demo_info.demo_protocol;
            writeln!(f, "- Demo protocol: {}", demo_protocol)?;
            let network_protocol = &self.demo_info.network_protocol;
            writeln!(f, "- Network protocol: {}", network_protocol)?;
            let app_version = env!("CARGO_PKG_VERSION");
            writeln!(f, "- Analyzer version: {}", app_version)?;
            let report_created_at = format_rfc3339_seconds(SystemTime::now());
            writeln!(f, "- Report created at: {}", report_created_at)?;
        }

        writeln!(f)?;

        // Player scoreboard section
        {
            let mut table_builder = Builder::default();
            table_builder.push_record(["ID", "Name", "Team", "Class", "Score", "Kills", "Deaths"]);

            for player in &ordered_players {
                table_builder.push_record([
                    player.player_global_id.0.to_string(),
                    md_escape(&player.name),
                    match &player.team {
                        None => "Unknown",
                        Some(Team::Allies) => "Allies",
                        Some(Team::Axis) => "Axis",
                        Some(Team::Spectators) => "Spectators",
                    }
                    .to_string(),
                    match &player.class {
                        None => "Unknown".to_string(),
                        Some(x) => format!("{:?}", x),
                    },
                    player.stats.0.to_string(),
                    player.stats.1.to_string(),
                    player.stats.2.to_string(),
                ]);
            }

            let match_result_fragment = match (
                self.analysis.team_scores.get(&Team::Allies),
                self.analysis.team_scores.get(&Team::Axis),
            ) {
                (Some(allies_score), Some(axis_score)) => {
                    format!(
                        ": Allies ({}) {} Axis ({})",
                        allies_score,
                        if allies_score > axis_score { ">" } else { "<" },
                        axis_score
                    )
                }
                _ => String::new(),
            };

            writeln!(f, "## Scoreboard{}\n", match_result_fragment)?;

            let mut table = table_builder.build();
            table.with(Style::markdown());

            writeln!(f, "{}", table)?;
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

            let mut rounds = self.analysis.rounds.iter().enumerate();

            while let Some((
                i,
                Round::Completed {
                    start_time,
                    end_time,
                    winner_stats,
                },
            )) = rounds.next()
            {
                let duration = Duration::new((end_time.offset - start_time.offset).as_secs(), 0);
                let start_time = Duration::new(start_time.offset.as_secs(), 0);

                table_builder.push_record([
                    (i + 1).to_string(),
                    format_duration(start_time).to_string(),
                    format_duration(duration).to_string(),
                    if let Some((winner, _)) = winner_stats {
                        format!("{:?}", winner)
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

            writeln!(f, "{}", table)?;
        }

        writeln!(f)?;

        // Individual player summaries
        {
            writeln!(f, "## Player Summaries\n")?;

            for player in &ordered_players {
                writeln!(f, "### {}\n", md_escape(&player.name))?;

                // Kills per weapon section
                writeln!(f, "#### Weapon Breakdown\n")?;

                let mut table_builder = Builder::default();
                table_builder.push_record(["Weapon", "Kills", "Team Kills"]);

                for (weapon, (kills, teamkills)) in player.weapon_breakdown.iter() {
                    table_builder.push_record([
                        format!("{:?}", weapon),
                        kills.to_string(),
                        teamkills.to_string(),
                    ]);
                }

                let mut table = table_builder.build();
                table.with(Style::markdown());

                writeln!(f, "{}\n", table)?;

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
                        let start_time_offset = Duration::new(start_time.offset.as_secs(), 0);
                        let streak_duration =
                            Duration::new((end_time.offset - start_time.offset).as_secs(), 0);

                        let weapons_used = kill_streak
                            .kills
                            .iter()
                            .map(|(_, weapon)| format!("{:?}", weapon))
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

                writeln!(f, "{}\n", table)?;
            }
        }

        Ok(())
    }
}

fn md_escape(str: &str) -> String {
    str.replace("|", r"\|")
        .replace("_", r"\_")
        .replace("*", r"\*")
        .replace("[", r"\[")
        .replace("]", r"\]")
}
