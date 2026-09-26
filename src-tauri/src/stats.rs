//! 统计库：`%APPDATA%\AutoMoyu\stats.db`（SQLite）。
//!
//! - sessions：一场一行（开始/结束、模式、条数、有效时长、结束原因、来源 app/legacy）。
//! - events：cast / catch / empty / castFailed / pause / resume / stop，data 为 JSON。

use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

pub struct Stats {
    db: Connection,
}

#[derive(Serialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub catches: i64,
    pub active_ms: i64,
    pub sessions: i64,
    pub since: Option<String>,
    pub today_catches: i64,
    pub avg_bite_ms: Option<f64>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Day {
    pub date: String,
    pub catches: i64,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SessionRow {
    pub id: i64,
    pub started_at: String,
    pub active_ms: i64,
    pub catches: i64,
    pub mode: String,
    pub source: String,
    pub stop_reason: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Bucket {
    pub label: String,
    pub count: i64,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StatsView {
    pub summary: Summary,
    pub days: Vec<Day>,
    pub bite_buckets: Vec<Bucket>,
    pub recent: Vec<SessionRow>,
}

fn now_str() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S%.3f%:z").to_string()
}

impl Stats {
    pub fn open(path: &Path) -> Result<Stats> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        let db = Connection::open(path).with_context(|| format!("打开 {}", path.display()))?;
        Self::init(db)
    }

    #[cfg(test)]
    pub fn memory() -> Result<Stats> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(db: Connection) -> Result<Stats> {
        db.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS sessions (
               id INTEGER PRIMARY KEY,
               started_at TEXT NOT NULL,
               ended_at TEXT,
               mode TEXT NOT NULL,
               catches INTEGER NOT NULL DEFAULT 0,
               active_ms INTEGER NOT NULL DEFAULT 0,
               stop_reason TEXT,
               source TEXT NOT NULL DEFAULT 'app'
             );
             CREATE UNIQUE INDEX IF NOT EXISTS sessions_legacy_uniq
               ON sessions(started_at, active_ms) WHERE source = 'legacy';
             CREATE TABLE IF NOT EXISTS events (
               id INTEGER PRIMARY KEY,
               session_id INTEGER NOT NULL REFERENCES sessions(id),
               at TEXT NOT NULL,
               kind TEXT NOT NULL,
               data TEXT
             );
             CREATE INDEX IF NOT EXISTS events_session ON events(session_id, kind);",
        )?;
        let s = Stats { db };
        s.close_dangling()?;
        Ok(s)
    }

    /// 上次异常退出没结束的场次：用最后一个事件的时间收尾。
    fn close_dangling(&self) -> Result<()> {
        self.db.execute(
            "UPDATE sessions SET
               ended_at = COALESCE((SELECT MAX(at) FROM events e WHERE e.session_id = sessions.id), started_at),
               stop_reason = 'crash'
             WHERE ended_at IS NULL",
            [],
        )?;
        Ok(())
    }

    pub fn begin(&self, mode: &str) -> Result<i64> {
        self.db.execute("INSERT INTO sessions(started_at, mode) VALUES (?1, ?2)", params![now_str(), mode])?;
        Ok(self.db.last_insert_rowid())
    }

    pub fn event(&self, session: i64, kind: &str, data: &serde_json::Value) -> Result<()> {
        self.db.execute(
            "INSERT INTO events(session_id, at, kind, data) VALUES (?1, ?2, ?3, ?4)",
            params![session, now_str(), kind, data.to_string()],
        )?;
        Ok(())
    }

    pub fn progress(&self, session: i64, catches: u32, active_ms: u64) -> Result<()> {
        self.db.execute(
            "UPDATE sessions SET catches = ?2, active_ms = ?3 WHERE id = ?1",
            params![session, catches, active_ms as i64],
        )?;
        Ok(())
    }

    pub fn end(&self, session: i64, catches: u32, active_ms: u64, reason: &str) -> Result<()> {
        self.db.execute(
            "UPDATE sessions SET ended_at = ?2, catches = ?3, active_ms = ?4, stop_reason = ?5 WHERE id = ?1",
            params![session, now_str(), catches, active_ms as i64, reason],
        )?;
        Ok(())
    }

    pub fn summary(&self) -> Result<Summary> {
        let (catches, active_ms, sessions, since): (i64, i64, i64, Option<String>) = self.db.query_row(
            "SELECT COALESCE(SUM(catches),0), COALESCE(SUM(active_ms),0), COUNT(*), MIN(started_at) FROM sessions",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;
        let today = Local::now().format("%Y-%m-%d").to_string();
        let today_catches: i64 = self.db.query_row(
            "SELECT COALESCE(SUM(catches),0) FROM sessions WHERE substr(started_at,1,10) = ?1",
            [today],
            |r| r.get(0),
        )?;
        let avg_bite_ms: Option<f64> = self
            .db
            .query_row(
                "SELECT AVG(CAST(json_extract(data,'$.biteWaitMs') AS REAL)) FROM events WHERE kind='catch'",
                [],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        Ok(Summary { catches, active_ms, sessions, since: since.map(|s| s[..10].to_string()), today_catches, avg_bite_ms })
    }

    pub fn days(&self, n: u32) -> Result<Vec<Day>> {
        let from = (Local::now() - chrono::Duration::days(n as i64 - 1)).format("%Y-%m-%d").to_string();
        let mut q = self.db.prepare(
            "SELECT substr(started_at,1,10) d, SUM(catches) FROM sessions WHERE d >= ?1 GROUP BY d ORDER BY d",
        )?;
        let rows = q.query_map([from], |r| Ok(Day { date: r.get(0)?, catches: r.get(1)? }))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// 全部有记录的日子（导入旧数据后近 7 天可能是空的，界面退回显示这个）。
    pub fn all_days(&self, limit: u32) -> Result<Vec<Day>> {
        let mut q = self.db.prepare(
            "SELECT * FROM (SELECT substr(started_at,1,10) d, SUM(catches) c FROM sessions GROUP BY d ORDER BY d DESC LIMIT ?1) ORDER BY d",
        )?;
        let rows = q.query_map([limit], |r| Ok(Day { date: r.get(0)?, catches: r.get(1)? }))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn bite_buckets(&self) -> Result<Vec<Bucket>> {
        let edges: [(i64, i64, &str); 6] = [
            (0, 5_000, "<5 秒"),
            (5_000, 10_000, "5–10"),
            (10_000, 15_000, "10–15"),
            (15_000, 20_000, "15–20"),
            (20_000, 30_000, "20–30"),
            (30_000, i64::MAX, "30+"),
        ];
        let mut out = Vec::new();
        for (lo, hi, label) in edges {
            let count: i64 = self.db.query_row(
                "SELECT COUNT(*) FROM events WHERE kind='catch'
                   AND CAST(json_extract(data,'$.biteWaitMs') AS INTEGER) >= ?1
                   AND CAST(json_extract(data,'$.biteWaitMs') AS INTEGER) < ?2",
                params![lo, hi],
                |r| r.get(0),
            )?;
            out.push(Bucket { label: label.into(), count });
        }
        Ok(out)
    }

    pub fn recent(&self, limit: u32) -> Result<Vec<SessionRow>> {
        let mut q = self.db.prepare(
            "SELECT id, started_at, active_ms, catches, mode, source, stop_reason FROM sessions
             ORDER BY started_at DESC LIMIT ?1",
        )?;
        let rows = q.query_map([limit], |r| {
            Ok(SessionRow {
                id: r.get(0)?,
                started_at: r.get(1)?,
                active_ms: r.get(2)?,
                catches: r.get(3)?,
                mode: r.get(4)?,
                source: r.get(5)?,
                stop_reason: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn view(&self, days: u32) -> Result<StatsView> {
        let mut d = self.days(days)?;
        if d.iter().all(|x| x.catches == 0) {
            d = self.all_days(days)?;
        }
        Ok(StatsView { summary: self.summary()?, days: d, bite_buckets: self.bite_buckets()?, recent: self.recent(50)? })
    }

    pub fn export_csv(&self, path: &Path) -> Result<usize> {
        let rows = self.recent(u32::MAX)?;
        let mut s = String::from("\u{feff}开始时间,时长(秒),条数,模式,来源,结束原因\n");
        for r in &rows {
            s.push_str(&format!(
                "{},{:.0},{},{},{},{}\n",
                r.started_at,
                r.active_ms as f64 / 1000.0,
                r.catches,
                r.mode,
                r.source,
                r.stop_reason.clone().unwrap_or_default()
            ));
        }
        std::fs::write(path, s)?;
        Ok(rows.len())
    }

    /// 导入 v0.1 的 `data/stats.json`。重复导入不会重复计数。
    pub fn import_legacy(&self, path: &Path) -> Result<usize> {
        #[derive(Deserialize)]
        struct Legacy {
            history: Vec<Entry>,
        }
        #[derive(Deserialize)]
        struct Entry {
            start: String,
            seconds: f64,
            fish: i64,
            mode: Option<String>,
        }
        let raw = std::fs::read_to_string(path).with_context(|| format!("读取 {}", path.display()))?;
        let legacy: Legacy = serde_json::from_str(&raw).context("不是 v0.1 的 stats.json")?;
        let mut n = 0;
        for e in legacy.history {
            let Ok(naive) = NaiveDateTime::parse_from_str(&e.start, "%Y-%m-%d %H:%M") else { continue };
            let Some(start): Option<DateTime<Local>> = Local.from_local_datetime(&naive).earliest() else { continue };
            let end = start + chrono::Duration::milliseconds((e.seconds * 1000.0) as i64);
            let mode = if e.mode.as_deref() == Some("full") { "full" } else { "rodOnly" };
            let fmt = |d: DateTime<Local>| d.format("%Y-%m-%dT%H:%M:%S%.3f%:z").to_string();
            n += self.db.execute(
                "INSERT OR IGNORE INTO sessions(started_at, ended_at, mode, catches, active_ms, stop_reason, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'user', 'legacy')",
                params![fmt(start), fmt(end), mode, e.fish, (e.seconds * 1000.0) as i64],
            )?;
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_lifecycle_and_summary() {
        let s = Stats::memory().unwrap();
        let id = s.begin("rodOnly").unwrap();
        s.event(id, "catch", &serde_json::json!({"biteWaitMs": 8000})).unwrap();
        s.event(id, "catch", &serde_json::json!({"biteWaitMs": 12000})).unwrap();
        s.end(id, 2, 60_000, "user").unwrap();
        let sum = s.summary().unwrap();
        assert_eq!((sum.catches, sum.sessions, sum.today_catches), (2, 1, 2));
        assert_eq!(sum.avg_bite_ms, Some(10_000.0));
        let b = s.bite_buckets().unwrap();
        assert_eq!(b[1].count, 1);
        assert_eq!(b[2].count, 1);
        assert_eq!(s.view(7).unwrap().days.len(), 1);
    }

    #[test]
    fn legacy_import_is_idempotent() {
        let dir = std::env::temp_dir().join(format!("moyu-legacy-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("stats.json");
        std::fs::write(
            &p,
            r#"{"career":{"fish":7},"history":[
              {"start":"2026-07-08 22:18","seconds":12.0,"fish":0,"mode":"semi","target":"xp"},
              {"start":"2026-07-08 23:10","seconds":223.1,"fish":7,"mode":"full","target":"hook"}]}"#,
        )
        .unwrap();
        let s = Stats::memory().unwrap();
        assert_eq!(s.import_legacy(&p).unwrap(), 2);
        assert_eq!(s.import_legacy(&p).unwrap(), 0);
        let sum = s.summary().unwrap();
        assert_eq!((sum.catches, sum.sessions), (7, 2));
        assert_eq!(sum.since.as_deref(), Some("2026-07-08"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
