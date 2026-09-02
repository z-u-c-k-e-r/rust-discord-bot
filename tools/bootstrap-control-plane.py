#!/usr/bin/env python3
from pathlib import Path
import subprocess
import textwrap

ROOT = Path(__file__).resolve().parents[1]


def write(path: str, content: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(textwrap.dedent(content).lstrip(), encoding="utf-8")


workspace = ROOT / "Cargo.toml"
text = workspace.read_text(encoding="utf-8")
if '"crates/storage"' not in text:
    text = text.replace('    "crates/lua-runtime",\n', '    "crates/lua-runtime",\n    "crates/storage",\n')
if 'async-trait = ' not in text:
    text = text.replace('[workspace.dependencies]\n', '[workspace.dependencies]\nasync-trait = "0.1.89"\nchrono = { version = "0.4.42", features = ["serde"] }\n')
if 'redis = ' not in text:
    text = text.replace(
        'mlua = { version = "0.12.1", features = ["error-send", "lua54", "send", "serde", "vendored"] }\n',
        'mlua = { version = "0.12.1", features = ["error-send", "lua54", "send", "serde", "vendored"] }\nredis = { version = "0.32.7", default-features = false, features = ["tokio-comp"] }\n',
    )
if 'sqlx = ' not in text:
    text = text.replace(
        'serenity = { version = "0.12.5",',
        'sqlx = { version = "0.8.6", default-features = false, features = ["chrono", "json", "macros", "migrate", "postgres", "runtime-tokio-rustls"] }\nserenity = { version = "0.12.5",',
    )
workspace.write_text(text, encoding="utf-8")

write(
    "crates/config/src/lib.rs",
    r'''
    use std::{
        env, fmt,
        net::SocketAddr,
        path::PathBuf,
        time::Duration,
    };

    use anyhow::{Context, Result, bail};

    #[derive(Clone)]
    pub struct BotConfig {
        discord_token: String,
        pub development_guild_id: Option<u64>,
        pub plugin_directory: PathBuf,
        pub lua_memory_limit_bytes: usize,
        pub lua_instruction_limit: u64,
        pub enable_message_content: bool,
        pub enable_guild_members: bool,
        pub enable_guild_presences: bool,
    }

    #[derive(Clone)]
    pub struct ApiConfig {
        pub bind_address: SocketAddr,
        database_url: String,
        redis_url: String,
        pub database_max_connections: u32,
        pub database_acquire_timeout: Duration,
        pub dependency_timeout: Duration,
        pub run_database_migrations: bool,
    }

    impl BotConfig {
        pub fn from_env() -> Result<Self> {
            dotenvy::dotenv().ok();

            let discord_token = required("DISCORD_TOKEN")?;
            let development_guild_id = optional("DISCORD_DEVELOPMENT_GUILD_ID")
                .map(|value| parse::<u64>("DISCORD_DEVELOPMENT_GUILD_ID", &value))
                .transpose()?;

            Ok(Self {
                discord_token,
                development_guild_id,
                plugin_directory: PathBuf::from(
                    optional("PLUGIN_DIRECTORY").unwrap_or_else(|| "plugins".to_owned()),
                ),
                lua_memory_limit_bytes: parse_or_default("LUA_MEMORY_LIMIT_BYTES", 16 * 1024 * 1024)?,
                lua_instruction_limit: parse_or_default("LUA_INSTRUCTION_LIMIT", 1_000_000)?,
                enable_message_content: parse_bool_or_default("DISCORD_ENABLE_MESSAGE_CONTENT", false)?,
                enable_guild_members: parse_bool_or_default("DISCORD_ENABLE_GUILD_MEMBERS", false)?,
                enable_guild_presences: parse_bool_or_default("DISCORD_ENABLE_GUILD_PRESENCES", false)?,
            })
        }

        pub fn discord_token(&self) -> &str {
            &self.discord_token
        }
    }

    impl fmt::Debug for BotConfig {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("BotConfig")
                .field("discord_token", &"[REDACTED]")
                .field("development_guild_id", &self.development_guild_id)
                .field("plugin_directory", &self.plugin_directory)
                .field("lua_memory_limit_bytes", &self.lua_memory_limit_bytes)
                .field("lua_instruction_limit", &self.lua_instruction_limit)
                .field("enable_message_content", &self.enable_message_content)
                .field("enable_guild_members", &self.enable_guild_members)
                .field("enable_guild_presences", &self.enable_guild_presences)
                .finish()
        }
    }

    impl ApiConfig {
        pub fn from_env() -> Result<Self> {
            dotenvy::dotenv().ok();

            let address = optional("API_BIND_ADDRESS").unwrap_or_else(|| "0.0.0.0:8080".to_owned());
            Ok(Self {
                bind_address: parse("API_BIND_ADDRESS", &address)?,
                database_url: required("DATABASE_URL")?,
                redis_url: required("REDIS_URL")?,
                database_max_connections: parse_or_default("DATABASE_MAX_CONNECTIONS", 10)?,
                database_acquire_timeout: duration_from_milliseconds(
                    "DATABASE_ACQUIRE_TIMEOUT_MS",
                    3_000,
                )?,
                dependency_timeout: duration_from_milliseconds("DEPENDENCY_TIMEOUT_MS", 1_500)?,
                run_database_migrations: parse_bool_or_default("RUN_DATABASE_MIGRATIONS", false)?,
            })
        }

        pub fn database_url(&self) -> &str {
            &self.database_url
        }

        pub fn redis_url(&self) -> &str {
            &self.redis_url
        }
    }

    impl fmt::Debug for ApiConfig {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("ApiConfig")
                .field("bind_address", &self.bind_address)
                .field("database_url", &"[REDACTED]")
                .field("redis_url", &"[REDACTED]")
                .field("database_max_connections", &self.database_max_connections)
                .field("database_acquire_timeout", &self.database_acquire_timeout)
                .field("dependency_timeout", &self.dependency_timeout)
                .field("run_database_migrations", &self.run_database_migrations)
                .finish()
        }
    }

    fn required(name: &str) -> Result<String> {
        optional(name).with_context(|| format!("environment variable {name} is required"))
    }

    fn optional(name: &str) -> Option<String> {
        env::var(name)
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    }

    fn parse<T>(name: &str, value: &str) -> Result<T>
    where
        T: std::str::FromStr,
        T::Err: std::error::Error + Send + Sync + 'static,
    {
        value
            .parse::<T>()
            .with_context(|| format!("environment variable {name} has an invalid value"))
    }

    fn parse_or_default<T>(name: &str, default: T) -> Result<T>
    where
        T: std::str::FromStr,
        T::Err: std::error::Error + Send + Sync + 'static,
    {
        optional(name)
            .map(|value| parse(name, &value))
            .transpose()
            .map(|value| value.unwrap_or(default))
    }

    fn parse_bool_or_default(name: &str, default: bool) -> Result<bool> {
        let Some(value) = optional(name) else {
            return Ok(default);
        };

        match value.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Ok(true),
            "0" | "false" | "no" | "off" => Ok(false),
            _ => bail!("environment variable {name} must be true or false"),
        }
    }

    fn duration_from_milliseconds(name: &str, default: u64) -> Result<Duration> {
        let milliseconds = parse_or_default(name, default)?;
        if milliseconds == 0 {
            bail!("environment variable {name} must be greater than zero");
        }
        Ok(Duration::from_millis(milliseconds))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn accepts_common_boolean_spellings() {
            for value in ["true", "TRUE", "1", "yes", "on"] {
                unsafe { env::set_var("ZUCKERBOT_TEST_BOOL", value) };
                assert!(parse_bool_or_default("ZUCKERBOT_TEST_BOOL", false).unwrap());
            }

            unsafe { env::remove_var("ZUCKERBOT_TEST_BOOL") };
        }

        #[test]
        fn secret_values_are_redacted_from_debug_output() {
            let api = ApiConfig {
                bind_address: "127.0.0.1:8080".parse().unwrap(),
                database_url: "postgres://user:password@example/database".to_owned(),
                redis_url: "redis://:password@example".to_owned(),
                database_max_connections: 5,
                database_acquire_timeout: Duration::from_secs(1),
                dependency_timeout: Duration::from_secs(1),
                run_database_migrations: false,
            };

            let debug = format!("{api:?}");
            assert!(!debug.contains("password"));
            assert!(debug.contains("[REDACTED]"));
        }
    }
    ''',
)

api_manifest = ROOT / "apps/api/Cargo.toml"
text = api_manifest.read_text(encoding="utf-8")
if "zuckerbot-storage" not in text:
    text += 'zuckerbot-storage = { path = "../../crates/storage" }\n'
api_manifest.write_text(text, encoding="utf-8")

write(
    "apps/api/src/main.rs",
    r'''
    use std::{sync::Arc, time::Instant};

    use anyhow::{Context, Result};
    use axum::{
        Json, Router,
        extract::State,
        http::StatusCode,
        response::{Html, IntoResponse, Response},
        routing::get,
    };
    use serde::Serialize;
    use tokio::net::TcpListener;
    use tower_http::trace::TraceLayer;
    use tracing::info;
    use tracing_subscriber::EnvFilter;
    use zuckerbot_config::ApiConfig;
    use zuckerbot_storage::{Infrastructure, InfrastructureOptions, ReadinessState};

    const INDEX_HTML: &str = include_str!("../static/index.html");
    const APP_JS: &str = include_str!("../static/app.js");
    const STYLES_CSS: &str = include_str!("../static/styles.css");

    #[derive(Clone)]
    struct AppState {
        started_at: Instant,
        infrastructure: Infrastructure,
    }

    #[derive(Serialize)]
    struct HealthResponse {
        status: &'static str,
        service: &'static str,
        version: &'static str,
        uptime_seconds: u64,
    }

    #[derive(Serialize)]
    struct MetaResponse {
        product: &'static str,
        phase: &'static str,
        architecture: Vec<&'static str>,
        modules: Vec<ModuleSummary>,
    }

    #[derive(Serialize)]
    struct ModuleSummary {
        id: &'static str,
        name: &'static str,
        status: &'static str,
        description: &'static str,
    }

    #[tokio::main]
    async fn main() -> Result<()> {
        init_tracing();
        let config = ApiConfig::from_env().context("could not load API configuration")?;
        let infrastructure = Infrastructure::new(InfrastructureOptions {
            database_url: config.database_url(),
            redis_url: config.redis_url(),
            database_max_connections: config.database_max_connections,
            database_acquire_timeout: config.database_acquire_timeout,
            dependency_timeout: config.dependency_timeout,
        })
        .context("could not initialize infrastructure clients")?;

        if config.run_database_migrations {
            info!("Running embedded database migrations");
            infrastructure
                .migrate()
                .await
                .context("database migrations failed")?;
        }

        let state = Arc::new(AppState {
            started_at: Instant::now(),
            infrastructure,
        });

        let app = Router::new()
            .route("/", get(index))
            .route("/assets/app.js", get(javascript))
            .route("/assets/styles.css", get(stylesheet))
            .route("/health", get(health))
            .route("/ready", get(readiness))
            .route("/api/v1/meta", get(meta))
            .with_state(state)
            .layer(TraceLayer::new_for_http());

        let listener = TcpListener::bind(config.bind_address)
            .await
            .with_context(|| format!("could not bind API to {}", config.bind_address))?;

        info!(address = %config.bind_address, "ZuckerBot control API started");
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .context("control API stopped unexpectedly")
    }

    async fn index() -> Html<&'static str> {
        Html(INDEX_HTML)
    }

    async fn javascript() -> impl IntoResponse {
        (
            [("content-type", "application/javascript; charset=utf-8")],
            APP_JS,
        )
    }

    async fn stylesheet() -> impl IntoResponse {
        ([("content-type", "text/css; charset=utf-8")], STYLES_CSS)
    }

    async fn health(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
        Json(HealthResponse {
            status: "ok",
            service: "zuckerbot-api",
            version: env!("CARGO_PKG_VERSION"),
            uptime_seconds: state.started_at.elapsed().as_secs(),
        })
    }

    async fn readiness(State(state): State<Arc<AppState>>) -> Response {
        let report = state.infrastructure.readiness().await;
        let status_code = match report.status {
            ReadinessState::Ready => StatusCode::OK,
            ReadinessState::NotReady => StatusCode::SERVICE_UNAVAILABLE,
        };

        (status_code, Json(report)).into_response()
    }

    async fn meta() -> Json<MetaResponse> {
        Json(MetaResponse {
            product: "ZuckerBot",
            phase: "control-plane-storage",
            architecture: vec![
                "Rust control plane and Discord gateway",
                "Sandboxed Lua 5.4 extension runtime",
                "PostgreSQL durable storage with optimistic configuration versions",
                "Redis cache, jobs and distributed coordination",
                "Web dashboard with Discord OAuth2 and guild RBAC",
            ],
            modules: vec![
                ModuleSummary {
                    id: "storage",
                    name: "Persistent Storage",
                    status: "available",
                    description: "PostgreSQL migrations, repositories and version-safe writes.",
                },
                ModuleSummary {
                    id: "lua",
                    name: "Lua Runtime",
                    status: "available",
                    description: "Command discovery, validation and bounded execution.",
                },
                ModuleSummary {
                    id: "gateway",
                    name: "Discord Gateway",
                    status: "available",
                    description: "Slash-command synchronization and interaction dispatch.",
                },
                ModuleSummary {
                    id: "voice",
                    name: "Voice Foundation",
                    status: "foundation",
                    description: "Songbird voice manager registered; queue and providers follow.",
                },
                ModuleSummary {
                    id: "dashboard",
                    name: "Control Dashboard",
                    status: "foundation",
                    description: "Liveness/readiness API; Discord OAuth2 and RBAC follow.",
                },
            ],
        })
    }

    async fn shutdown_signal() {
        let ctrl_c = async {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to install Ctrl+C handler");
        };

        #[cfg(unix)]
        let terminate = async {
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("failed to install SIGTERM handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            () = ctrl_c => {},
            () = terminate => {},
        }
    }

    fn init_tracing() {
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(true)
            .compact()
            .init();
    }
    ''',
)

env_file = ROOT / ".env.example"
text = env_file.read_text(encoding="utf-8")
if "DATABASE_MAX_CONNECTIONS" not in text:
    text = text.replace(
        "# Required by apps/api\nAPI_BIND_ADDRESS=0.0.0.0:8080\n\n# Infrastructure reserved for the next milestones\nDATABASE_URL=postgres://zuckerbot:zuckerbot@localhost:5432/zuckerbot\nREDIS_URL=redis://localhost:6379\nRUST_LOG=info,zuckerbot_bot=debug,zuckerbot_api=debug\n",
        "# Required by apps/api\nAPI_BIND_ADDRESS=0.0.0.0:8080\nDATABASE_URL=postgres://zuckerbot:zuckerbot@localhost:5432/zuckerbot\nREDIS_URL=redis://localhost:6379\nDATABASE_MAX_CONNECTIONS=10\nDATABASE_ACQUIRE_TIMEOUT_MS=3000\nDEPENDENCY_TIMEOUT_MS=1500\nRUN_DATABASE_MIGRATIONS=false\n\nRUST_LOG=info,zuckerbot_bot=debug,zuckerbot_api=debug,zuckerbot_storage=debug\n",
    )
env_file.write_text(text, encoding="utf-8")

compose = ROOT / "docker-compose.yml"
text = compose.read_text(encoding="utf-8")
if "RUN_DATABASE_MIGRATIONS" not in text:
    text = text.replace(
        "    environment:\n      API_BIND_ADDRESS: 0.0.0.0:8080\n",
        "    environment:\n      API_BIND_ADDRESS: 0.0.0.0:8080\n      RUN_DATABASE_MIGRATIONS: \"true\"\n",
        1,
    )
compose.write_text(text, encoding="utf-8")

write(
    ".github/workflows/ci.yml",
    r'''
    name: CI

    on:
      pull_request:
      push:
        branches: [main]

    permissions:
      contents: read

    concurrency:
      group: ci-${{ github.workflow }}-${{ github.ref }}
      cancel-in-progress: true

    jobs:
      quality:
        name: Rust, Lua, storage and web shell
        runs-on: ubuntu-24.04
        timeout-minutes: 35

        services:
          postgres:
            image: postgres:17-alpine
            env:
              POSTGRES_DB: zuckerbot
              POSTGRES_USER: zuckerbot
              POSTGRES_PASSWORD: zuckerbot
            ports:
              - 5432:5432
            options: >-
              --health-cmd "pg_isready -U zuckerbot -d zuckerbot"
              --health-interval 5s
              --health-timeout 5s
              --health-retries 10

          redis:
            image: redis:8-alpine
            ports:
              - 6379:6379
            options: >-
              --health-cmd "redis-cli ping"
              --health-interval 5s
              --health-timeout 5s
              --health-retries 10

        env:
          DATABASE_URL: postgres://zuckerbot:zuckerbot@127.0.0.1:5432/zuckerbot
          REDIS_URL: redis://127.0.0.1:6379
          DATABASE_MAX_CONNECTIONS: 5
          DATABASE_ACQUIRE_TIMEOUT_MS: 3000
          DEPENDENCY_TIMEOUT_MS: 2000
          RUN_DATABASE_MIGRATIONS: "true"

        steps:
          - name: Checkout exact revision
            uses: actions/checkout@v4
            with:
              ref: ${{ github.event.pull_request.head.sha || github.sha }}

          - name: Verify checked-out revision
            env:
              EXPECTED_SHA: ${{ github.event.pull_request.head.sha || github.sha }}
            run: test "$(git rev-parse HEAD)" = "$EXPECTED_SHA"

          - name: Install Rust 1.88
            uses: dtolnay/rust-toolchain@master
            with:
              toolchain: 1.88.0
              components: clippy,rustfmt

          - name: Install native dependencies
            run: |
              sudo apt-get update
              sudo apt-get install --yes --no-install-recommends cmake libopus-dev pkg-config

          - name: Generate dependency lockfile
            run: cargo generate-lockfile

          - name: Check formatting
            run: cargo fmt --all -- --check

          - name: Run Clippy
            run: cargo clippy --workspace --all-targets -- -D warnings

          - name: Run unit and contract tests
            run: cargo test --workspace

          - name: Run PostgreSQL and Redis integration tests
            run: cargo test -p zuckerbot-storage --test infrastructure -- --ignored --test-threads=1

          - name: Build workspace
            run: cargo build --workspace

          - name: Validate dashboard JavaScript
            run: node --check apps/api/static/app.js

          - name: Validate Compose model
            run: |
              cp .env.example .env
              docker compose config --quiet

          - name: Smoke-test liveness and readiness APIs
            env:
              API_BIND_ADDRESS: 127.0.0.1:18080
              RUST_LOG: warn
            run: |
              target/debug/zuckerbot-api > /tmp/zuckerbot-api.log 2>&1 &
              api_pid=$!
              trap 'kill "$api_pid" 2>/dev/null || true' EXIT
              for attempt in {1..30}; do
                if curl --fail --silent http://127.0.0.1:18080/health | grep --quiet '"status":"ok"' \
                  && curl --fail --silent http://127.0.0.1:18080/ready | grep --quiet '"status":"ready"'; then
                  exit 0
                fi
                sleep 0.5
              done
              cat /tmp/zuckerbot-api.log
              exit 1

          - name: Preserve generated lockfile
            uses: actions/upload-artifact@v4
            with:
              name: cargo-lock
              path: Cargo.lock
              if-no-files-found: error
    ''',
)

readme = ROOT / "README.md"
text = readme.read_text(encoding="utf-8")
if "docs/STORAGE_AND_READINESS.md" not in text:
    text = text.replace(
        "- [Środowisko deweloperskie](docs/DEVELOPMENT.md)\n",
        "- [Środowisko deweloperskie](docs/DEVELOPMENT.md)\n- [PostgreSQL, Redis i readiness](docs/STORAGE_AND_READINESS.md)\n",
    )
readme.write_text(text, encoding="utf-8")

for probe in ["TEST_LOCAL_UPLOAD", "CONNECTOR_PROBE"]:
    candidate = ROOT / probe
    if candidate.exists():
        candidate.unlink()

subprocess.run(["cargo", "fmt", "--all"], cwd=ROOT, check=True)
subprocess.run(["cargo", "check", "-p", "zuckerbot-storage", "-p", "zuckerbot-api"], cwd=ROOT, check=True)
subprocess.run(["cargo", "test", "-p", "zuckerbot-storage", "--lib"], cwd=ROOT, check=True)
subprocess.run(["cargo", "test", "-p", "zuckerbot-config", "--lib"], cwd=ROOT, check=True)
subprocess.run(["node", "--check", "apps/api/static/app.js"], cwd=ROOT, check=True)

lockfile = ROOT / "Cargo.lock"
if lockfile.exists():
    lockfile.unlink()

for transient in [
    ".github/workflows/control-plane-bootstrap.yml",
    "tools/bootstrap-control-plane.py",
]:
    candidate = ROOT / transient
    if candidate.exists():
        candidate.unlink()
