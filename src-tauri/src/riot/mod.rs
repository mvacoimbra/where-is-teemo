pub mod config;
pub mod process;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Game {
    LeagueOfLegends,
    Valorant,
}

impl Game {
    pub fn display_name(&self) -> &str {
        match self {
            Game::LeagueOfLegends => "League of Legends",
            Game::Valorant => "VALORANT",
        }
    }
}
