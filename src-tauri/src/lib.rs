use std::{fs, path::PathBuf, time::{Duration, SystemTime, UNIX_EPOCH}};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use futures_util::StreamExt;
use jsonwebtoken::{decode, decode_header, jwk::JwkSet, DecodingKey, Validation};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpListener, time::timeout};
use url::Url;

const AUTH_URL: &str = "https://auth.openai.com/api/accounts/authorize";
const TOKEN_URL: &str = "https://auth.openai.com/api/accounts/oauth/token";
const JWKS_URL: &str = "https://auth.openai.com/.well-known/jwks.json";
const API_BASE: &str = "https://api.openai.com/v1";
const RESOURCE: &str = "https://api.openai.com/v1";
const ISSUER: &str = "https://auth.openai.com";
const SCOPES: &str = "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";
const AGENT_NAME: &str = "N3 Loop";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Profile {
    email: String,
    #[serde(default)]
    name: String,
    issuer: String,
    subject: String,
    client_id: String,
    ext_agent_host_id: String,
    id_token: String,
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    token_type: String,
    expires_in: u64,
    scopes: Vec<String>,
    saved_at: i64,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    token_type: Option<String>,
    expires_in: u64,
    #[serde(default)]
    scope: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenIdDiscovery {
    #[serde(default)]
    revocation_endpoint: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IdClaims {
    sub: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    name: String,
    iss: String,
    exp: usize,
    #[serde(default)]
    nonce: String,
    #[serde(default)]
    aud: Value,
}

#[derive(Debug, Serialize)]
struct ChatGptStatus {
    connected: bool,
    plan_usage: bool,
    email: String,
    model: String,
}

#[derive(Debug, Serialize)]
struct TranslationResult {
    translation: String,
    model: String,
}

fn now_ts() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64
}

fn app_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let p = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&p).map_err(|e| e.to_string())?;
    Ok(p)
}

fn profile_path(app: &AppHandle) -> Result<PathBuf, String> { Ok(app_dir(app)?.join("chatgpt-profile.json")) }
fn host_path(app: &AppHandle) -> Result<PathBuf, String> { Ok(app_dir(app)?.join("chatgpt-host.json")) }

fn load_profile(app: &AppHandle) -> Result<Option<Profile>, String> {
    let p = profile_path(app)?;
    if !p.exists() { return Ok(None); }
    let raw = fs::read_to_string(p).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map(Some).map_err(|e| e.to_string())
}

fn save_profile(app: &AppHandle, p: &Profile) -> Result<(), String> {
    let path = profile_path(app)?;
    let tmp = path.with_extension("json.tmp");
    let raw = serde_json::to_vec_pretty(p).map_err(|e| e.to_string())?;
    fs::write(&tmp, raw).map_err(|e| e.to_string())?;
    fs::rename(tmp, path).map_err(|e| e.to_string())?;
    Ok(())
}

