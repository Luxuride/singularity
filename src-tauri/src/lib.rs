use tauri::Manager;
use tauri::async_runtime::block_on;
use tauri::http::header::{ACCEPT_RANGES, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE};
use tauri::http::{Request, Response, StatusCode};

mod auth;
mod db;
mod messages;
mod protocol;
mod rooms;
mod storage;
mod verification;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(auth::AuthState::default())
        .manage(messages::VideoStreamState::default())
        .register_asynchronous_uri_scheme_protocol("matrix-media", move |ctx, request, responder| {
            let response = handle_media_stream_request(ctx.app_handle(), request);
            responder.respond(response);
        })
        .setup(|app| {
            let app_db = db::AppDb::initialize(app.handle())?;
            app.manage(app_db);

            let trigger_state = rooms::start_room_update_worker(app.handle().clone());
            app.manage(trigger_state);
            Ok(())
        })
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            auth::commands::matrix_start_oauth,
            auth::commands::matrix_complete_oauth,
            auth::commands::matrix_session_status,
            auth::commands::matrix_recovery_status,
            auth::commands::matrix_recover_with_key,
            auth::commands::matrix_logout,
            auth::commands::matrix_clear_cache_except_auth,
            rooms::commands::matrix_get_chats,
            rooms::commands::matrix_trigger_room_update,
            messages::commands::matrix_get_chat_messages,
            messages::commands::matrix_stream_chat_messages,
            messages::commands::matrix_send_chat_message,
            messages::commands::matrix_prepare_video_playback,
            verification::commands::matrix_own_verification_status,
            verification::commands::matrix_get_user_devices,
            verification::commands::matrix_request_device_verification,
            verification::commands::matrix_get_verification_flow,
            verification::commands::matrix_accept_verification_request,
            verification::commands::matrix_start_sas_verification,
            verification::commands::matrix_accept_sas_verification,
            verification::commands::matrix_confirm_sas_verification,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn handle_media_stream_request(
    app: &tauri::AppHandle,
    request: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    let stream_state = app.state::<messages::VideoStreamState>();
    let auth_state = app.state::<auth::AuthState>();

    let path = request.uri().path();
    let token = path.trim_start_matches('/').trim_start_matches("stream/");
    if token.is_empty() {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Vec::new())
            .unwrap_or_else(|_| Response::new(Vec::new()));
    }

    let range_header = request
        .headers()
        .get(RANGE)
        .and_then(|value| value.to_str().ok());

    let (start, requested_end) = match parse_single_range_header(range_header) {
        Ok(range) => range,
        Err(()) => {
            return Response::builder()
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .body(Vec::new())
                .unwrap_or_else(|_| Response::new(Vec::new()));
        }
    };

    let client = match block_on(auth_state.restore_client_and_get(app)) {
        Ok(client) => client,
        Err(_) => {
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Vec::new())
                .unwrap_or_else(|_| Response::new(Vec::new()));
        }
    };

    let result = match block_on(stream_state.read_stream_segment(&client, token, start, requested_end)) {
        Ok(result) => result,
        Err(_) => {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Vec::new())
                .unwrap_or_else(|_| Response::new(Vec::new()));
        }
    };

    let (bytes, mime_type, total_len, actual_start, actual_end) = match result {
        messages::streaming::VideoStreamReadOutcome::NotFound => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Vec::new())
                .unwrap_or_else(|_| Response::new(Vec::new()));
        }
        messages::streaming::VideoStreamReadOutcome::NotSatisfiable { total_len } => {
            return Response::builder()
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header(CONTENT_RANGE, format!("bytes */{total_len}"))
                .body(Vec::new())
                .unwrap_or_else(|_| Response::new(Vec::new()));
        }
        messages::streaming::VideoStreamReadOutcome::Segment(segment) => (
            segment.bytes,
            segment.mime_type,
            segment.total_len,
            segment.start,
            segment.end,
        ),
    };

    let mut response = Response::builder()
        .header(CONTENT_TYPE, mime_type)
        .header(ACCEPT_RANGES, "bytes")
        .header(CONTENT_LENGTH, bytes.len().to_string());

    if range_header.is_some() {
        response = response
            .status(StatusCode::PARTIAL_CONTENT)
            .header(CONTENT_RANGE, format!("bytes {actual_start}-{actual_end}/{total_len}"));
    }

    response
        .body(bytes)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

fn parse_single_range_header(value: Option<&str>) -> Result<(usize, Option<usize>), ()> {
    let Some(value) = value else {
        return Ok((0, None));
    };

    let trimmed = value.trim();
    if !trimmed.starts_with("bytes=") {
        return Err(());
    }

    let ranges = &trimmed[6..];
    if ranges.contains(',') {
        return Err(());
    }

    let mut parts = ranges.splitn(2, '-');
    let start_raw = parts.next().ok_or(())?.trim();
    let end_raw = parts.next().ok_or(())?.trim();

    if start_raw.is_empty() {
        return Err(());
    }

    let start = start_raw.parse::<usize>().map_err(|_| ())?;
    if end_raw.is_empty() {
        return Ok((start, None));
    }

    let end = end_raw.parse::<usize>().map_err(|_| ())?;
    Ok((start, Some(end)))
}
