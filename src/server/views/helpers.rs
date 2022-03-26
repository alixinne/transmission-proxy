use handlebars::{
    Context, Handlebars, Helper, HelperResult, JsonRender, Output, RenderContext, RenderError,
};

pub fn urlencode_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let param = h
        .param(0)
        .ok_or_else(|| RenderError::new("missing parameter for urlencode"))?;

    out.write(urlencoding::encode(param.value().render().as_ref()).as_ref())?;

    Ok(())
}
