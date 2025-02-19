use crate::analysis::AnalyzerState;
use crate::dod::Team;
use dem::types::Demo;
use humantime::{format_duration, format_rfc3339_seconds};
use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use tabled::{builder::Builder, settings::Style};

pub struct FileInfo<'a> {
    pub created_at: &'a SystemTime,
    pub path: &'a PathBuf,
}

pub struct Report<'a> {
    pub analysis: &'a AnalyzerState,
    pub file_info: FileInfo<'a>,
    pub demo: &'a Demo,
}

impl Display for Report<'_> {
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
        let file_name = &self
            .file_info
            .path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap();
        let map_name = String::from_utf8(self.demo.header.map_name.to_vec()).unwrap();
        let map_name = map_name.trim_end_matches('\x00');
        writeln!(f, "# Summary: {} on {}\n", file_name, map_name)?;

        let file_path = &self.file_info.path.to_str().unwrap();
        writeln!(f, "- File name: `{}`", file_path)?;
        let file_created_at = format_rfc3339_seconds(*self.file_info.created_at);
        writeln!(f, "- File created at: {}", file_created_at)?;
        let report_created_at = format_rfc3339_seconds(SystemTime::now());
        writeln!(f, "- Report created at: {}\n", report_created_at)?;

        // Player scoreboard section
        let mut table_builder = Builder::default();
        table_builder.push_record(["ID", "Name", "Team", "Class", "Score", "Kills", "Deaths"]);

        for player in &ordered_players {
            table_builder.push_record([
                player.player_global_id.0.to_string(),
                player.name.to_string(),
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

        writeln!(f, "{}\n", table)?;

        // Individual player summaries
        writeln!(f, "## Player Summaries\n")?;

        for player in &ordered_players {
            writeln!(f, "### {}\n", player.name)?;

            // Kills per weapon section
            writeln!(f, "#### Weapon Breakdown\n")?;

            let mut table_builder = Builder::default();
            table_builder.push_record(["Weapon", "Kills"]);

            for (weapon, kills) in player.weapon_breakdown.iter() {
                table_builder.push_record([format!("{:?}", weapon), kills.to_string()]);
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

        Ok(())
    }
}
