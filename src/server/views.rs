use handlebars::{Handlebars, RenderError};
use hyper::{header::CONTENT_TYPE, Body, Response};

mod helpers;

// View module declarations
pub mod login;

/// Trait for the data required for a view
pub trait ViewData: serde::Serialize {
    /// Name of the view for registration
    const NAME: &'static str;

    /// Source code for the template
    const SOURCE: &'static str;
}

pub struct Views {
    handlebars: Handlebars<'static>,
}

impl Views {
    pub fn new() -> Self {
        let mut handlebars = Handlebars::new();

        // Register helpers
        handlebars.register_helper("urlencode", Box::new(helpers::urlencode_helper));

        // Register templates
        handlebars
            .register_template_string(login::Data::NAME, login::Data::SOURCE)
            .expect("failed to load template");

        Self { handlebars }
    }

    pub fn render<T>(&self, data: &T) -> Result<Response<Body>, RenderError>
    where
        T: ViewData,
    {
        Ok(Response::builder()
            .status(200)
            .header(CONTENT_TYPE, "text/html")
            .body(Body::from(self.handlebars.render(T::NAME, &data)?))
            .unwrap())
    }
}
