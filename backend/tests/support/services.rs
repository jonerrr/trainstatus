//! Disposable cache ownership shared by public integration and private engine tests.

use std::process::Command;
use std::time::Duration;

use anyhow::{Context, ensure};
use bb8_redis::RedisConnectionManager;

pub type RedisPool = bb8::Pool<RedisConnectionManager>;

pub struct TestRedis {
    name: String,
    pool: RedisPool,
}

impl TestRedis {
    pub async fn start() -> anyhow::Result<Self> {
        let name = format!("trainstatus-test-cache-{}", uuid::Uuid::now_v7());
        let image = std::env::var("TEST_VALKEY_IMAGE")
            .context("run integration tests through the backend mise tasks")?;
        let run_id = std::env::var("TEST_SERVICE_RUN_ID")
            .context("run integration tests through the backend mise tasks")?;
        let output = tokio::process::Command::new("podman")
            .kill_on_drop(true)
            .args([
                "run",
                "--detach",
                "--name",
                &name,
                "--label",
                &format!("trainstatus-test-run={run_id}"),
                "--publish",
                "127.0.0.1::6379",
                &image,
            ])
            .output()
            .await
            .context("start test Valkey container")?;
        // Install ownership before checking startup/port/readiness errors.
        let manager = RedisConnectionManager::new("redis://127.0.0.1:1")?;
        let mut service = Self {
            name,
            pool: bb8::Pool::builder().build_unchecked(manager),
        };
        ensure!(
            output.status.success(),
            "Valkey startup: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = tokio::process::Command::new("podman")
            .args(["port", &service.name, "6379"])
            .output()
            .await
            .context("read test Valkey port")?;
        ensure!(
            output.status.success(),
            "Valkey port: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let address = String::from_utf8(output.stdout)?;
        let manager = RedisConnectionManager::new(format!("redis://{}", address.trim()))?;
        service.pool = bb8::Pool::builder()
            .connection_timeout(Duration::from_secs(5))
            .build(manager)
            .await
            .context("connect to test Valkey")?;
        let mut connection = service.pool.get().await?;
        let pong: String = redis::cmd("PING").query_async(&mut *connection).await?;
        ensure!(pong == "PONG", "test Valkey did not respond to PING");
        drop(connection);
        Ok(service)
    }

    pub fn pool(&self) -> RedisPool {
        self.pool.clone()
    }

    #[allow(dead_code)] // Private engine tests include the guard but do not clear caches.
    pub async fn flush(&self) -> anyhow::Result<()> {
        let mut connection = self.pool.get().await?;
        redis::cmd("FLUSHDB")
            .query_async::<()>(&mut *connection)
            .await?;
        Ok(())
    }
}

impl Drop for TestRedis {
    fn drop(&mut self) {
        // Drop cannot await; removing only this guard's container also handles panics.
        match Command::new("podman")
            .args(["rm", "--force", &self.name])
            .output()
        {
            Ok(output) if output.status.success() => {}
            Ok(output) => eprintln!(
                "test cache cleanup {}: {}",
                self.name,
                String::from_utf8_lossy(&output.stderr)
            ),
            Err(error) => eprintln!("test cache cleanup {}: {error}", self.name),
        }
    }
}
