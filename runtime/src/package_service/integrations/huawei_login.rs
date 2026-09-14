use super::secret_store::{
    clear_package_integration_secret, read_package_integration_secret,
    save_package_integration_secret,
};
use super::{PackageError, Value};
use tokio::sync::{Mutex, MutexGuard};

const PENDING: &str = ":huawei-login";
const APP_ID: &str = "10414141";
const SESSION_ORIGIN: &str = "https://healthsession-drru.things.dbankcloud.ru";
const COMMON_ORIGIN: &str = "https://healthcommon-drru.things.dbankcloud.ru";
const DATA_ORIGIN_DR1: &str = "healthdata.dbankcloud.cn";
const DATA_ORIGIN_DR2: &str = "sportdata-dra.things.dbankcloud.com";
const DATA_ORIGIN_DR3: &str = "sportdata-dre.things.dbankcloud.com";
const DATA_ORIGIN_DR4: &str = "sportdata-drru.things.dbankcloud.ru";
const DR1_COUNTRIES: &str = "CN";
// Provenance: Desktop/Huawei Health APK 16.1.6.320 extracted at
// C:\Users\kirill\Desktop\huawei-health-analysis\grs_app_global_route_config.json,
// services.healthcloud.countryGroups (routeBy=reg_country), not the broader
// top-level countryGroups list. DR2=143, DR3=50, DR4=1 in that service.
const DR2_COUNTRIES: &str = "AE,AF,AG,AI,AM,AO,AR,AW,AZ,BB,BD,BF,BH,BI,BJ,BN,BO,BR,BS,BW,BY,BZ,CD,CF,CG,CI,CK,CL,CM,CO,CR,CU,CV,DJ,DO,DZ,EC,EG,ER,ET,FJ,GA,GD,GE,GF,GH,GM,GN,GP,GQ,GT,GW,GY,HK,HN,HT,ID,IN,IQ,JM,JO,JP,KE,KG,KH,KM,KR,KW,KY,KZ,LA,LB,LC,LK,LR,LS,LY,MA,MG,ML,MM,MN,MQ,MR,MS,MU,MV,MW,MX,MY,MZ,NA,NE,NG,NI,NP,NR,OM,PA,PE,PF,PG,PH,PK,PR,PS,PY,QA,RE,RW,SA,SB,SC,SD,SG,SL,SN,SO,SR,ST,SV,SY,SZ,TD,TG,TH,TJ,TN,TO,TT,TZ,TW,UG,UY,UZ,VE,VG,VN,YE,YT,ZA,ZM,ZW";
const DR3_COUNTRIES: &str = "AD,AL,AN,AT,AU,BA,BE,BG,CH,CY,CZ,DE,DK,EE,ES,FI,FO,FR,GB,GI,GL,GR,HR,HU,IE,IS,IT,LI,LT,LU,LV,MC,MD,ME,MK,MT,NL,NO,NZ,PL,PT,RO,RS,SE,SI,SK,SM,TR,UA,VA";
const DR4_COUNTRIES: &str = "RU";
// ponytail: serialize local pending-login mutations; use per-provider locks if login volume warrants it.
static LOGIN_LOCK: Mutex<()> = Mutex::const_new(());

pub(super) async fn lock() -> MutexGuard<'static, ()> {
    LOGIN_LOCK.lock().await
}

pub(super) fn try_lock() -> Result<MutexGuard<'static, ()>, PackageError> {
    LOGIN_LOCK.try_lock().map_err(|_| PackageError::Invalid)
}

pub(super) struct Exchange {
    pub(super) credential: String,
    state: Value,
}

fn data_origin_for_country(country: &str) -> Option<&'static str> {
    [
        (DR1_COUNTRIES, DATA_ORIGIN_DR1),
        (DR2_COUNTRIES, DATA_ORIGIN_DR2),
        (DR3_COUNTRIES, DATA_ORIGIN_DR3),
        (DR4_COUNTRIES, DATA_ORIGIN_DR4),
    ]
    .into_iter()
    .find_map(|(countries, origin)| {
        countries
            .split(',')
            .any(|code| code == country)
            .then_some(origin)
    })
}

