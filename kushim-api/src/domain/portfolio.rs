use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PortfolioVisibility {
    Private,
    Public,
    Unlisted,
}

impl PortfolioVisibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Public => "public",
            Self::Unlisted => "unlisted",
        }
    }
}

impl TryFrom<&str> for PortfolioVisibility {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "private" => Ok(Self::Private),
            "public" => Ok(Self::Public),
            "unlisted" => Ok(Self::Unlisted),
            _ => Err("unknown portfolio visibility"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Portfolio {
    pub id_portfolio: Uuid,
    pub id_user: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub base_currency: String,
    pub visibility: PortfolioVisibility,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct NewPortfolio {
    pub id_user: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub base_currency: String,
    pub visibility: PortfolioVisibility,
}
