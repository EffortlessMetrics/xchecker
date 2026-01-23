const ALNUM: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
const ALNUM_UPPER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
const BASE64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const BASE64_URL: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-";
const TOKEN_SAFE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-";
const SLACK_SAFE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-";
const SIG_SAFE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789%+/=";
const AZURE_SECRET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789~._-";

fn make_from(alphabet: &[u8], len: usize, seed: usize) -> String {
    let mut output = String::with_capacity(len);
    let mut idx = seed % alphabet.len();

    for _ in 0..len {
        output.push(alphabet[idx] as char);
        idx = (idx + 7) % alphabet.len();
    }

    output
}

/// Check whether real LLM integration tests should run.
///
/// `XCHECKER_SKIP_LLM_TESTS=1` always disables real LLM tests.
/// `XCHECKER_REAL_LLM_TESTS=1` enables real LLM tests.
#[must_use]
pub fn llm_tests_enabled() -> bool {
    let skip = std::env::var("XCHECKER_SKIP_LLM_TESTS")
        .ok()
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if skip {
        return false;
    }

    std::env::var("XCHECKER_REAL_LLM_TESTS")
        .ok()
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

pub fn github_pat() -> String {
    format!("ghp_{}", make_from(ALNUM, 36, 1))
}

pub fn github_oauth_token() -> String {
    format!("gho_{}", make_from(ALNUM, 36, 2))
}

pub fn github_app_token() -> String {
    format!("ghu_{}", make_from(ALNUM, 36, 3))
}

pub fn gitlab_token() -> String {
    format!("glpat-{}", make_from(ALNUM, 24, 4))
}

pub fn slack_bot_token() -> String {
    format!("xoxb-{}", make_from(SLACK_SAFE, 24, 5))
}

pub fn slack_user_token() -> String {
    format!("xoxp-{}", make_from(SLACK_SAFE, 24, 6))
}

pub fn stripe_key_live() -> String {
    format!("sk_live_{}", make_from(ALNUM, 24, 7))
}

pub fn stripe_key_test() -> String {
    format!("sk_test_{}", make_from(ALNUM, 24, 8))
}

pub fn twilio_key() -> String {
    format!("SK{}", make_from(ALNUM, 32, 9))
}

pub fn sendgrid_key() -> String {
    format!(
        "SG.{}.{}",
        make_from(BASE64_URL, 22, 10),
        make_from(BASE64_URL, 43, 11)
    )
}

pub fn npm_token() -> String {
    format!("npm_{}", make_from(ALNUM, 36, 12))
}

pub fn pypi_token() -> String {
    format!("pypi-{}", make_from(BASE64_URL, 50, 13))
}

pub fn aws_access_key_id() -> String {
    format!("AKIA{}", make_from(ALNUM_UPPER, 16, 14))
}

pub fn aws_secret_access_key() -> String {
    format!("AWS_SECRET_ACCESS_KEY={}", make_from(BASE64, 40, 15))
}

pub fn aws_secret_access_key_value() -> String {
    format!("secret_access_key={}", make_from(BASE64, 40, 16))
}

pub fn aws_session_token() -> String {
    format!("AWS_SESSION_TOKEN={}", make_from(BASE64, 120, 17))
}

pub fn aws_session_token_value() -> String {
    format!("session_token={}", make_from(BASE64, 120, 18))
}

pub fn gcp_api_key() -> String {
    format!("AIza{}", make_from(BASE64_URL, 35, 19))
}

pub fn gcp_oauth_client_secret() -> String {
    format!("client_secret={}", make_from(BASE64_URL, 24, 20))
}

pub fn azure_storage_key_value() -> String {
    make_from(BASE64, 88, 21)
}

pub fn azure_storage_key_assignment() -> String {
    format!("AccountKey={}", azure_storage_key_value())
}

pub fn azure_connection_string() -> String {
    let account = make_from(ALNUM, 12, 22);
    let key = azure_storage_key_value();
    format!(
        "DefaultEndpointsProtocol=https;AccountName={};AccountKey={}",
        account, key
    )
}

pub fn azure_sas_signature() -> String {
    make_from(SIG_SAFE, 48, 23)
}

pub fn azure_sas_url() -> String {
    format!(
        "https://example.blob.core.windows.net/container?sv=2020-08-04&sig={}",
        azure_sas_signature()
    )
}

pub fn azure_client_secret() -> String {
    format!("AZURE_CLIENT_SECRET={}", make_from(AZURE_SECRET, 34, 24))
}

pub fn bearer_token() -> String {
    format!("Bearer {}", make_from(TOKEN_SAFE, 24, 25))
}

pub fn api_key_header() -> String {
    format!("x-api-key={}", make_from(ALNUM, 24, 26))
}

pub fn oauth_access_token() -> String {
    format!("access_token={}", make_from(TOKEN_SAFE, 24, 27))
}

pub fn oauth_refresh_token() -> String {
    format!("refresh_token={}", make_from(TOKEN_SAFE, 24, 28))
}

pub fn jwt_token() -> String {
    format!(
        "eyJ{}.eyJ{}.{}",
        make_from(BASE64_URL, 20, 29),
        make_from(BASE64_URL, 20, 30),
        make_from(BASE64_URL, 20, 31)
    )
}

pub fn authorization_basic() -> String {
    format!("Basic {}", make_from(BASE64, 24, 32))
}

pub fn postgres_url() -> String {
    let user = make_from(ALNUM, 6, 33);
    let pass = make_from(ALNUM, 10, 34);
    let db = make_from(ALNUM, 6, 35);
    let scheme = "postgres";
    format!("{scheme}://{user}:{pass}@localhost:5432/{db}")
}

pub fn mysql_url() -> String {
    let user = make_from(ALNUM, 6, 36);
    let pass = make_from(ALNUM, 10, 37);
    let db = make_from(ALNUM, 6, 38);
    let scheme = "mysql";
    format!("{scheme}://{user}:{pass}@localhost:3306/{db}")
}

pub fn sqlserver_url() -> String {
    let user = make_from(ALNUM, 6, 39);
    let pass = make_from(ALNUM, 10, 40);
    format!("sqlserver://{}:{}@localhost:1433/db", user, pass)
}

pub fn mongodb_url() -> String {
    let user = make_from(ALNUM, 6, 41);
    let pass = make_from(ALNUM, 10, 42);
    let scheme = "mongodb+srv";
    format!("{scheme}://{user}:{pass}@cluster0.mongodb.net/db")
}

pub fn redis_url() -> String {
    let pass = make_from(ALNUM, 10, 43);
    let scheme = "redis";
    format!("{scheme}://:{pass}@localhost:6379")
}

pub fn nuget_key_assignment() -> String {
    format!("nuget_key={}", make_from(ALNUM, 46, 44))
}

pub fn docker_auth_json() -> String {
    format!(r#"{{"auth":"{}"}}"#, make_from(BASE64, 24, 45))
}

pub fn pem_marker(label: &str) -> String {
    format!("-----BEGIN {}PRIVATE KEY-----", label)
}

pub fn pem_block(label: &str) -> String {
    let begin = pem_marker(label);
    let end = format!("-----END {}PRIVATE KEY-----", label);
    let body = make_from(BASE64, 48, 46);
    format!("{}\n{}\n{}", begin, body, end)
}
