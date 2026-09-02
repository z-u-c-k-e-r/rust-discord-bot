use axum::{
    http::{
        HeaderValue,
        header::{CACHE_CONTROL, CONTENT_TYPE},
    },
    response::{Html, IntoResponse, Response},
};

pub async fn index() -> Html<&'static str> {
    Html(include_str!("../../web/static/index.html"))
}

pub async fn styles() -> Response {
    asset(
        include_str!("../../web/static/styles.css"),
        "text/css; charset=utf-8",
    )
}

pub async fn javascript() -> Response {
    asset(
        include_str!("../../web/static/app.js"),
        "text/javascript; charset=utf-8",
    )
}

fn asset(body: &'static str, content_type: &'static str) -> Response {
    let mut response = body.into_response();
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    response.headers_mut().insert(
        CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=300"),
    );
    response
}
