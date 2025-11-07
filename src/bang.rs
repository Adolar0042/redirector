use std::fmt::Display;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Bang {
    /// The category of the bang command (e.g., "Tech", "Entertainment").
    #[serde(alias = "category", rename = "c")]
    pub category: Option<Category>,
    /// The domain associated with the bang command (e.g., "www.example.com").
    #[serde(alias = "domain", rename = "d")]
    pub domain: Option<String>,
    /// The relevance score of the bang command.
    #[serde(alias = "relevance", rename = "r")]
    pub relevance: Option<u64>,
    /// The short name or abbreviation of the bang command.
    #[serde(alias = "short_name", rename = "s")]
    pub short_name: Option<String>,
    /// The subcategory of the bang command, if applicable.
    #[serde(alias = "subcategory", rename = "sc")]
    pub subcategory: Option<String>,
    /// The trigger text for the bang command (e.g., "g" for Google).
    #[serde(alias = "trigger", rename = "t")]
    pub trigger: String,
    /// The URL template where the search term is inserted.
    #[serde(alias = "url_template", rename = "u")]
    pub url_template: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub enum Category {
    Entertainment,
    Multimedia,
    News,
    #[serde(alias = "Online Services", rename = "Online Services")]
    OnlineServices,
    Research,
    Shopping,
    Tech,
    Translation,
}

impl Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Entertainment => write!(f, "Entertainment"),
            Self::Multimedia => write!(f, "Multimedia"),
            Self::News => write!(f, "News"),
            Self::OnlineServices => write!(f, "Online Services"),
            Self::Research => write!(f, "Research"),
            Self::Shopping => write!(f, "Shopping"),
            Self::Tech => write!(f, "Tech"),
            Self::Translation => write!(f, "Translation"),
        }
    }
}
