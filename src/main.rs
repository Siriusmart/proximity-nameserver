use std::{
    collections::HashMap,
    env,
    sync::{Arc, OnceLock},
};

use axum::{Json, Router, extract::State, routing::post};
use rusqlite::{Connection, fallible_streaming_iterator::FallibleStreamingIterator};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpListener, sync::Mutex};

#[derive(Serialize, Deserialize)]
struct WhoIsReq {
    ids: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct ModifyReq {
    id: String,
    ign: String,
    pw: String,
}

#[tokio::main]
async fn main() {
    let port = env::var("PORT")
        .map(|s| s.parse::<u16>().unwrap())
        .unwrap_or(25550);
    let db_path = env::var("DB").unwrap_or("db.sqlite".to_string());
    static BOTPASS: OnceLock<String> = OnceLock::new();
    BOTPASS
        .set(env::var("PASSWORD").expect("missing env PASSWORD"))
        .unwrap();

    let conn = Arc::new(Mutex::new(Connection::open(db_path).unwrap()));
    conn.lock()
        .await
        .execute(
            "CREATE TABLE IF NOT EXISTS Translation (
            discordID       TEXT NOT NULL,
            ign             TEXT NOT NULL,

            PRIMARY KEY (discordID, ign)
        )",
            [], // No parameters needed
        )
        .unwrap();

    let app = Router::new()
        .route("/api/v1/add", post(|State(conn): State<Arc<Mutex<Connection>>>, content: Json<ModifyReq>| async move {
            let conn = conn.lock().await;

            if content.pw.as_str() != BOTPASS.get().unwrap().as_str() {
                return "-1".to_string();
            }

            let ign = content.ign.as_str();
            let discord_id = content.id.as_str();

            if discord_id.chars().any(|c| !c.is_ascii_digit()) || ign.chars().any(|c| !c.is_ascii_alphanumeric() && c != '_') {
                return "0".to_string();
            }

            let mut stmt = conn.prepare(
                format!(r#"INSERT OR IGNORE INTO Translation (discordID, ign) VALUES ("{discord_id}", "{ign}");"#).as_str()).unwrap();

            stmt.execute([]).unwrap().to_string()
        }))
        .route("/api/v1/remove", post(|State(conn): State<Arc<Mutex<Connection>>>, content: Json<ModifyReq>| async move {
            let conn = conn.lock().await;

            if content.pw.as_str() != BOTPASS.get().unwrap().as_str() {
                return "-1".to_string();
            }

            let ign = content.ign.as_str();
            let discord_id = content.id.as_str();

            if discord_id.chars().any(|c| !c.is_ascii_digit()) || ign.chars().any(|c| !c.is_ascii_alphanumeric() && c != '_') {
                return "0".to_string();
            }

            let mut stmt = conn.prepare(
                format!(r#"DELETE FROM Translation WHERE discordID="{discord_id}" AND ign="{ign}""#).as_str()).unwrap();

            stmt.execute([]).unwrap().to_string()
        }))
        .route(
            "/api/v1/whois",
            post(
                |State(conn): State<Arc<Mutex<Connection>>>, content: Json<WhoIsReq>| async move {
                    let conn = conn.lock().await;

                    if content.ids.iter().any(|id| id.chars().any(|c| !c.is_ascii_digit())) || content.ids.is_empty() {
                        return Json(HashMap::new());
                    }

                    let mut stmt = conn.prepare(
                        format!(
                            "SELECT discordID, ign FROM Translation WHERE {}",
                            content
                                .ids
                                .iter()
                                .map(|id| format!(r#"discordID="{id}""#))
                                .collect::<Vec<_>>()
                                .join(" OR ")
                        )
                        .as_str(),
                    ).unwrap();

                    let mut res = HashMap::new();

                    stmt.query([]).unwrap().for_each(|row| {
                        let id: String = row.get_unwrap("discordID");
                        let ign: String = row.get_unwrap("ign");

                        res.entry(id).or_insert(Vec::new()).push(ign);
                    }).unwrap();

                    Json(res)
                },
            ),
        )
        .with_state(conn);

    let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
