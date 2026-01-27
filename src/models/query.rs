use anyhow::{anyhow, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum QueryExpr {
    Field {
        name: String,
        operator: Operator,
        value: String,
    },
    And(Box<QueryExpr>, Box<QueryExpr>),
    Or(Box<QueryExpr>, Box<QueryExpr>),
    Not(Box<QueryExpr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Operator {
    Equals,      // field:value
    GreaterThan, // field:>value
    LessThan,    // field:<value
}

pub struct QueryParser;

impl QueryParser {
    pub fn parse(query: &str) -> Result<QueryExpr> {
        let tokens = Self::tokenize(query)?;
        Self::parse_tokens(&tokens)
    }

    fn tokenize(query: &str) -> Result<Vec<String>> {
        // Split on whitespace, preserve quoted strings
        let mut tokens = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;

        for ch in query.chars() {
            match ch {
                '"' => in_quotes = !in_quotes,
                ' ' if !in_quotes => {
                    if !current.is_empty() {
                        tokens.push(current.clone());
                        current.clear();
                    }
                }
                _ => current.push(ch),
            }
        }

        if !current.is_empty() {
            tokens.push(current);
        }

        Ok(tokens)
    }

    fn parse_tokens(tokens: &[String]) -> Result<QueryExpr> {
        // Recursive descent parser
        // Handle OR (lowest precedence)
        // Handle AND (medium precedence)
        // Handle NOT (highest precedence)
        // Handle field:value expressions

        // Simplified implementation for phase 1
        if tokens.is_empty() {
            return Err(anyhow!("Empty query"));
        }

        // Find OR operators first
        for (i, token) in tokens.iter().enumerate() {
            if token.to_uppercase() == "OR" {
                let left = Self::parse_tokens(&tokens[..i])?;
                let right = Self::parse_tokens(&tokens[i + 1..])?;
                return Ok(QueryExpr::Or(Box::new(left), Box::new(right)));
            }
        }

        // Find AND operators
        for (i, token) in tokens.iter().enumerate() {
            if token.to_uppercase() == "AND" {
                let left = Self::parse_tokens(&tokens[..i])?;
                let right = Self::parse_tokens(&tokens[i + 1..])?;
                return Ok(QueryExpr::And(Box::new(left), Box::new(right)));
            }
        }

        // Handle NOT
        if tokens[0].to_uppercase() == "NOT" || tokens[0] == "!" {
            let inner = Self::parse_tokens(&tokens[1..])?;
            return Ok(QueryExpr::Not(Box::new(inner)));
        }

        // Parse field:value
        if tokens.len() == 1 {
            return Self::parse_field_expr(&tokens[0]);
        }

        // Implicit AND for multiple tokens
        let left = Self::parse_field_expr(&tokens[0])?;
        let right = Self::parse_tokens(&tokens[1..])?;
        Ok(QueryExpr::And(Box::new(left), Box::new(right)))
    }

    fn parse_field_expr(token: &str) -> Result<QueryExpr> {
        if let Some((field, value)) = token.split_once(':') {
            if value.is_empty() {
                return Err(anyhow!(
                    "Empty value for field '{}'. Expected format: field:value\n\nExample: from:user@example.com",
                    field
                ));
            }

            let (operator, clean_value) = if value.starts_with('>') {
                (Operator::GreaterThan, &value[1..])
            } else if value.starts_with('<') {
                (Operator::LessThan, &value[1..])
            } else {
                (Operator::Equals, value)
            };

            if clean_value.is_empty() {
                return Err(anyhow!(
                    "Empty value after operator in '{}'. Expected format: field:>value or field:<value",
                    token
                ));
            }

            Ok(QueryExpr::Field {
                name: field.to_string(),
                operator,
                value: clean_value.to_string(),
            })
        } else {
            Err(anyhow!(
                "Invalid query syntax: '{}'\n\nExpected format: field:value (e.g., from:user@example.com)\nOr use boolean operators: AND, OR, NOT",
                token
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_field() {
        let result = QueryParser::parse("from:test@example.com").unwrap();
        assert!(matches!(result, QueryExpr::Field { .. }));
    }

    #[test]
    fn test_and_operator() {
        let result = QueryParser::parse("from:test@example.com AND subject:hello").unwrap();
        assert!(matches!(result, QueryExpr::And(_, _)));
    }

    #[test]
    fn test_or_operator() {
        let result = QueryParser::parse("from:test@example.com OR from:other@example.com").unwrap();
        assert!(matches!(result, QueryExpr::Or(_, _)));
    }

    #[test]
    fn test_not_operator() {
        let result = QueryParser::parse("NOT from:spam@example.com").unwrap();
        assert!(matches!(result, QueryExpr::Not(_)));
    }

    #[test]
    fn test_comparison_operators() {
        let result = QueryParser::parse("date:>2024-01-01").unwrap();
        if let QueryExpr::Field { operator, .. } = result {
            assert_eq!(operator, Operator::GreaterThan);
        } else {
            panic!("Expected Field expression");
        }
    }

    #[test]
    fn test_less_than_operator() {
        let result = QueryParser::parse("date:<2024-01-01").unwrap();
        if let QueryExpr::Field { operator, .. } = result {
            assert_eq!(operator, Operator::LessThan);
        } else {
            panic!("Expected Field expression");
        }
    }

    #[test]
    fn test_quoted_string_in_value() {
        let result = QueryParser::parse("subject:\"hello world\"").unwrap();
        if let QueryExpr::Field { value, .. } = result {
            assert_eq!(value, "hello world");
        } else {
            panic!("Expected Field expression");
        }
    }

    #[test]
    fn test_implicit_and_multiple_tokens() {
        // Multiple tokens without explicit AND should be implicit AND
        let result = QueryParser::parse("from:alice@example.com subject:hello").unwrap();
        assert!(matches!(result, QueryExpr::And(_, _)));
    }

    #[test]
    fn test_exclamation_not() {
        let result = QueryParser::parse("! from:spam@example.com").unwrap();
        assert!(matches!(result, QueryExpr::Not(_)));
    }

    #[test]
    fn test_empty_query_error() {
        let result = QueryParser::parse("");
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_value_error() {
        let result = QueryParser::parse("from:");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Empty value"));
    }

    #[test]
    fn test_empty_value_after_operator_error() {
        let result = QueryParser::parse("date:>");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Empty value after operator"));
    }

    #[test]
    fn test_invalid_syntax_no_colon() {
        let result = QueryParser::parse("justtext");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Invalid query syntax"));
    }

    #[test]
    fn test_complex_or_and_expression() {
        // OR has lower precedence than AND, so this should parse correctly
        let result = QueryParser::parse("from:a@b.com AND subject:test OR from:c@d.com").unwrap();
        // Should be: OR((AND(from:a, subject:test)), from:c)
        assert!(matches!(result, QueryExpr::Or(_, _)));
    }

    #[test]
    fn test_case_insensitive_operators() {
        // Operators should be case-insensitive
        let result1 = QueryParser::parse("from:a AND to:b").unwrap();
        let result2 = QueryParser::parse("from:a and to:b").unwrap();
        assert!(matches!(result1, QueryExpr::And(_, _)));
        assert!(matches!(result2, QueryExpr::And(_, _)));
    }
}
