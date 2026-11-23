//! `brachio skills`: the skill guides, built into the binary so they match its version.

use anyhow::{bail, Result};
use clap::Subcommand;

/// A skill name and its guide, without the front matter the shipped stubs carry.
pub const GUIDES: [(&str, &str); 2] = [
    ("brachio-send", include_str!("../guides/brachio-send.md")),
    ("brachio-lead", include_str!("../guides/brachio-lead.md")),
];

#[derive(Subcommand)]
pub enum SkillsCommand {
    /// Print one skill guide.
    Get {
        /// Skill name, for example `brachio-send`.
        name: String,
    },
    /// List the skill names.
    List,
}

pub fn run(command: SkillsCommand) -> Result<()> {
    match command {
        SkillsCommand::Get { name } => print!("{}", guide(&name)?),
        SkillsCommand::List => {
            for (name, _) in GUIDES {
                println!("{name}");
            }
        }
    }
    Ok(())
}

fn guide(name: &str) -> Result<&'static str> {
    match GUIDES.iter().find(|(known, _)| *known == name) {
        Some((_, text)) => Ok(text),
        None => {
            let known: Vec<_> = GUIDES.iter().map(|(known, _)| *known).collect();
            bail!("unknown skill {name:?}; available: {}", known.join(", "))
        }
    }
}