fn routing_from_home_country(value: &Value) -> Result<(String, u32), PackageError> {
    if value["resultCode"] != 0 {
        return Err(PackageError::Invalid);
    }
    let use_service_country = value["useSrvNationalCode"]
        .as_bool()
        .ok_or(PackageError::Invalid)?;
    let field = if use_service_country {
        "srvNationalCode"
    } else {
        "nationalCode"
    };
    let country = value[field].as_str().ok_or(PackageError::Invalid)?;
    if country.len() != 2 || !country.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err(PackageError::Invalid);
    }
    let origin = data_origin_for_country(country).ok_or(PackageError::Invalid)?;
    // APK logic gives currentSiteId precedence over the country fallback. A
    // missing or zero site cannot be reconstructed safely, so fail closed.
    let site_id = value["currentSiteId"]
        .as_u64()
        .filter(|site| *site > 0 && *site <= u32::MAX as u64)
        .ok_or(PackageError::Invalid)? as u32;
    Ok((origin.to_owned(), site_id))
}

fn routing_from_session(value: &Value) -> Result<(String, u32), PackageError> {
    if value["routing_verified"] != true {
        return Err(PackageError::Invalid);
    }
    let origin = value["data_origin"].as_str().ok_or(PackageError::Invalid)?;
    if ![
        DATA_ORIGIN_DR1,
        DATA_ORIGIN_DR2,
        DATA_ORIGIN_DR3,
        DATA_ORIGIN_DR4,
    ]
    .contains(&origin)
    {
        return Err(PackageError::Invalid);
    }
    let site_id = value["site_id"]
        .as_u64()
        .filter(|site| *site > 0 && *site <= u32::MAX as u64)
        .ok_or(PackageError::Invalid)? as u32;
    Ok((origin.to_owned(), site_id))
}

fn attach_routing(value: &mut Value, routing: &(String, u32)) {
    value["routing_verified"] = Value::Bool(true);
    value["data_origin"] = Value::String(routing.0.clone());
    value["site_id"] = Value::from(routing.1);
}