fn load_or_create_host_id(app: &AppHandle) -> Result<String, String> {
    let p = host_path(app)?;
    if p.exists() {
        let raw = fs::read_to_string(&p).map_err(|e| e.to_string())?;
        let v: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        if let Some(s) = v.get("ext_agent_host_id").and_then(Value::as_str) { if !s.is_empty() { return Ok(s.to_string()); } }
    }
    let id = format!("urn:uuid:{}", uuid::Uuid::new_v4());
    fs::write(&p, serde_json::to_vec_pretty(&json!({"ext_agent_host_id": id})).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(id)
}

fn random_b64(bytes: usize) -> String {
    let mut b = vec![0u8; bytes]; OsRng.fill_bytes(&mut b); URL_SAFE_NO_PAD.encode(b)
}

async fn verify_id_token(client: &reqwest::Client, token: &str, client_id: &str, nonce: &str) -> Result<IdClaims, String> {
    let header = decode_header(token).map_err(|e| format!("ID 토큰 헤더 확인 실패: {e}"))?;
    let kid = header.kid.as_deref().ok_or("ID 토큰에 kid가 없습니다.")?;
    let set: JwkSet = client.get(JWKS_URL).send().await.map_err(|e| e.to_string())?.error_for_status().map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    let jwk = set.find(kid).ok_or("OpenAI 서명 키를 찾지 못했습니다.")?;
    let key = DecodingKey::from_jwk(jwk).map_err(|e| e.to_string())?;
    let mut validation = Validation::new(header.alg);
    validation.set_issuer(&[ISSUER]);
    validation.set_audience(&[client_id]);
    let data = decode::<IdClaims>(token, &key, &validation).map_err(|e| format!("ID 토큰 검증 실패: {e}"))?;
    if data.claims.nonce != nonce { return Err("로그인 nonce가 일치하지 않습니다.".into()); }
    if data.claims.iss != ISSUER { return Err("로그인 발급자가 일치하지 않습니다.".into()); }
    Ok(data.claims)
}

async fn read_callback(listener: TcpListener, expected_state: String) -> Result<(String, Option<String>), String> {
    let (mut socket, _) = timeout(Duration::from_secs(300), listener.accept()).await.map_err(|_| "ChatGPT 로그인 시간이 초과되었습니다.".to_string())?.map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 32768];
    let n = timeout(Duration::from_secs(10), socket.read(&mut buf)).await.map_err(|_| "로그인 콜백 읽기 시간이 초과되었습니다.".to_string())?.map_err(|e| e.to_string())?;
    let req = String::from_utf8_lossy(&buf[..n]);
    let first = req.lines().next().ok_or("잘못된 로그인 콜백입니다.")?;
    let target = first.split_whitespace().nth(1).ok_or("잘못된 로그인 요청입니다.")?;
    let url = Url::parse(&format!("http://127.0.0.1{target}")).map_err(|e| e.to_string())?;
    let q: std::collections::HashMap<String,String> = url.query_pairs().into_owned().collect();
    if q.get("state") != Some(&expected_state) { return Err("로그인 state가 일치하지 않습니다.".into()); }
    if let Some(err) = q.get("error") { return Err(format!("ChatGPT 로그인이 취소되었거나 거부되었습니다: {err}")); }
    let code = q.get("code").cloned().ok_or("로그인 코드가 없습니다.")?;
    let issued = q.get("client_id").cloned();
    let body = r#"<!doctype html><html lang="ko"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>N3 Loop 로그인 완료</title><style>body{font-family:-apple-system,BlinkMacSystemFont,'Malgun Gothic',sans-serif;margin:0;min-height:100vh;display:grid;place-items:center;background:#f6f7f5;color:#182620}.box{text-align:center;padding:28px}.ok{width:58px;height:58px;border-radius:18px;background:#235b45;color:white;display:grid;place-items:center;margin:0 auto 16px;font-size:28px}h1{font-size:22px}p{color:#6d7b72;font-size:13px;line-height:1.7}</style><div class="box"><div class="ok">✓</div><h1>ChatGPT 연결 완료</h1><p>N3 Loop로 돌아가면 자동으로 계속 진행됩니다.<br>이 창은 닫아도 됩니다.</p></div><script>setTimeout(()=>window.close(),900)</script></html>"#;
    let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.as_bytes().len(), body);
    let _ = socket.write_all(response.as_bytes()).await;
    let _ = socket.shutdown().await;
    Ok((code, issued))
}

async fn refresh_if_needed(app: &AppHandle, mut p: Profile, force: bool) -> Result<Profile, String> {
    let expires_at = p.saved_at + p.expires_in as i64;
    if !force && now_ts() < expires_at - 120 { return Ok(p); }
    let client = reqwest::Client::new();
    let res = client.post(TOKEN_URL).form(&[
        ("grant_type", "refresh_token"),
        ("client_id", p.client_id.as_str()),
        ("refresh_token", p.refresh_token.as_deref().ok_or("저장된 ChatGPT 갱신 토큰이 없습니다. 다시 로그인해 주세요.")?),
        ("resource", RESOURCE),
    ]).send().await.map_err(|e| e.to_string())?;
    if !res.status().is_success() { return Err(format!("ChatGPT 로그인 갱신 실패: {}", res.text().await.unwrap_or_default())); }
    let t: TokenResponse = res.json().await.map_err(|e| e.to_string())?;
    p.access_token = t.access_token;
    if let Some(refresh) = t.refresh_token { p.refresh_token = Some(refresh); }
    if let Some(id) = t.id_token { p.id_token = id; }
    p.token_type = t.token_type.unwrap_or_else(|| "Bearer".into());
    p.expires_in = t.expires_in;
    if let Some(scope) = t.scope { p.scopes = scope.split_whitespace().map(str::to_string).collect(); }
    p.saved_at = now_ts(); save_profile(app, &p)?; Ok(p)
}

