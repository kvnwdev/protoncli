use crate::models::query::{Operator, QueryExpr, QueryParser};
use anyhow::{anyhow, Context, Result};
use chrono::{Duration, NaiveDate, Utc};

#[derive(Debug, Clone)]
pub struct MessageFilter {
    pub days: Option<u32>,
    pub unread_only: bool,
    pub agent_unread: bool,
    pub limit: Option<usize>,
    pub query: Option<String>,
    pub preview: bool,
    /// Folder extracted from `in:` query field (overrides --folder flag)
    pub query_folder: Option<String>,
}

impl MessageFilter {
    pub fn new() -> Self {
        Self {
            days: None,
            unread_only: false,
            agent_unread: false,
            limit: None,
            query: None,
            preview: false,
            query_folder: None,
        }
    }

    pub fn with_days(mut self, days: u32) -> Self {
        self.days = Some(days);
        self
    }

    pub fn with_unread_only(mut self, unread_only: bool) -> Self {
        self.unread_only = unread_only;
        self
    }

    pub fn with_agent_unread(mut self, agent_unread: bool) -> Self {
        self.agent_unread = agent_unread;
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn with_query(mut self, query: String) -> Self {
        self.query = Some(query);
        self
    }

    pub fn with_preview(mut self, preview: bool) -> Self {
        self.preview = preview;
        self
    }

    pub fn build_imap_search_query(&self) -> Result<String> {
        let mut parts = Vec::new();

        // Handle query expression if present
        if let Some(query_str) = &self.query {
            // Skip empty queries
            if query_str.trim().is_empty() {
                // Empty query is treated as no query
            } else {
                let expr = QueryParser::parse(query_str)
                    .context(format!("Invalid query syntax: '{}'\n\nRun 'protoncli query-help' to see syntax examples.", query_str))?;
                let imap_query = self.translate_to_imap(&expr)?;
                parts.push(imap_query);
            }
        }

        // Handle legacy flags
        if self.unread_only {
            parts.push("UNSEEN".to_string());
        }

        if let Some(days) = self.days {
            let since_date = Utc::now() - Duration::days(days as i64);
            let date_str = since_date.format("%d-%b-%Y").to_string();
            parts.push(format!("SINCE {}", date_str));
        }

        if parts.is_empty() {
            Ok("ALL".to_string())
        } else {
            Ok(parts.join(" "))
        }
    }

    fn translate_to_imap(&self, expr: &QueryExpr) -> Result<String> {
        match expr {
            QueryExpr::Field { name, operator, value } => {
                self.translate_field(name, operator, value)
            }
            QueryExpr::And(left, right) => {
                let left_imap = self.translate_to_imap(left)?;
                let right_imap = self.translate_to_imap(right)?;
                Ok(format!("{} {}", left_imap, right_imap))
            }
            QueryExpr::Or(left, right) => {
                let left_imap = self.translate_to_imap(left)?;
                let right_imap = self.translate_to_imap(right)?;
                Ok(format!("OR ({}) ({})", left_imap, right_imap))
            }
            QueryExpr::Not(inner) => {
                let inner_imap = self.translate_to_imap(inner)?;
                Ok(format!("NOT {}", inner_imap))
            }
        }
    }

    fn escape_imap_string(s: &str) -> String {
        s.replace('\\', "\\\\").replace('"', "\\\"")
    }

    fn translate_field(&self, field: &str, operator: &Operator, value: &str) -> Result<String> {
        let escaped_value = Self::escape_imap_string(value);
        match (field.to_lowercase().as_str(), operator) {
            ("from", Operator::Equals) => Ok(format!("FROM \"{}\"", escaped_value)),
            ("to", Operator::Equals) => Ok(format!("TO \"{}\"", escaped_value)),
            ("subject", Operator::Equals) => Ok(format!("SUBJECT \"{}\"", escaped_value)),
            ("body", Operator::Equals) => Ok(format!("BODY \"{}\"", escaped_value)),
            ("unread", Operator::Equals) if value == "true" => Ok("UNSEEN".to_string()),
            ("is", Operator::Equals) if value == "unread" => Ok("UNSEEN".to_string()),
            ("date", Operator::GreaterThan) => {
                let date = self.parse_date(value)?;
                Ok(format!("SINCE {}", date))
            }
            ("date", Operator::LessThan) => {
                let date = self.parse_date(value)?;
                Ok(format!("BEFORE {}", date))
            }
            ("since", Operator::Equals) => {
                let date = self.parse_date(value)?;
                Ok(format!("SINCE {}", date))
            }
            ("before", Operator::Equals) => {
                let date = self.parse_date(value)?;
                Ok(format!("BEFORE {}", date))
            }
            ("size", Operator::GreaterThan) => {
                Ok(format!("LARGER {}", value))
            }
            ("size", Operator::LessThan) => {
                Ok(format!("SMALLER {}", value))
            }
            ("has", Operator::Equals) if value == "attachment" => {
                // Mark for client-side filtering
                Ok("ALL".to_string()) // Will filter client-side
            }
            // Relative date shortcuts: newer:30d, older:7d
            ("newer", Operator::Equals) => {
                let days = Self::parse_relative_days(value)?;
                let since_date = Utc::now() - Duration::days(days);
                Ok(format!("SINCE {}", since_date.format("%d-%b-%Y")))
            }
            ("older", Operator::Equals) => {
                let days = Self::parse_relative_days(value)?;
                let before_date = Utc::now() - Duration::days(days);
                Ok(format!("BEFORE {}", before_date.format("%d-%b-%Y")))
            }
            // Folder shorthand: in:Sent (handled at CLI level, returns ALL here)
            ("in" | "folder", Operator::Equals) => {
                // The folder is extracted separately; here we just return ALL
                // to not add unnecessary constraints to the IMAP query
                Ok("ALL".to_string())
            }
            _ => {
                let supported_fields = vec![
                    "from", "to", "subject", "body", "unread", "is",
                    "date", "since", "before", "size", "has", "newer", "older", "in", "folder"
                ];
                Err(anyhow!(
                    "Unsupported query: '{}:{}'\n\nSupported fields: {}\n\nRun 'protoncli query-help' for more information.",
                    field, value, supported_fields.join(", ")
                ))
            }
        }
    }

    fn parse_date(&self, value: &str) -> Result<String> {
        // Try relative date first (e.g., 30d, 2w, 1m)
        if let Ok(days) = Self::parse_relative_days(value) {
            let date = Utc::now() - Duration::days(days);
            return Ok(date.format("%d-%b-%Y").to_string());
        }
        // Fall back to absolute date
        let date = NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .context("Invalid date format. Use YYYY-MM-DD or relative like 30d, 2w, 1m")?;
        Ok(date.format("%d-%b-%Y").to_string())
    }

    /// Parse relative date strings like "30d", "2w", "1m" into days
    fn parse_relative_days(value: &str) -> Result<i64> {
        let value = value.trim().to_lowercase();
        if value.is_empty() {
            return Err(anyhow!("Empty relative date"));
        }

        let (num_str, unit) = value.split_at(value.len() - 1);
        let num: i64 = num_str.parse()
            .context(format!("Invalid number in relative date: '{}'", value))?;

        match unit {
            "d" => Ok(num),           // days
            "w" => Ok(num * 7),       // weeks
            "m" => Ok(num * 30),      // months (approximate)
            "y" => Ok(num * 365),     // years (approximate)
            _ => Err(anyhow!(
                "Invalid relative date format: '{}'. Use format like 30d, 2w, 1m, 1y",
                value
            )),
        }
    }

    /// Extract folder from query string if `in:` or `folder:` is present
    pub fn extract_folder_from_query(query: &str) -> Option<String> {
        // Simple extraction - look for in:folder or folder:folder
        for token in query.split_whitespace() {
            if let Some(folder) = token.strip_prefix("in:") {
                if !folder.is_empty() {
                    return Some(folder.to_string());
                }
            }
            if let Some(folder) = token.strip_prefix("folder:") {
                if !folder.is_empty() {
                    return Some(folder.to_string());
                }
            }
        }
        None
    }
}

impl Default for MessageFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_relative_days_valid() {
        assert_eq!(MessageFilter::parse_relative_days("30d").unwrap(), 30);
        assert_eq!(MessageFilter::parse_relative_days("7d").unwrap(), 7);
        assert_eq!(MessageFilter::parse_relative_days("1d").unwrap(), 1);
    }

