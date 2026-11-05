//! Service settings, read from environment variables.

use std::env;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

#[derive(Debug)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub request_timeout: Duration,
}

impl Config {
    /// Reads `BIND_ADDR` and `REQUEST_TIMEOUT_SECS`, using defaults for unset variables.
    /// A variable with an invalid value is an error, so a typo stops the service at
    /// startup instead of silently running it with the default.
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            bind_addr: read_var("BIND_ADDR", SocketAddr::from(([127, 0, 0, 1], 3000)))?,
            request_timeout: Duration::from_secs(read_var("REQUEST_TIMEOUT_SECS", 10)?),
        })
    }
}

fn read_var<T: FromStr>(name: &str, default: T) -> Result<T, String> {
    match env::var(name) {
        Ok(value) => value
            .parse()
            .map_err(|_| format!("environment variable {name} has invalid value '{value}'")),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(env::VarError::NotUnicode(_)) => {
            Err(format!("environment variable {name} is not valid Unicode"))
        }
    }
}
