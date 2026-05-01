//! Server configuration and CLI argument parsing.

use clap::Parser;
use std::fmt;

/// Transport protocol type for the MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TransportType {
    #[default]
    Stdio,
    Sse,
}

impl fmt::Display for TransportType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransportType::Stdio => write!(f, "stdio"),
            TransportType::Sse => write!(f, "sse"),
        }
    }
}

impl std::str::FromStr for TransportType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "stdio" => Ok(TransportType::Stdio),
            "sse" => Ok(TransportType::Sse),
            _ => Err(format!("unknown transport type: '{}'. Use 'stdio' or 'sse'", s)),
        }
    }
}

/// CLI arguments for the MCP macOS Calendar server.
#[derive(Debug, Parser)]
#[command(name = "mcp-macos-calendar")]
#[command(about = "MCP server for macOS Calendar access via EventKit")]
#[command(version)]
pub struct CliArgs {
    /// Transport type: stdio or sse
    #[arg(long, default_value = "stdio")]
    pub transport: TransportType,

    /// Port for SSE mode
    #[arg(long, default_value_t = 8080)]
    pub port: u16,

    /// Host for SSE mode
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Log level
    #[arg(long, default_value = "info")]
    pub log_level: String,
}

/// Server configuration derived from CLI arguments.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub transport: TransportType,
    pub port: u16,
    pub host: String,
    pub log_level: String,
}

impl From<CliArgs> for ServerConfig {
    fn from(args: CliArgs) -> Self {
        ServerConfig {
            transport: args.transport,
            port: args.port,
            host: args.host,
            log_level: args.log_level,
        }
    }
}

impl ServerConfig {
    /// Returns the SSE endpoint URL.
    pub fn sse_endpoint(&self) -> String {
        format!("http://{}:{}/sse", self.host, self.port)
    }

    /// Returns the Streamable HTTP (MCP) endpoint URL.
    pub fn mcp_endpoint(&self) -> String {
        format!("http://{}:{}/mcp", self.host, self.port)
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_S01AC6_default_cli_args() {
        let args = CliArgs::try_parse_from(["mcp-macos-calendar"]).unwrap();
        assert_eq!(args.transport, TransportType::Stdio);
        assert_eq!(args.port, 8080);
        assert_eq!(args.host, "127.0.0.1");
        assert_eq!(args.log_level, "info");
    }

    #[test]
    fn test_S01AC6_custom_sse_transport() {
        let args = CliArgs::try_parse_from([
            "mcp-macos-calendar",
            "--transport",
            "sse",
            "--port",
            "3000",
            "--host",
            "0.0.0.0",
            "--log-level",
            "debug",
        ])
        .unwrap();
        assert_eq!(args.transport, TransportType::Sse);
        assert_eq!(args.port, 3000);
        assert_eq!(args.host, "0.0.0.0");
        assert_eq!(args.log_level, "debug");
    }

    #[test]
    fn test_S01AC6_invalid_transport_fails() {
        let result = CliArgs::try_parse_from(["mcp-macos-calendar", "--transport", "invalid"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_S01AC6_transport_type_display() {
        assert_eq!(format!("{}", TransportType::Stdio), "stdio");
        assert_eq!(format!("{}", TransportType::Sse), "sse");
    }

    #[test]
    fn test_S01AC6_transport_type_from_str() {
        assert_eq!("stdio".parse::<TransportType>().unwrap(), TransportType::Stdio);
        assert_eq!("sse".parse::<TransportType>().unwrap(), TransportType::Sse);
        assert_eq!("STDIO".parse::<TransportType>().unwrap(), TransportType::Stdio);
        assert!("invalid".parse::<TransportType>().is_err());
    }

    #[test]
    fn test_S01AC6_server_config_endpoints() {
        let config = ServerConfig {
            transport: TransportType::Sse,
            port: 3000,
            host: "0.0.0.0".to_string(),
            log_level: "debug".to_string(),
        };
        assert_eq!(config.sse_endpoint(), "http://0.0.0.0:3000/sse");
        assert_eq!(config.mcp_endpoint(), "http://0.0.0.0:3000/mcp");
    }

    #[test]
    fn test_S01AC6_server_config_from_cli_args() {
        let args = CliArgs::try_parse_from([
            "mcp-macos-calendar",
            "--transport",
            "sse",
            "--port",
            "3000",
        ])
        .unwrap();
        let config = ServerConfig::from(args);
        assert_eq!(config.transport, TransportType::Sse);
        assert_eq!(config.port, 3000);
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.log_level, "info");
    }
}