async fn choose_model(access_token: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let res = client.get(format!("{API_BASE}/models")).bearer_auth(access_token).send().await.map_err(|e| e.to_string())?;
    if !res.status().is_success() { return Err(format!("모델 목록을 불러오지 못했습니다: {}", res.text().await.unwrap_or_default())); }
    let v: Value = res.json().await.map_err(|e| e.to_string())?;
    let models = v.get("models").or_else(|| v.get("data")).and_then(Value::as_array).ok_or("사용 가능한 모델 목록이 비어 있습니다.")?;
    let mut visible = Vec::new();
    for m in models {
        let vis = m.get("visibility").and_then(Value::as_str).unwrap_or("list");
        if vis != "list" { continue; }
        if let Some(slug) = m.get("slug").or_else(|| m.get("id")).and_then(Value::as_str) { visible.push(slug.to_string()); }
    }
    if visible.is_empty() { return Err("ChatGPT 플랜에서 사용할 수 있는 모델을 찾지 못했습니다.".into()); }
    for preferred in ["gpt-6.1-sol", "gpt-6-astra", "gpt-5.6-sol"] {
        if visible.iter().any(|m| m == preferred) { return Ok(preferred.to_string()); }
    }
    visible.into_iter().find(|m| !m.to_ascii_lowercase().contains("codex")).or_else(|| models.first().and_then(|m| m.get("slug").or_else(||m.get("id")).and_then(Value::as_str).map(str::to_string))).ok_or_else(|| "사용 가능한 일반 모델을 찾지 못했습니다.".into())
}

async fn translate_with_token(access_token: &str, model: &str, artist: &str, title: &str, lyrics: &str) -> Result<String, String> {
    let instructions = "사용자가 직접 제공한 일본어 노래 가사를 한국어 학습용으로 번역한다. 원문의 줄바꿈과 행 순서를 최대한 1:1로 유지한다. 출력에는 한국어 번역만 포함하고 일본어 원문, 번호, 마크다운, 설명, 저작권 안내를 추가하지 않는다. 노래 전체 문맥과 비유를 고려하되 지나친 창작 의역은 피한다.";
    let input = format!("가수: {artist}\n곡명: {title}\n\n[사용자가 제공한 가사]\n{lyrics}");
    let body = json!({"model":model,"instructions":instructions,"input":[{"role":"user","content":input}],"store":false,"stream":true});
    let client = reqwest::Client::new();
    let res = client.post(format!("{API_BASE}/responses")).bearer_auth(access_token).json(&body).send().await.map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        let status = res.status();
        return Err(format!("ChatGPT 해석 요청 실패 (HTTP {}): {}", status.as_u16(), res.text().await.unwrap_or_default()));
    }
    let mut stream = res.bytes_stream();
    let mut buf = String::new(); let mut out = String::new(); let mut completed = false;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        buf.push_str(&String::from_utf8_lossy(&chunk));
        buf = buf.replace("\r\n", "\n");
        while let Some(pos) = buf.find("\n\n") {
            let frame = buf[..pos].to_string(); buf.drain(..pos+2);
            for line in frame.lines() {
                let Some(data) = line.strip_prefix("data:") else { continue; };
                let data = data.trim(); if data.is_empty() || data == "[DONE]" { continue; }
                let Ok(v) = serde_json::from_str::<Value>(data) else { continue; };
                match v.get("type").and_then(Value::as_str).unwrap_or("") {
                    "response.output_text.delta" => if let Some(d) = v.get("delta").and_then(Value::as_str) { out.push_str(d); },
                    "response.completed" => completed = true,
                    "response.failed" => {
                        let msg = v.pointer("/response/error/message").and_then(Value::as_str).or_else(||v.pointer("/response/error/code").and_then(Value::as_str)).unwrap_or("응답 생성에 실패했습니다.");
                        return Err(msg.to_string());
                    },
                    "response.incomplete" => return Err("ChatGPT 응답이 완성되기 전에 종료되었습니다.".into()),
                    "error" => return Err(v.get("message").and_then(Value::as_str).unwrap_or("ChatGPT 스트림 오류").to_string()),
                    _ => {}
                }
            }
        }
    }
    if !completed { return Err("ChatGPT 응답 완료 신호를 받지 못했습니다.".into()); }
    if out.trim().is_empty() { return Err("ChatGPT 해석 결과가 비어 있습니다.".into()); }
    Ok(out.trim().to_string())
}