    #[test]
    fn test_parse_relative_weeks() {
        assert_eq!(MessageFilter::parse_relative_days("2w").unwrap(), 14);
        assert_eq!(MessageFilter::parse_relative_days("1w").unwrap(), 7);
    }

    #[test]
    fn test_parse_relative_months() {
        assert_eq!(MessageFilter::parse_relative_days("1m").unwrap(), 30);
        assert_eq!(MessageFilter::parse_relative_days("3m").unwrap(), 90);
    }

    #[test]
    fn test_parse_relative_years() {
        assert_eq!(MessageFilter::parse_relative_days("1y").unwrap(), 365);
    }

    #[test]
    fn test_parse_relative_invalid() {
        assert!(MessageFilter::parse_relative_days("abc").is_err());
        assert!(MessageFilter::parse_relative_days("d").is_err());
        assert!(MessageFilter::parse_relative_days("").is_err());
    }

    #[test]
    fn test_extract_folder_in_syntax() {
        assert_eq!(
            MessageFilter::extract_folder_from_query("in:Sent"),
            Some("Sent".to_string())
        );
        assert_eq!(
            MessageFilter::extract_folder_from_query("from:test@example.com in:Archive"),
            Some("Archive".to_string())
        );
    }

    #[test]
    fn test_extract_folder_folder_syntax() {
        assert_eq!(
            MessageFilter::extract_folder_from_query("folder:Drafts"),
            Some("Drafts".to_string())
        );
    }

    #[test]
    fn test_extract_folder_none() {
        assert_eq!(
            MessageFilter::extract_folder_from_query("from:test@example.com"),
            None
        );
        assert_eq!(
            MessageFilter::extract_folder_from_query("subject:hello"),
            None
        );
    }

    #[test]
    fn test_newer_query_translation() {
        let filter = MessageFilter::new().with_query("newer:30d".to_string());
        let imap_query = filter.build_imap_search_query().unwrap();
        assert!(imap_query.starts_with("SINCE "));
    }

    #[test]
    fn test_older_query_translation() {
        let filter = MessageFilter::new().with_query("older:7d".to_string());
        let imap_query = filter.build_imap_search_query().unwrap();
        assert!(imap_query.starts_with("BEFORE "));
    }

    #[test]
    fn test_in_folder_query_translation() {
        // in:folder returns ALL since folder selection happens at CLI level
        let filter = MessageFilter::new().with_query("in:Sent".to_string());
        let imap_query = filter.build_imap_search_query().unwrap();
        assert_eq!(imap_query, "ALL");
    }
}
