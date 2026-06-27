// SPDX-License-Identifier: AGPL-3.0-only

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use libsql::Builder;
use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use time::{Date, OffsetDateTime, Time};

use crate::config::Config;
use crate::db::DbPool;

const BACKUP_PREFIX: &str = "ragdoll-";
const BACKUP_SUFFIX: &str = ".db";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BackupTrigger {
    Manual,
    Daily,
}

impl BackupTrigger {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Daily => "daily",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "manual" => Some(Self::Manual),
            "daily" => Some(Self::Daily),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupInfo {
    pub file_name: String,
    pub trigger: BackupTrigger,
    pub created_at: String,
    pub size_bytes: u64,
}

pub fn format_timestamp(now: OffsetDateTime) -> String {
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}{:03}Z",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
        now.millisecond()
    )
}

pub fn backup_file_name(trigger: BackupTrigger, now: OffsetDateTime) -> String {
    format!(
        "{}{}-{}.db",
        BACKUP_PREFIX,
        format_timestamp(now),
        trigger.as_str()
    )
}

fn parse_backup_file_name(file_name: &str) -> Option<(OffsetDateTime, BackupTrigger)> {
    let stem = file_name
        .strip_prefix(BACKUP_PREFIX)?
        .strip_suffix(BACKUP_SUFFIX)?;
    let (ts, trigger_str) = stem.rsplit_once('-')?;
    let trigger = BackupTrigger::parse(trigger_str)?;

    if !(ts.len() == 16 || ts.len() == 19) || !ts.ends_with('Z') {
        return None;
    }

    let year: i32 = ts[0..4].parse().ok()?;
    let month: u8 = ts[4..6].parse().ok()?;
    let day: u8 = ts[6..8].parse().ok()?;
    let hour: u8 = ts[9..11].parse().ok()?;
    let minute: u8 = ts[11..13].parse().ok()?;
    let second: u8 = ts[13..15].parse().ok()?;
    let millisecond: u16 = if ts.len() == 19 {
        ts[15..18].parse().ok()?
    } else {
        0
    };

    let date = Date::from_calendar_date(year, time::Month::try_from(month).ok()?, day).ok()?;
    let time = Time::from_hms_milli(hour, minute, second, millisecond).ok()?;
    let created_at = OffsetDateTime::new_utc(date, time);

    Some((created_at, trigger))
}

pub fn validate_backup_file_name(file_name: &str) -> Result<(OffsetDateTime, BackupTrigger)> {
    if file_name.contains('/') || file_name.contains('\\') {
        anyhow::bail!("invalid backup file name");
    }
    parse_backup_file_name(file_name)
        .ok_or_else(|| anyhow::anyhow!("invalid backup file name format"))
}

fn sort_backups_newest_first(backups: &mut [BackupInfo]) {
    backups.sort_by(|a, b| b.file_name.cmp(&a.file_name));
}

fn sort_backups_by_timestamp_newest_first(backups: &mut [BackupInfo]) {
    backups.sort_by(|a, b| {
        let a_ts = parse_backup_file_name(&a.file_name).map(|(ts, _)| ts);
        let b_ts = parse_backup_file_name(&b.file_name).map(|(ts, _)| ts);
        b_ts.cmp(&a_ts)
    });
}

fn temp_file_name(final_name: &str) -> String {
    format!(".{final_name}.tmp")
}

fn escape_sql_string(path: &Path) -> String {
    path.to_string_lossy().replace('\'', "''")
}

