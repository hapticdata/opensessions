use clap::Parser;

use crate::runtime_config::DEFAULT_SERVER_PORT;

#[derive(Debug, Clone, Parser)]
#[command(name = "opensessions-sidebar")]
pub struct Args {
    #[arg(long, default_value = "127.0.0.1")]
    pub server_host: String,
    #[arg(long, default_value_t = DEFAULT_SERVER_PORT)]
    pub server_port: u16,
}

impl Args {
    pub fn try_parse_from<I, T>(itr: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        <Self as Parser>::try_parse_from(itr)
    }
}