#[tauri::command]
async fn chatgpt_status(app: AppHandle) -> Result<ChatGptStatus, String> {
    let Some(p) = load_profile(&app)? else { return Ok(ChatGptStatus{connected:false,plan_usage:false,email:String::new(),model:String::new()}); };
    let p = match refresh_if_needed(&app, p, false).await { Ok(p)=>p, Err(_)=>return Ok(ChatGptStatus{connected:false,plan_usage:false,email:String::new(),model:String::new()}) };
    let plan = p.scopes.iter().any(|s| s == "chatgpt.tokens.use.direct");
    let model = if plan { choose_model(&p.access_token).await.unwrap_or_default() } else { String::new() };
    Ok(ChatGptStatus{connected:true,plan_usage:plan,email:p.email,model})
}

#[tauri::command]
async fn chatgpt_sign_in(app: AppHandle) -> Result<ChatGptStatus, String> {
    let host_id = load_or_create_host_id(&app)?;
    let existing = load_profile(&app)?;
    let state = random_b64(32); let nonce = random_b64(32); let verifier = random_b64(64);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}/auth/callback");
    let initial = existing.is_none();
    let client_id = existing.as_ref().map(|p|p.client_id.clone()).unwrap_or_else(||"dynamic_agent_client".into());
    let mut url = Url::parse(AUTH_URL).map_err(|e| e.to_string())?;
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("client_id", &client_id)
            .append_pair("ext_agent_host_id", &host_id)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", &redirect_uri)
            .append_pair("scope", SCOPES)
            .append_pair("resource", RESOURCE)
            .append_pair("state", &state)
            .append_pair("nonce", &nonce)
            .append_pair("code_challenge_method", "S256")
            .append_pair("code_challenge", &challenge);
        if initial { q.append_pair("agent_name_hint", AGENT_NAME); }
        if let Some(p) = existing.as_ref() {
            if !p.id_token.is_empty() { q.append_pair("id_token_hint", &p.id_token); }
            if !p.email.is_empty() { q.append_pair("login_hint", &p.email); }
        }
    }
    app.opener().open_url(url.as_str(), None::<&str>).map_err(|e| e.to_string())?;
    let (code, callback_client) = read_callback(listener, state).await?;
    let issued_client = if initial { callback_client.ok_or("OpenAI가 발급한 client_id를 반환하지 않았습니다.")? } else {
        if let Some(cb)=callback_client { if cb != client_id { return Err("로그인 client_id가 기존 연결과 일치하지 않습니다.".into()); } }
        client_id
    };
    let http = reqwest::Client::new();
    let res = http.post(TOKEN_URL).form(&[
        ("grant_type","authorization_code"),
        ("client_id",issued_client.as_str()),
        ("code",code.as_str()),
        ("code_verifier",verifier.as_str()),
        ("redirect_uri",redirect_uri.as_str()),
        ("resource",RESOURCE),
    ]).send().await.map_err(|e| e.to_string())?;
    if !res.status().is_success() { return Err(format!("ChatGPT 로그인 교환 실패: {}", res.text().await.unwrap_or_default())); }
    let t: TokenResponse = res.json().await.map_err(|e| e.to_string())?;
    let id_token = t.id_token.clone().ok_or("로그인 응답에 ID 토큰이 없습니다.")?;
    let claims = verify_id_token(&http, &id_token, &issued_client, &nonce).await?;
    if let Some(old) = existing.as_ref() { if !old.subject.is_empty() && old.subject != claims.sub { return Err("기존 ChatGPT 계정과 다른 계정이 선택되었습니다. 연결 해제 후 다시 시도해 주세요.".into()); } }
    let scopes: Vec<String> = t.scope.as_deref().unwrap_or("").split_whitespace().map(str::to_string).collect();
    let refresh_token = t.refresh_token.ok_or("로그인 응답에 갱신 토큰이 없습니다.")?;
    let p = Profile { email: claims.email, name: claims.name, issuer: claims.iss, subject: claims.sub, client_id: issued_client, ext_agent_host_id: host_id, id_token, access_token:t.access_token, refresh_token:Some(refresh_token), token_type:t.token_type.unwrap_or_else(||"Bearer".into()), expires_in:t.expires_in, scopes, saved_at:now_ts() };
    save_profile(&app,&p)?;
    let plan = p.scopes.iter().any(|s| s=="chatgpt.tokens.use.direct");
    if !plan { return Err("ChatGPT 플랜 사용 권한이 허용되지 않았습니다. 로그인 화면에서 플랜 사용을 허용해 주세요.".into()); }
    let model = choose_model(&p.access_token).await.unwrap_or_default();
    Ok(ChatGptStatus{connected:true,plan_usage:true,email:p.email,model})
}