pub(crate) fn worker_routing(session: &str) -> Result<(String, u32), PackageError> {
    let value: Value = serde_json::from_str(session).map_err(|_| PackageError::Invalid)?;
    validate_session(&value, 0)?;
    routing_from_session(&value)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub(super) async fn begin(id: &str, version: &str) -> Result<String, PackageError> {
    let _lock = lock().await;
    let state = uuid::Uuid::new_v4().to_string();
    let pending =
        serde_json::json!({"state": state, "expires": now_ms() + 5 * 60_000, "exchanging": false});
    save_package_integration_secret(id, version, PENDING, &pending.to_string())?;
    let mut url = reqwest::Url::parse("https://oauth-login.cloud.huawei.com/oauth2/v3/authorize")
        .map_err(|_| PackageError::Invalid)?;
    url.query_pairs_mut().extend_pairs([
        ("client_id", APP_ID),
        ("redirect_uri", "hms://redirect_url"),
        ("response_type", "code"),
        ("access_type", "offline"),
        ("display", "touch"),
        (
            "scope",
            "openid https://www.huawei.com/auth/account/base.profile",
        ),
        ("state", &state),
        ("nonce", &uuid::Uuid::new_v4().to_string()),
    ]);
    Ok(url.to_string())
}

fn callback_code(callback: &str, pending: &Value, now: u64) -> Result<String, PackageError> {
    if callback.len() > 16384
        || pending["expires"]
            .as_u64()
            .is_none_or(|expires| now >= expires)
        || pending["exchanging"] != false
    {
        return Err(PackageError::Invalid);
    }
    let url = reqwest::Url::parse(callback).map_err(|_| PackageError::Invalid)?;
    if url.scheme() != "hms"
        || url.host_str() != Some("redirect_url")
        || !url.path().is_empty()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
    {
        return Err(PackageError::Invalid);
    }
    let pairs: Vec<_> = url.query_pairs().collect();
    if pairs.len() != 2
        || pairs.iter().filter(|(key, _)| key == "state").count() != 1
        || pairs.iter().filter(|(key, _)| key == "code").count() != 1
        || !pairs
            .iter()
            .any(|(key, value)| key == "state" && pending["state"].as_str() == Some(value.as_ref()))
    {
        return Err(PackageError::Invalid);
    }
    pairs
        .into_iter()
        .find(|(key, value)| key == "code" && !value.is_empty())
        .map(|(_, value)| value.into_owned())
        .ok_or(PackageError::Invalid)
}

fn validate_session(value: &Value, now: u64) -> Result<(), PackageError> {
    if value["resultCode"] != 0
        || value["accessTokenExpireTime"]
            .as_u64()
            .is_none_or(|expiry| expiry <= now)
        || ["uid", "accessToken", "refreshToken"].iter().any(|field| {
            value[*field]
                .as_str()
                .is_none_or(|v| v.is_empty() || v.len() > 8192 || v.chars().any(char::is_control))
        })
    {
        return Err(PackageError::Invalid);
    }
    Ok(())
}

pub(crate) fn account_key(session: &str) -> Result<String, PackageError> {
    use sha2::{Digest, Sha256};
    let session: Value = serde_json::from_str(session).map_err(|_| PackageError::Invalid)?;
    validate_session(&session, 0)?;
    Ok(format!("{:x}", Sha256::digest(format!("huawei-health:{}", session["uid"].as_str().ok_or(PackageError::Invalid)?))))
}

pub(super) async fn exchange(
    id: &str,
    version: &str,
    callback: &str,
) -> Result<Exchange, PackageError> {
    let (code, state) = {
        let _lock = lock().await;
        let mut pending: Value = serde_json::from_str(
            &read_package_integration_secret(id, version, PENDING).ok_or(PackageError::Invalid)?,
        )
        .map_err(|_| PackageError::Invalid)?;
        let code = callback_code(callback, &pending, now_ms())?;
        pending["exchanging"] = Value::Bool(true);
        save_package_integration_secret(id, version, PENDING, &pending.to_string())?;
        (code, pending["state"].clone())
    };
    let mut value = post_session(
        &format!("{SESSION_ORIGIN}/commonAbility/userAccessToken/obtain"),
        &serde_json::json!({"authorizationCode": code, "appId": APP_ID}), None).await?;
    validate_session(&value, now_ms())?;
    let routing = discover_routing(&value).await?;
    attach_routing(&mut value, &routing);
    Ok(Exchange {
        credential: value.to_string(),
        state,
    })
}

async fn post_session(url: &str, body: &Value, uid: Option<&str>) -> Result<Value, PackageError> {
    let mut request = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|_| PackageError::Invalid)?
        .post(url)
        .header("x-ts", now_ms().to_string())
        .header("x-version", "and_health_16.1.6.320")
        .json(body);
    if let Some(uid) = uid { request = request.header("x-huid", uid); }
    let response = request
        .send()
        .await
        .map_err(|_| PackageError::Invalid)?;
    response_json(response).await
}

async fn response_json(mut response: reqwest::Response) -> Result<Value, PackageError> {
    if !response.status().is_success() {
        return Err(PackageError::Invalid);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| PackageError::Invalid)? {
        if bytes.len() + chunk.len() > 65536 {
            return Err(PackageError::Invalid);
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| PackageError::Invalid)
}

async fn discover_routing(session: &Value) -> Result<(String, u32), PackageError> {
    let uid = session["uid"].as_str().ok_or(PackageError::Invalid)?;
    let access_token = session["accessToken"].as_str().ok_or(PackageError::Invalid)?;
    let response = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|_| PackageError::Invalid)?
        .get(format!("{COMMON_ORIGIN}/sessionService/v1/user/homeCountryInfo"))
        .header("Authorization", format!("Bearer {access_token}"))
        .header("x-client-id", APP_ID)
        .header("x-huid", uid)
        .header("x-ts", now_ms().to_string())
        .header("x-version", "and_health_16.1.6.320")
        .send()
        .await
        .map_err(|_| PackageError::Invalid)?;
    routing_from_home_country(&response_json(response).await?)
}

