use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Path, Query, State, WebSocketUpgrade};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde::Deserialize;
use tokio::sync::broadcast;

use crate::index::ImageIndex;
use crate::page::gallery_html;
use crate::types::content_type_for_extension;

#[derive(Clone)]
pub struct AppState {
    pub index: Arc<ImageIndex>,
    pub root: PathBuf,
    pub tx: broadcast::Sender<String>,
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(handler_gallery))
        .route("/api/images", get(handler_api_images))
        .route("/image/{*path}", get(handler_image))
        .route("/ws", get(handler_ws))
        .with_state(state)
}

async fn handler_gallery() -> Html<&'static str> {
    Html(gallery_html())
}

#[derive(Deserialize)]
struct ImageQuery {
    offset: Option<usize>,
    limit: Option<usize>,
}

async fn handler_api_images(
    State(state): State<AppState>,
    Query(query): Query<ImageQuery>,
) -> impl IntoResponse {
    let entries = state.index.get_all().await;
    let total = entries.len();
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(15);
    let images: Vec<_> = entries.iter().skip(offset).take(limit).map(|e| e.to_info()).collect();
    let has_more = offset + limit < total;
    let body = serde_json::json!({
        "root": state.root.to_string_lossy(),
        "total": total,
        "count": images.len(),
        "has_more": has_more,
        "images": images,
    });
    axum::Json(body)
}

async fn handler_image(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> Response {
    let requested = state.root.join(&path);

    // Path traversal protection: canonicalize and verify it's under root
    let canonical = match tokio::fs::canonicalize(&requested).await {
        Ok(p) => p,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };

    let canonical_root = match tokio::fs::canonicalize(&state.root).await {
        Ok(p) => p,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    if !canonical.starts_with(&canonical_root) {
        return StatusCode::FORBIDDEN.into_response();
    }

    // Read file
    let data = match tokio::fs::read(&canonical).await {
        Ok(d) => d,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };

    // Determine content type
    let content_type = canonical
        .extension()
        .and_then(|e| e.to_str())
        .map(content_type_for_extension)
        .unwrap_or("application/octet-stream");

    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        data,
    )
        .into_response()
}

async fn handler_ws(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| handle_ws_connection(socket, state.tx))
}

async fn handle_ws_connection(mut socket: WebSocket, tx: broadcast::Sender<String>) {
    let mut rx = tx.subscribe();
    loop {
        match rx.recv().await {
            Ok(msg) => {
                if socket.send(Message::Text(msg.into())).await.is_err() {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::debug!("WebSocket client lagged by {} messages", n);
                continue;
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}