#[tauri::command]
async fn chatgpt_translate(app: AppHandle, lyrics: String, artist: String, title: String) -> Result<TranslationResult, String> {
    if lyrics.trim().is_empty() { return Err("가사가 비어 있습니다.".into()); }
    let p = load_profile(&app)?.ok_or("ChatGPT 로그인이 필요합니다.")?;
    let mut p = refresh_if_needed(&app, p, false).await?;
    if !p.scopes.iter().any(|s| s=="chatgpt.tokens.use.direct") { return Err("ChatGPT 플랜 사용 권한이 없습니다. 다시 로그인해 주세요.".into()); }
    let model = choose_model(&p.access_token).await?;
    match translate_with_token(&p.access_token,&model,&artist,&title,&lyrics).await {
        Ok(translation)=>Ok(TranslationResult{translation,model}),
        Err(first) if first.contains("401") || first.to_ascii_lowercase().contains("unauthorized") => {
            p = refresh_if_needed(&app,p,true).await?;
            let model = choose_model(&p.access_token).await?;
            let translation = translate_with_token(&p.access_token,&model,&artist,&title,&lyrics).await?;
            Ok(TranslationResult{translation,model})
        },
        Err(e)=>Err(e)
    }
}

#[tauri::command]
async fn chatgpt_sign_out(app: AppHandle) -> Result<(), String> {
    if let Some(p)=load_profile(&app)? {
        if let Some(refresh) = p.refresh_token.as_deref() {
            let client = reqwest::Client::new();
            if let Ok(discovery_res) = client.get("https://auth.openai.com/.well-known/openid-configuration").send().await {
                if let Ok(discovery_res) = discovery_res.error_for_status() {
                    if let Ok(discovery) = discovery_res.json::<OpenIdDiscovery>().await {
                        if let Some(endpoint) = discovery.revocation_endpoint {
                            let _ = client.post(endpoint).form(&[
                                ("token", refresh),
                                ("token_type_hint", "refresh_token"),
                                ("client_id", p.client_id.as_str()),
                            ]).send().await;
                        }
                    }
                }
            }
        }
    }
    let path = profile_path(&app)?;
    if path.exists() { let _ = fs::remove_file(path); }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![chatgpt_status, chatgpt_sign_in, chatgpt_translate, chatgpt_sign_out])
        .run(tauri::generate_context!())
        .expect("error while running N3 Loop");
}