pub(crate) async fn session_for_request(
    id: &str,
    version: &str,
    setting: &str,
    expected: &str,
) -> Result<String, PackageError> {
    refresh_at(id, version, setting, expected,
        &format!("{COMMON_ORIGIN}/commonAbility/userAccessToken/refresh")).await
}

async fn refresh_at(
    id: &str,
    version: &str,
    setting: &str,
    expected: &str,
    url: &str,
) -> Result<String, PackageError> {
    let _lock = lock().await;
    let expected: Value = serde_json::from_str(expected).map_err(|_| PackageError::Invalid)?;
    validate_session(&expected, 0)?;
    // Read the current credential: worker handles may predate a token rotation.
    let mut current: Value = serde_json::from_str(
        &read_package_integration_secret(id, version, setting).ok_or(PackageError::Invalid)?
    ).map_err(|_| PackageError::Invalid)?;
    validate_session(&current, 0)?;
    if current["uid"] != expected["uid"] { return Err(PackageError::Invalid); }
    let routing = routing_from_session(&current)?;
    let recovery_key = format!(":huawei-refresh:{setting}");
    if let Some(saved) = read_package_integration_secret(id, version, &recovery_key) {
        let recovery: Value = serde_json::from_str(&saved).map_err(|_| PackageError::Invalid)?;
        if recovery["source"] == current["refreshToken"] {
            let updated = &recovery["session"];
            validate_session(updated, 0)?;
            if updated["uid"] != current["uid"] { return Err(PackageError::Invalid); }
            let mut recovered = updated.clone();
            attach_routing(&mut recovered, &routing);
            save_package_integration_secret(id, version, setting, &recovered.to_string())?;
            current = recovered;
        }
        // A new login/replacement supersedes any older rotation journal.
        clear_package_integration_secret(id, version, &recovery_key)?;
    }
    if current["accessTokenExpireTime"].as_u64().unwrap_or(0) > now_ms() + 60_000 {
        return Ok(current.to_string());
    }
    let updated = post_session(url, &serde_json::json!({
        "refreshToken": current["refreshToken"], "appId": APP_ID
    }), current["uid"].as_str()).await?;
    validate_session(&updated, now_ms())?;
    if updated["uid"] != current["uid"] { return Err(PackageError::Invalid); }
    let mut updated = updated;
    attach_routing(&mut updated, &routing);
    // Persist the rotated token before replacing the primary credential.
    let recovery = serde_json::json!({"source":current["refreshToken"], "session":updated});
    save_package_integration_secret(id, version, &recovery_key, &recovery.to_string())?;
    save_package_integration_secret(id, version, setting, &updated.to_string())?;
    clear_package_integration_secret(id, version, &recovery_key)?;
    Ok(updated.to_string())
}

// Caller holds the login lock through publication and worker activation.
pub(super) fn prepare_commit(
    id: &str,
    version: &str,
    exchanged: &Exchange,
) -> Result<(), PackageError> {
    let mut pending: Value = serde_json::from_str(
        &read_package_integration_secret(id, version, PENDING).ok_or(PackageError::Invalid)?,
    )
    .map_err(|_| PackageError::Invalid)?;
    if pending["state"] != exchanged.state || pending["exchanging"] != true {
        return Err(PackageError::Invalid);
    }
    pending["session"] =
        serde_json::from_str(&exchanged.credential).map_err(|_| PackageError::Invalid)?;
    save_package_integration_secret(id, version, PENDING, &pending.to_string())
}

