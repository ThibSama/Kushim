use time::OffsetDateTime;
use uuid::Uuid;

/// Durable portfolio refresh request status.
///
/// Mirrors the `chk_portfolio_refresh_requests_status` DDL constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshRequestStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

impl RefreshRequestStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Processing => "processing",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

impl TryFrom<&str> for RefreshRequestStatus {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "pending" => Ok(Self::Pending),
            "processing" => Ok(Self::Processing),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            _ => Err(()),
        }
    }
}

/// A durable portfolio refresh request as stored in
/// `portfolio_refresh_requests`. The raw `last_error` column is intentionally
/// not part of this domain type; it stays in PostgreSQL/logs for diagnostics
/// and is never exposed through the public API.
#[derive(Debug, Clone)]
pub struct PortfolioRefreshRequest {
    pub id_portfolio_refresh_request: Uuid,
    pub id_portfolio: Uuid,
    pub status: RefreshRequestStatus,
    pub attempts: i32,
    pub requested_at: OffsetDateTime,
    pub processing_started_at: Option<OffsetDateTime>,
    pub completed_at: Option<OffsetDateTime>,
    pub has_error: bool,
    pub updated_at: OffsetDateTime,
}
