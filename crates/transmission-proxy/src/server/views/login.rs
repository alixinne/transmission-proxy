use serde::Serialize;

use crate::config::Config;

use super::ViewData;

#[derive(Debug, Serialize)]
pub struct Data<'c> {
    pub config: &'c Config,
    pub redirect_to: Option<String>,
}

impl ViewData for Data<'_> {
    const NAME: &'static str = "login";

    const SOURCE: &'static str = include_str!("login.html.hbs");
}