pub(super) fn cancel(id: &str, version: &str) -> Result<(), PackageError> {
    clear_package_integration_secret(id, version, PENDING)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grs_mapping_uses_home_country_and_current_site() {
        assert_eq!(DR1_COUNTRIES.split(',').count(), 1);
        assert_eq!(DR2_COUNTRIES.split(',').count(), 143);
        assert_eq!(DR3_COUNTRIES.split(',').count(), 50);
        assert_eq!(DR4_COUNTRIES.split(',').count(), 1);
        for excluded in ["US", "CA", "AQ", "AS", "AX", "BQ"] {
            assert_eq!(data_origin_for_country(excluded), None, "broad-group country {excluded} must not route");
        }
        let home = serde_json::json!({
            "resultCode": 0,
            "nationalCode": "FR",
            "srvNationalCode": "RU",
            "useSrvNationalCode": false,
            "currentSiteId": 7
        });
        assert_eq!(routing_from_home_country(&home).unwrap(), (DATA_ORIGIN_DR3.into(), 7));
        let mut missing_site = home.clone();
        missing_site["currentSiteId"] = Value::from(0);
        assert!(routing_from_home_country(&missing_site).is_err());

        let service_country = serde_json::json!({
            "resultCode": 0,
            "nationalCode": "FR",
            "srvNationalCode": "RU",
            "useSrvNationalCode": true,
            "currentSiteId": 7
        });
        assert_eq!(routing_from_home_country(&service_country).unwrap(), (DATA_ORIGIN_DR4.into(), 7));
        assert!(routing_from_home_country(&serde_json::json!({
            "resultCode": 0,
            "nationalCode": "IR",
            "srvNationalCode": "IR",
            "useSrvNationalCode": false,
            "currentSiteId": 7
        })).is_err());
    }

    #[tokio::test]
    async fn huawei_refresh_rotates_recovers_and_binds_account() {
        use std::io::{Read, Write};
        let id = "huawei-refresh-test";
        let old = serde_json::json!({"resultCode":0,"uid":"u","accessToken":"old","refreshToken":"r","accessTokenExpireTime":1,"routing_verified":true,"data_origin":DATA_ORIGIN_DR3,"site_id":7});
        let updated = serde_json::json!({"resultCode":0,"uid":"u","accessToken":"new","refreshToken":"r2","accessTokenExpireTime":now_ms()+3_600_000,"routing_verified":true,"data_origin":DATA_ORIGIN_DR3,"site_id":7});
        save_package_integration_secret(id, "1", "session", &old.to_string()).unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/refresh", listener.local_addr().unwrap());
        let body = serde_json::json!({"resultCode":0,"uid":"u","accessToken":"new","refreshToken":"r2","accessTokenExpireTime":updated["accessTokenExpireTime"]}).to_string();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut chunk = [0; 1024];
                let count = stream.read(&mut chunk).unwrap();
                assert!(count > 0);
                bytes.extend_from_slice(&chunk[..count]);
                if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                    let len: usize = headers.lines().find_map(|line| line.strip_prefix("content-length: ")).unwrap().parse().unwrap();
                    if bytes.len() < end + 4 + len { continue; }
                    assert!(headers.contains("x-huid: u"));
                    let request: Value = serde_json::from_slice(&bytes[end+4..end+4+len]).unwrap();
                    assert_eq!(request, serde_json::json!({"refreshToken":"r","appId":"10414141"}));
                    break;
                }
            }
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
        });
        let actual = refresh_at(id, "1", "session", &old.to_string(), &url).await.unwrap();
        server.join().unwrap();
        assert_eq!(serde_json::from_str::<Value>(&actual).unwrap(), updated);
        // The server is gone: this must use the saved rotated credential.
        assert_eq!(refresh_at(id, "1", "session", &old.to_string(), &url).await.unwrap(), actual);
        let recovery = serde_json::json!({"source":"r", "session":updated});
        save_package_integration_secret(id, "1", "session", &old.to_string()).unwrap();
        save_package_integration_secret(id, "1", ":huawei-refresh:session", &recovery.to_string()).unwrap();
        assert_eq!(refresh_at(id, "1", "session", &old.to_string(), &url).await.unwrap(), actual);
        let mut other = updated.clone();
        other["uid"] = Value::String("other".into());
        save_package_integration_secret(id, "1", "session", &other.to_string()).unwrap();
        assert!(refresh_at(id, "1", "session", &old.to_string(), &url).await.is_err());
        clear_package_integration_secret(id, "1", "session").unwrap();
        assert!(refresh_at(id, "1", "session", &old.to_string(), &url).await.is_err());
    }
    #[tokio::test]
    async fn huawei_cancel_or_new_login_rejects_old_publication() {
        let id = "huawei-publication-test";
        begin(id, "1").await.unwrap();
        let mut pending: Value =
            serde_json::from_str(&read_package_integration_secret(id, "1", PENDING).unwrap())
                .unwrap();
        pending["exchanging"] = Value::Bool(true);
        save_package_integration_secret(id, "1", PENDING, &pending.to_string()).unwrap();
        let exchanged = Exchange {
            state: pending["state"].clone(),
            credential: "{\"accessToken\":\"secret\"}".into(),
        };
        {
            let _lock = lock().await;
            cancel(id, "1").unwrap();
            assert!(prepare_commit(id, "1", &exchanged).is_err());
        }
        begin(id, "1").await.unwrap();
        {
            let _lock = lock().await;
            assert!(prepare_commit(id, "1", &exchanged).is_err());
            save_package_integration_secret(id, "1", PENDING, &pending.to_string()).unwrap();
            prepare_commit(id, "1", &exchanged).unwrap();
            let saved: Value =
                serde_json::from_str(&read_package_integration_secret(id, "1", PENDING).unwrap())
                    .unwrap();
            assert_eq!(saved["session"]["accessToken"], "secret");
            cancel(id, "1").unwrap();
        }
    }
    #[test]
    fn huawei_callback_and_session_validation() {
        let pending = serde_json::json!({"state":"expected", "expires":200, "exchanging":false});
        assert_eq!(
            callback_code(
                "hms://redirect_url?state=expected&code=a%2Bb%3D",
                &pending,
                100
            )
            .unwrap(),
            "a+b="
        );
        for url in [
            "hms://redirect_url?state=wrong&code=a",
            "hms://redirect_url?state=expected&code=a&code=b",
            "https://redirect_url?state=expected&code=a",
            "hms://redirect_url?state=expected&error=denied",
        ] {
            assert!(callback_code(url, &pending, 100).is_err());
        }
        assert!(callback_code("hms://redirect_url?state=expected&code=a", &pending, 200).is_err());
        let good = serde_json::json!({"resultCode":0,"uid":"u","accessToken":"a","refreshToken":"r","accessTokenExpireTime":200});
        assert!(validate_session(&good, 100).is_ok());
        assert!(validate_session(&good, 200).is_err());
        let mut bad = good;
        bad["refreshToken"] = Value::Null;
        assert!(validate_session(&bad, 100).is_err());
    }

    #[test]
    fn huawei_account_key_binds_uid_across_token_rotation() {
        let session = serde_json::json!({
            "resultCode": 0,
            "uid": "account-a",
            "accessToken": "access-1",
            "refreshToken": "refresh-1",
            "accessTokenExpireTime": u64::MAX,
        });
        let mut rotated = session.clone();
        rotated["accessToken"] = Value::String("access-2".into());
        rotated["refreshToken"] = Value::String("refresh-2".into());
        let mut other_account = session.clone();
        other_account["uid"] = Value::String("account-b".into());

        let key = account_key(&session.to_string()).unwrap();
        assert_eq!(key, account_key(&rotated.to_string()).unwrap());
        assert_ne!(key, account_key(&other_account.to_string()).unwrap());
        assert_eq!(key.len(), 64);
        assert!(key.bytes().all(|byte| byte.is_ascii_hexdigit()));

        assert!(account_key("{}").is_err());
    }
}
