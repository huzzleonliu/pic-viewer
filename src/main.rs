#![recursion_limit = "512"]

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::routing::get;
    use axum::Router;
    use leptos::logging::log;
    use leptos::prelude::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use pic_viewer::function::{pic_root, serve_export, serve_media, serve_preview, serve_thumb};
    use pic_viewer::page::app::{shell, App};

    let conf = get_configuration(None).unwrap();
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(App);

    log!("PIC_ROOT = {}", pic_root().display());
    match std::env::var("IMGPROXY_URL") {
        Ok(url) if !url.trim().is_empty() => {
            log!("IMGPROXY_URL = {} (thumbs/preview via imgproxy)", url.trim());
        }
        _ => log!("IMGPROXY_URL unset (thumbs/preview serve originals)"),
    }

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/media/{*path}", get(serve_media))
        .route("/thumb/{*path}", get(serve_thumb))
        .route("/preview/{*path}", get(serve_preview))
        .route("/export/{id}", get(serve_export))
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(leptos_options);

    log!("listening on http://{}", &addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

#[cfg(not(feature = "ssr"))]
pub fn main() {}