async fn verify_integrity(path: &Path) -> Result<()> {
    let db = Builder::new_local(path)
        .build()
        .await
        .with_context(|| format!("failed to open backup at {}", path.display()))?;
    let conn = db.connect().map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut rows = conn
        .query("PRAGMA integrity_check", ())
        .await
        .context("integrity_check query failed")?;
    let row = rows
        .next()
        .await
        .context("integrity_check returned no rows")?
        .context("integrity_check returned no rows")?;
    let result: String = row.get(0).context("integrity_check result missing")?;
    if result != "ok" {
        anyhow::bail!("integrity_check failed: {result}");
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct RestoreInfo {
    pub restored_from: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_backup: Option<String>,
    pub restored_at: String,
}

pub fn resolve_backup_path(config: &Config, file_name: &str) -> Result<PathBuf> {
    validate_backup_file_name(file_name)?;
    let path = config.backup_dir.join(file_name);
    if !path.is_file() {
        anyhow::bail!("backup not found");
    }
    Ok(path)
}

fn remove_wal_sidecars(db_path: &Path) {
    for suffix in ["-wal", "-shm"] {
        let sidecar = PathBuf::from(format!("{}{suffix}", db_path.to_string_lossy()));
        if sidecar.exists() {
            let _ = std::fs::remove_file(sidecar);
        }
    }
}

pub async fn restore_backup(
    pool: &DbPool,
    config: &Config,
    file_name: &str,
    create_safety_backup: bool,
) -> Result<RestoreInfo> {
    let backup_path = resolve_backup_path(config, file_name)?;
    verify_integrity(&backup_path).await?;

    let preserved = config
        .backup_dir
        .join(format!(".restore-source-{file_name}.tmp"));
    if preserved.exists() {
        std::fs::remove_file(&preserved)?;
    }
    std::fs::copy(&backup_path, &preserved)
        .with_context(|| format!("failed to preserve {} for restore", backup_path.display()))?;

    let safety_backup = if create_safety_backup {
        Some(
            create_backup(pool, config, BackupTrigger::Manual)
                .await?
                .file_name,
        )
    } else {
        None
    };

    let conn = pool.connect_one().await?;
    run_pragma(&conn, "PRAGMA wal_checkpoint(TRUNCATE)").await?;
    drop(conn);

    remove_wal_sidecars(&config.db_path);

    let tmp_path = config.db_path.with_file_name(format!(
        ".{}.restore.tmp",
        config
            .db_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("ragdoll.db")
    ));
    if tmp_path.exists() {
        std::fs::remove_file(&tmp_path).with_context(|| {
            format!("failed to remove stale restore temp {}", tmp_path.display())
        })?;
    }

    std::fs::copy(&preserved, &tmp_path).with_context(|| {
        format!(
            "failed to copy {} to {}",
            preserved.display(),
            tmp_path.display()
        )
    })?;
    let _ = std::fs::remove_file(&preserved);
    verify_integrity(&tmp_path).await?;
    std::fs::rename(&tmp_path, &config.db_path).with_context(|| {
        format!(
            "failed to replace {} with restored database",
            config.db_path.display()
        )
    })?;

    pool.reload(config)
        .await
        .context("failed to reopen database after restore")?;
    verify_integrity(&config.db_path).await?;

    let restored_at = OffsetDateTime::now_utc();
    Ok(RestoreInfo {
        restored_from: file_name.to_string(),
        safety_backup,
        restored_at: restored_at
            .format(&Rfc3339)
            .unwrap_or_else(|_| format_timestamp(restored_at)),
    })
}

async fn run_pragma(conn: &libsql::Connection, sql: &str) -> Result<()> {
    let mut rows = conn.query(sql, ()).await?;
    while rows.next().await?.is_some() {}
    Ok(())
}

pub async fn create_backup(
    pool: &DbPool,
    config: &Config,
    trigger: BackupTrigger,
) -> Result<BackupInfo> {
    std::fs::create_dir_all(&config.backup_dir).with_context(|| {
        format!(
            "failed to create backup directory {}",
            config.backup_dir.display()
        )
    })?;

    let now = OffsetDateTime::now_utc();
    let file_name = backup_file_name(trigger, now);
    let final_path = config.backup_dir.join(&file_name);
    let tmp_path = config.backup_dir.join(temp_file_name(&file_name));

    if tmp_path.exists() {
        std::fs::remove_file(&tmp_path).with_context(|| {
            format!("failed to remove stale temp backup {}", tmp_path.display())
        })?;
    }

    let conn = pool.connect_one().await?;
    let escaped = escape_sql_string(&tmp_path);
    let sql = format!("VACUUM INTO '{escaped}'");
    conn.execute(&sql, ())
        .await
        .with_context(|| format!("VACUUM INTO failed for {}", tmp_path.display()))?;

    if let Err(err) = verify_integrity(&tmp_path).await {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(err.context("backup failed integrity check"));
    }

    std::fs::rename(&tmp_path, &final_path).with_context(|| {
        format!(
            "failed to rename {} to {}",
            tmp_path.display(),
            final_path.display()
        )
    })?;

    prune(config, trigger)?;

    let size_bytes = std::fs::metadata(&final_path)
        .with_context(|| format!("failed to stat {}", final_path.display()))?
        .len();

    Ok(BackupInfo {
        file_name,
        trigger,
        created_at: now
            .format(&Rfc3339)
            .unwrap_or_else(|_| format_timestamp(now)),
        size_bytes,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupRetention {
    pub keep_daily: u32,
    pub keep_manual: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupsListResponse {
    pub backups: Vec<BackupInfo>,
    pub retention: BackupRetention,
}

pub fn backups_list_response(config: &Config) -> Result<BackupsListResponse> {
    Ok(BackupsListResponse {
        backups: list_backups(config)?,
        retention: BackupRetention {
            keep_daily: config.backup_keep_daily,
            keep_manual: config.backup_keep_manual,
        },
    })
}

pub fn list_backups(config: &Config) -> Result<Vec<BackupInfo>> {
    if !config.backup_dir.exists() {
        return Ok(Vec::new());
    }

    let mut backups = Vec::new();
    for entry in std::fs::read_dir(&config.backup_dir)
        .with_context(|| format!("failed to read {}", config.backup_dir.display()))?
    {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if file_name.starts_with('.') || !file_name.starts_with(BACKUP_PREFIX) {
            continue;
        }
        let Some((created_at, trigger)) = parse_backup_file_name(&file_name) else {
            continue;
        };
        let size_bytes = entry.metadata()?.len();
        backups.push(BackupInfo {
            file_name,
            trigger,
            created_at: created_at
                .format(&Rfc3339)
                .unwrap_or_else(|_| format_timestamp(created_at)),
            size_bytes,
        });
    }

    sort_backups_newest_first(&mut backups);
    Ok(backups)
}

pub fn delete_backup(config: &Config, file_name: &str) -> Result<()> {
    let path = resolve_backup_path(config, file_name)?;
    std::fs::remove_file(&path)
        .with_context(|| format!("failed to delete backup {}", path.display()))?;
    Ok(())
}

pub async fn import_backup_bytes(
    config: &Config,
    file_name: &str,
    data: &[u8],
) -> Result<BackupInfo> {
    let (created_at, trigger) = validate_backup_file_name(file_name)?;

    std::fs::create_dir_all(&config.backup_dir).with_context(|| {
        format!(
            "failed to create backup directory {}",
            config.backup_dir.display()
        )
    })?;

    let final_path = config.backup_dir.join(file_name);
    if final_path.exists() {
        anyhow::bail!("backup already exists");
    }

    let tmp_path = config.backup_dir.join(temp_file_name(file_name));

    if tmp_path.exists() {
        std::fs::remove_file(&tmp_path)?;
    }

    std::fs::write(&tmp_path, data)
        .with_context(|| format!("failed to write uploaded backup to {}", tmp_path.display()))?;

    if let Err(err) = verify_integrity(&tmp_path).await {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(err.context("uploaded file is not a valid database backup"));
    }

    std::fs::rename(&tmp_path, &final_path).with_context(|| {
        format!(
            "failed to store uploaded backup at {}",
            final_path.display()
        )
    })?;

    prune(config, trigger)?;

    let size_bytes = std::fs::metadata(&final_path)?.len();
    Ok(BackupInfo {
        file_name: file_name.to_string(),
        trigger,
        created_at: created_at
            .format(&Rfc3339)
            .unwrap_or_else(|_| format_timestamp(created_at)),
        size_bytes,
    })
}

pub fn has_daily_for_today(config: &Config, now: OffsetDateTime) -> Result<bool> {
    let today = now.date();
    Ok(list_backups(config)?.into_iter().any(|b| {
        b.trigger == BackupTrigger::Daily && parse_backup_date(&b.file_name) == Some(today)
    }))
}

fn parse_backup_date(file_name: &str) -> Option<Date> {
    let (created_at, _) = parse_backup_file_name(file_name)?;
    Some(created_at.date())
}

pub fn prune(config: &Config, trigger: BackupTrigger) -> Result<()> {
    let keep = match trigger {
        BackupTrigger::Daily => config.backup_keep_daily,
        BackupTrigger::Manual => config.backup_keep_manual,
    };

    let mut matching: Vec<BackupInfo> = list_backups(config)?
        .into_iter()
        .filter(|b| b.trigger == trigger)
        .collect();

    sort_backups_by_timestamp_newest_first(&mut matching);

    for backup in matching.into_iter().skip(keep as usize) {
        let path = config.backup_dir.join(&backup.file_name);
        if path.exists() {
            std::fs::remove_file(&path)
                .with_context(|| format!("failed to remove old backup {}", path.display()))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::db::{migrations, DbPool};
    use tempfile::TempDir;

    #[test]
    fn backup_file_name_roundtrip() {
        let now = OffsetDateTime::from_unix_timestamp(1_718_000_000).unwrap();
        let file_name = backup_file_name(BackupTrigger::Daily, now);
        assert_eq!(file_name, "ragdoll-20240610T061320000Z-daily.db");
        let (parsed, trigger) = parse_backup_file_name(&file_name).unwrap();
        assert_eq!(trigger, BackupTrigger::Daily);
        assert_eq!(parsed.unix_timestamp(), now.unix_timestamp());
    }

    #[test]
    fn has_daily_for_today_detects_today_and_yesterday() {
        let dir = TempDir::new().unwrap();
        let config = Config::for_test(dir.path().to_path_buf(), "secret");
        std::fs::create_dir_all(&config.backup_dir).unwrap();

        let today = OffsetDateTime::now_utc();
        let yesterday = today - time::Duration::days(1);

        std::fs::write(
            config
                .backup_dir
                .join(backup_file_name(BackupTrigger::Daily, yesterday)),
            b"x",
        )
        .unwrap();
        assert!(!has_daily_for_today(&config, today).unwrap());

        std::fs::write(
            config
                .backup_dir
                .join(backup_file_name(BackupTrigger::Daily, today)),
            b"x",
        )
        .unwrap();
        assert!(has_daily_for_today(&config, today).unwrap());
    }

    #[test]
    fn prune_keeps_limits_per_trigger() {
        let dir = TempDir::new().unwrap();
        let mut config = Config::for_test(dir.path().to_path_buf(), "secret");
        config.backup_keep_daily = 2;
        config.backup_keep_manual = 1;
        std::fs::create_dir_all(&config.backup_dir).unwrap();

        let base = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        for i in 0..4 {
            let ts = base + time::Duration::seconds(i);
            std::fs::write(
                config
                    .backup_dir
                    .join(backup_file_name(BackupTrigger::Daily, ts)),
                b"x",
            )
            .unwrap();
        }
        for i in 0..3 {
            let ts = base + time::Duration::seconds(100 + i);
            std::fs::write(
                config
                    .backup_dir
                    .join(backup_file_name(BackupTrigger::Manual, ts)),
                b"x",
            )
            .unwrap();
        }

        prune(&config, BackupTrigger::Daily).unwrap();
        prune(&config, BackupTrigger::Manual).unwrap();

        let backups = list_backups(&config).unwrap();
        assert_eq!(
            backups
                .iter()
                .filter(|b| b.trigger == BackupTrigger::Daily)
                .count(),
            2
        );
        assert_eq!(
            backups
                .iter()
                .filter(|b| b.trigger == BackupTrigger::Manual)
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn create_backup_produces_valid_file() {
        let dir = TempDir::new().unwrap();
        let config = Config::for_test(dir.path().to_path_buf(), "secret");
        config.ensure_directories().unwrap();

        let pool = DbPool::connect(&config).await.unwrap();
        migrations::run_migrations(&pool, &config.migrations_dir)
            .await
            .unwrap();

        let info = create_backup(&pool, &config, BackupTrigger::Manual)
            .await
            .unwrap();
        assert_eq!(info.trigger, BackupTrigger::Manual);
        assert!(config.backup_dir.join(&info.file_name).exists());
        verify_integrity(&config.backup_dir.join(&info.file_name))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn restore_backup_replaces_live_database() {
        let dir = TempDir::new().unwrap();
        let config = Config::for_test(dir.path().to_path_buf(), "secret");
        config.ensure_directories().unwrap();

        let pool = DbPool::connect(&config).await.unwrap();
        migrations::run_migrations(&pool, &config.migrations_dir)
            .await
            .unwrap();

        let conn = pool.connect_one().await.unwrap();
        conn.execute(
            "INSERT INTO releases (id, tag, message, created_at) VALUES ('r2', 'marker', 'before restore', datetime('now'))",
            (),
        )
        .await
        .unwrap();
        drop(conn);

        let snapshot = create_backup(&pool, &config, BackupTrigger::Manual)
            .await
            .unwrap();

        let conn = pool.connect_one().await.unwrap();
        conn.execute("DELETE FROM releases WHERE tag = 'marker'", ())
            .await
            .unwrap();
        drop(conn);

        let mut rows = pool
            .connect_one()
            .await
            .unwrap()
            .query("SELECT COUNT(*) FROM releases WHERE tag = 'marker'", ())
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let count: i64 = row.get(0).unwrap();
        assert_eq!(count, 0);

        let restore = restore_backup(&pool, &config, &snapshot.file_name, false)
            .await
            .unwrap();
        assert_eq!(restore.restored_from, snapshot.file_name);
        assert!(restore.safety_backup.is_none());

        let mut rows = pool
            .connect_one()
            .await
            .unwrap()
            .query("SELECT COUNT(*) FROM releases WHERE tag = 'marker'", ())
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let count: i64 = row.get(0).unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn restore_backup_can_create_optional_safety_backup() {
        let dir = TempDir::new().unwrap();
        let config = Config::for_test(dir.path().to_path_buf(), "secret");
        config.ensure_directories().unwrap();

        let pool = DbPool::connect(&config).await.unwrap();
        migrations::run_migrations(&pool, &config.migrations_dir)
            .await
            .unwrap();

        let snapshot = create_backup(&pool, &config, BackupTrigger::Manual)
            .await
            .unwrap();

        let restore = restore_backup(&pool, &config, &snapshot.file_name, true)
            .await
            .unwrap();
        assert!(restore.safety_backup.is_some());
    }

    #[test]
    fn resolve_backup_path_rejects_invalid_names() {
        let dir = TempDir::new().unwrap();
        let config = Config::for_test(dir.path().to_path_buf(), "secret");
        assert!(resolve_backup_path(&config, "../etc/passwd").is_err());
        assert!(resolve_backup_path(&config, "not-a-backup.db").is_err());
    }

    #[tokio::test]
    async fn delete_backup_removes_file() {
        let dir = TempDir::new().unwrap();
        let config = Config::for_test(dir.path().to_path_buf(), "secret");
        config.ensure_directories().unwrap();

        let pool = DbPool::connect(&config).await.unwrap();
        migrations::run_migrations(&pool, &config.migrations_dir)
            .await
            .unwrap();

        let info = create_backup(&pool, &config, BackupTrigger::Manual)
            .await
            .unwrap();
        let path = config.backup_dir.join(&info.file_name);
        assert!(path.exists());

        delete_backup(&config, &info.file_name).unwrap();
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn import_backup_bytes_stores_valid_upload() {
        let dir = TempDir::new().unwrap();
        let config = Config::for_test(dir.path().to_path_buf(), "secret");
        config.ensure_directories().unwrap();

        let pool = DbPool::connect(&config).await.unwrap();
        migrations::run_migrations(&pool, &config.migrations_dir)
            .await
            .unwrap();

        let source = create_backup(&pool, &config, BackupTrigger::Manual)
            .await
            .unwrap();
        let bytes = std::fs::read(config.backup_dir.join(&source.file_name)).unwrap();

        delete_backup(&config, &source.file_name).unwrap();

        let imported = import_backup_bytes(&config, &source.file_name, &bytes)
            .await
            .unwrap();
        assert_eq!(imported.file_name, source.file_name);
        assert!(config.backup_dir.join(&imported.file_name).exists());
        verify_integrity(&config.backup_dir.join(&imported.file_name))
            .await
            .unwrap();
    }

    #[test]
    fn validate_backup_file_name_rejects_invalid_names() {
        assert!(validate_backup_file_name("backup.db").is_err());
        assert!(validate_backup_file_name("ragdoll-not-a-timestamp-manual.db").is_err());
        assert!(validate_backup_file_name("../ragdoll-20240610T061320000Z-manual.db").is_err());
    }

    #[tokio::test]
    async fn import_backup_bytes_rejects_invalid_file_name() {
        let dir = TempDir::new().unwrap();
        let config = Config::for_test(dir.path().to_path_buf(), "secret");
        config.ensure_directories().unwrap();

        let err = import_backup_bytes(&config, "invalid-backup.db", b"sqlite")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("invalid backup file name"));
    }

    #[test]
    fn prune_uses_timestamp_from_file_name() {
        let dir = TempDir::new().unwrap();
        let mut config = Config::for_test(dir.path().to_path_buf(), "secret");
        config.backup_keep_manual = 1;
        std::fs::create_dir_all(&config.backup_dir).unwrap();

        let older = backup_file_name(
            BackupTrigger::Manual,
            OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
        );
        let newer = backup_file_name(
            BackupTrigger::Manual,
            OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap(),
        );
        std::fs::write(config.backup_dir.join(&older), b"x").unwrap();
        std::fs::write(config.backup_dir.join(&newer), b"x").unwrap();

        prune(&config, BackupTrigger::Manual).unwrap();

        assert!(!config.backup_dir.join(&older).exists());
        assert!(config.backup_dir.join(&newer).exists());
    }

    #[test]
    fn list_backups_sorts_newest_file_name_first() {
        let dir = TempDir::new().unwrap();
        let config = Config::for_test(dir.path().to_path_buf(), "secret");
        std::fs::create_dir_all(&config.backup_dir).unwrap();

        let older = backup_file_name(
            BackupTrigger::Daily,
            OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
        );
        let newer = backup_file_name(
            BackupTrigger::Daily,
            OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap(),
        );
        std::fs::write(config.backup_dir.join(&older), b"x").unwrap();
        std::fs::write(config.backup_dir.join(&newer), b"x").unwrap();

        let backups = list_backups(&config).unwrap();
        assert_eq!(backups.len(), 2);
        assert_eq!(backups[0].file_name, newer);
        assert_eq!(backups[1].file_name, older);
    }
}
