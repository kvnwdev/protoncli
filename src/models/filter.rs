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

    fn translate_field(&self, field: &str, operator: &Operator, value: &str) -> Result<String> {
        match (field.to_lowercase().as_str(), operator) {
            ("from", Operator::Equals) => Ok(format!("FROM \"{}\"", value)),
            ("to", Operator::Equals) => Ok(format!("TO \"{}\"", value)),
            ("subject", Operator::Equals) => Ok(format!("SUBJECT \"{}\"", value)),
            ("body", Operator::Equals) => Ok(format!("BODY \"{}\"", value)),
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
            _ => {
                let supported_fields = vec![
                    "from", "to", "subject", "body", "unread", "is",
                    "date", "since", "before", "size", "has"
                ];
                Err(anyhow!(
                    "Unsupported query: '{}:{}'\n\nSupported fields: {}\n\nRun 'protoncli query-help' for more information.",
                    field, value, supported_fields.join(", ")
                ))
            }
        }
    }

    fn parse_date(&self, value: &str) -> Result<String> {
        let date = NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .context("Invalid date format. Use YYYY-MM-DD")?;
        Ok(date.format("%d-%b-%Y").to_string())
    }
}

impl Default for MessageFilter {
    fn default() -> Self {
        Self::new()
    }
}
