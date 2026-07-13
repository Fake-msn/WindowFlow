use crate::types::*;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result as SqliteResult};
use std::sync::{Arc, Mutex};

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("SQLite error: {0}")]
    SqliteError(#[from] rusqlite::Error),

    #[error("Database not initialized")]
    NotInitialized,

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

#[derive(Clone)]
pub struct DatabaseService {
    conn: Arc<Mutex<Connection>>,
}

impl DatabaseService {
    pub fn new(db_path: &str) -> Result<Self, DatabaseError> {
        let conn = Connection::open(db_path)?;

        conn.execute_batch("PRAGMA journal_mode=WAL;")?;

        let service = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        service.init_tables()?;

        Ok(service)
    }

    pub fn init_tables(&self) -> Result<(), DatabaseError> {
        let conn = self.conn.lock().map_err(|_| DatabaseError::NotInitialized)?;

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS focus_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                hwnd INTEGER NOT NULL,
                pid INTEGER NOT NULL,
                process_name TEXT NOT NULL,
                window_title_hash TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                monitor_id INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_timestamp ON focus_events(timestamp);
            CREATE INDEX IF NOT EXISTS idx_process ON focus_events(process_name);
            CREATE INDEX IF NOT EXISTS idx_monitor ON focus_events(monitor_id);

            -- 窗口停留记录表（用于推荐引擎）
            CREATE TABLE IF NOT EXISTS dwell_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                hwnd INTEGER NOT NULL,
                process_name TEXT NOT NULL,
                dwell_secs INTEGER NOT NULL,
                switch_count INTEGER NOT NULL,
                last_active INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_dwell_process ON dwell_records(process_name);
            CREATE INDEX IF NOT EXISTS idx_dwell_last_active ON dwell_records(last_active);

            -- 共现矩阵表（记录应用之间的共现关系）
            CREATE TABLE IF NOT EXISTS co_occurrence (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                process1 TEXT NOT NULL,
                process2 TEXT NOT NULL,
                count INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                UNIQUE(process1, process2)
            );

            CREATE INDEX IF NOT EXISTS idx_cooccurrence_processes ON co_occurrence(process1, process2);

            -- 推荐设置表（持久化用户配置）
            CREATE TABLE IF NOT EXISTS recommendation_settings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                settings_json TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            );

            -- 模型调用计数表（用于滚动清理）
            CREATE TABLE IF NOT EXISTS model_call_count (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                call_count INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL
            );

            -- 模型推荐结果表（保留模型判断供后续排查）
            CREATE TABLE IF NOT EXISTS model_recommendations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                scenario_type TEXT NOT NULL,
                window_combinations TEXT NOT NULL,
                confidence_score REAL NOT NULL,
                created_at INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_model_rec_created ON model_recommendations(created_at);
            ",
        )?;

        Ok(())
    }

    pub fn insert_focus_event(&self, event: &WindowFocusEvent) -> Result<(), DatabaseError> {
        let conn = self.conn.lock().map_err(|_| DatabaseError::NotInitialized)?;

        conn.execute(
            "INSERT INTO focus_events (hwnd, pid, process_name, window_title_hash, timestamp, monitor_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                event.hwnd.0,
                event.pid,
                event.process_name,
                event.window_title_hash,
                event.timestamp.timestamp(),
                event.monitor_id.0,
            ],
        )?;

        Ok(())
    }

    pub fn get_focus_events_since(
        &self,
        since: DateTime<Utc>,
    ) -> Result<Vec<WindowFocusEvent>, DatabaseError> {
        let conn = self.conn.lock().map_err(|_| DatabaseError::NotInitialized)?;

        let mut stmt = conn.prepare(
            "SELECT hwnd, pid, process_name, window_title_hash, timestamp, monitor_id
             FROM focus_events
             WHERE timestamp > ?1
             ORDER BY timestamp ASC",
        )?;

        let events = stmt.query_map(params![since.timestamp()], |row| {
            Ok(WindowFocusEvent {
                hwnd: WindowHandle(row.get::<_, i64>(0)?),
                pid: row.get(1)?,
                process_name: row.get(2)?,
                window_title_hash: row.get(3)?,
                timestamp: DateTime::from_timestamp(row.get::<_, i64>(4)?, 0)
                    .unwrap_or_else(Utc::now),
                monitor_id: MonitorId(row.get::<_, i64>(5)?),
            })
        })?;

        let mut result = Vec::new();
        for event in events {
            result.push(event?);
        }

        Ok(result)
    }

    pub fn get_usage_stats(&self, days: u32) -> Result<Vec<AppUsageStats>, DatabaseError> {
        let conn = self.conn.lock().map_err(|_| DatabaseError::NotInitialized)?;

        let since = Utc::now() - chrono::TimeDelta::days(days as i64);

        let mut stmt = conn.prepare(
            "SELECT process_name, COUNT(*) as focus_count, MAX(timestamp) as last_used
             FROM focus_events
             WHERE timestamp > ?1
             GROUP BY process_name
             ORDER BY focus_count DESC",
        )?;

        let stats = stmt.query_map(params![since.timestamp()], |row| {
            let last_used_ts: i64 = row.get(2)?;
            Ok(AppUsageStats {
                process_name: row.get(0)?,
                focus_count: row.get(1)?,
                total_focus_time: std::time::Duration::from_secs(0),
                last_used: DateTime::from_timestamp(last_used_ts, 0)
                    .unwrap_or_else(Utc::now),
            })
        })?;

        let mut result = Vec::new();
        for stat in stats {
            result.push(stat?);
        }

        Ok(result)
    }

    pub fn cleanup_old_data(&self, retention_days: u32) -> Result<u64, DatabaseError> {
        let conn = self.conn.lock().map_err(|_| DatabaseError::NotInitialized)?;

        let cutoff = Utc::now() - chrono::TimeDelta::days(retention_days as i64);

        let deleted = conn.execute(
            "DELETE FROM focus_events WHERE timestamp < ?1",
            params![cutoff.timestamp()],
        )?;

        Ok(deleted as u64)
    }

    pub fn get_all_events(&self) -> Result<Vec<WindowFocusEvent>, DatabaseError> {
        let conn = self.conn.lock().map_err(|_| DatabaseError::NotInitialized)?;

        let mut stmt = conn.prepare(
            "SELECT hwnd, pid, process_name, window_title_hash, timestamp, monitor_id
             FROM focus_events
             ORDER BY timestamp ASC",
        )?;

        let events = stmt.query_map([], |row| {
            Ok(WindowFocusEvent {
                hwnd: WindowHandle(row.get::<_, i64>(0)?),
                pid: row.get(1)?,
                process_name: row.get(2)?,
                window_title_hash: row.get(3)?,
                timestamp: DateTime::from_timestamp(row.get::<_, i64>(4)?, 0)
                    .unwrap_or_else(Utc::now),
                monitor_id: MonitorId(row.get::<_, i64>(5)?),
            })
        })?;

        let mut result = Vec::new();
        for event in events {
            result.push(event?);
        }

        Ok(result)
    }

    // ========== 窗口停留记录操作 ==========

    pub fn save_dwell_records(&self, records: &[DwellRecordRow]) -> Result<(), DatabaseError> {
        let conn = self.conn.lock().map_err(|_| DatabaseError::NotInitialized)?;
        
        for record in records {
            conn.execute(
                "INSERT OR REPLACE INTO dwell_records (hwnd, process_name, dwell_secs, switch_count, last_active, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    record.hwnd,
                    record.process_name,
                    record.dwell_secs,
                    record.switch_count,
                    record.last_active,
                    record.created_at,
                ],
            )?;
        }

        Ok(())
    }

    pub fn get_dwell_records(&self) -> Result<Vec<DwellRecordRow>, DatabaseError> {
        let conn = self.conn.lock().map_err(|_| DatabaseError::NotInitialized)?;

        let mut stmt = conn.prepare(
            "SELECT id, hwnd, process_name, dwell_secs, switch_count, last_active, created_at
             FROM dwell_records
             ORDER BY last_active DESC",
        )?;

        let records = stmt.query_map([], |row| {
            Ok(DwellRecordRow {
                id: row.get(0)?,
                hwnd: row.get(1)?,
                process_name: row.get(2)?,
                dwell_secs: row.get(3)?,
                switch_count: row.get(4)?,
                last_active: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;

        let mut result = Vec::new();
        for record in records {
            result.push(record?);
        }

        Ok(result)
    }

    // ========== 共现矩阵操作 ==========

    pub fn save_co_occurrence(&self, process1: &str, process2: &str, count: i32) -> Result<(), DatabaseError> {
        let conn = self.conn.lock().map_err(|_| DatabaseError::NotInitialized)?;
        let now = Utc::now().timestamp();

        // 确保 process1 < process2（保持字典序）
        let (p1, p2) = if process1 < process2 {
            (process1, process2)
        } else {
            (process2, process1)
        };

        conn.execute(
            "INSERT INTO co_occurrence (process1, process2, count, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(process1, process2) DO UPDATE SET count = ?3, updated_at = ?4",
            params![p1, p2, count, now],
        )?;

        Ok(())
    }

    pub fn get_co_occurrence(&self) -> Result<Vec<CoOccurrenceRow>, DatabaseError> {
        let conn = self.conn.lock().map_err(|_| DatabaseError::NotInitialized)?;

        let mut stmt = conn.prepare(
            "SELECT id, process1, process2, count, updated_at
             FROM co_occurrence
             ORDER BY count DESC",
        )?;

        let records = stmt.query_map([], |row| {
            Ok(CoOccurrenceRow {
                id: row.get(0)?,
                process1: row.get(1)?,
                process2: row.get(2)?,
                count: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;

        let mut result = Vec::new();
        for record in records {
            result.push(record?);
        }

        Ok(result)
    }

    // ========== 推荐设置操作 ==========

    pub fn save_recommendation_settings(&self, settings_json: &str) -> Result<(), DatabaseError> {
        let conn = self.conn.lock().map_err(|_| DatabaseError::NotInitialized)?;
        let now = Utc::now().timestamp();

        conn.execute(
            "INSERT INTO recommendation_settings (settings_json, updated_at)
             VALUES (?1, ?2)",
            params![settings_json, now],
        )?;

        Ok(())
    }

    pub fn get_latest_recommendation_settings(&self) -> Result<Option<String>, DatabaseError> {
        let conn = self.conn.lock().map_err(|_| DatabaseError::NotInitialized)?;

        let mut stmt = conn.prepare(
            "SELECT settings_json FROM recommendation_settings
             ORDER BY updated_at DESC LIMIT 1",
        )?;

        let result = stmt.query_row([], |row| row.get::<_, String>(0));

        match result {
            Ok(json) => Ok(Some(json)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DatabaseError::SqliteError(e)),
        }
    }

    // ========== 数据清理操作 ==========

    pub fn cleanup_old_dwell_records(&self, retention_days: u32) -> Result<u64, DatabaseError> {
        let conn = self.conn.lock().map_err(|_| DatabaseError::NotInitialized)?;
        let cutoff = Utc::now() - chrono::TimeDelta::days(retention_days as i64);

        let deleted = conn.execute(
            "DELETE FROM dwell_records WHERE last_active < ?1",
            params![cutoff.timestamp()],
        )?;

        Ok(deleted as u64)
    }

    pub fn cleanup_old_co_occurrence(&self, retention_days: u32) -> Result<u64, DatabaseError> {
        let conn = self.conn.lock().map_err(|_| DatabaseError::NotInitialized)?;
        let cutoff = Utc::now() - chrono::TimeDelta::days(retention_days as i64);

        let deleted = conn.execute(
            "DELETE FROM co_occurrence WHERE updated_at < ?1",
            params![cutoff.timestamp()],
        )?;

        Ok(deleted as u64)
    }

    pub fn cleanup_old_recommendation_settings(&self, keep_count: usize) -> Result<u64, DatabaseError> {
        let conn = self.conn.lock().map_err(|_| DatabaseError::NotInitialized)?;

        // 删除除最新的 keep_count 条之外的所有记录
        let deleted = conn.execute(
            "DELETE FROM recommendation_settings
             WHERE id NOT IN (
                 SELECT id FROM recommendation_settings
                 ORDER BY updated_at DESC
                 LIMIT ?1
             )",
            params![keep_count as i64],
        )?;

        Ok(deleted as u64)
    }

    // ========== 模型调用计数操作 ==========

    pub fn increment_model_call_count(&self) -> Result<i32, DatabaseError> {
        let conn = self.conn.lock().map_err(|_| DatabaseError::NotInitialized)?;
        let now = Utc::now().timestamp();

        // 获取当前计数
        let current_count: i32 = conn.query_row(
            "SELECT call_count FROM model_call_count ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        let new_count = current_count + 1;

        // 更新或插入计数
        conn.execute(
            "INSERT INTO model_call_count (call_count, updated_at) VALUES (?1, ?2)
             ON CONFLICT(id) DO UPDATE SET call_count = ?1, updated_at = ?2",
            params![new_count, now],
        )?;

        Ok(new_count)
    }

    pub fn get_model_call_count(&self) -> Result<i32, DatabaseError> {
        let conn = self.conn.lock().map_err(|_| DatabaseError::NotInitialized)?;

        let count: i32 = conn.query_row(
            "SELECT call_count FROM model_call_count ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        Ok(count)
    }

    pub fn reset_model_call_count(&self) -> Result<(), DatabaseError> {
        let conn = self.conn.lock().map_err(|_| DatabaseError::NotInitialized)?;
        let now = Utc::now().timestamp();

        conn.execute(
            "INSERT INTO model_call_count (call_count, updated_at) VALUES (0, ?1)",
            params![now],
        )?;

        Ok(())
    }

    // ========== 模型推荐结果操作 ==========

    pub fn save_model_recommendation(
        &self,
        scenario_type: &str,
        window_combinations: &str,
        confidence_score: f64,
    ) -> Result<(), DatabaseError> {
        let conn = self.conn.lock().map_err(|_| DatabaseError::NotInitialized)?;
        let now = Utc::now().timestamp();

        conn.execute(
            "INSERT INTO model_recommendations (scenario_type, window_combinations, confidence_score, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![scenario_type, window_combinations, confidence_score, now],
        )?;

        Ok(())
    }

    pub fn get_recent_model_recommendations(&self, limit: i32) -> Result<Vec<ModelRecommendationRow>, DatabaseError> {
        let conn = self.conn.lock().map_err(|_| DatabaseError::NotInitialized)?;

        let mut stmt = conn.prepare(
            "SELECT id, scenario_type, window_combinations, confidence_score, created_at
             FROM model_recommendations
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;

        let records = stmt.query_map(params![limit], |row| {
            Ok(ModelRecommendationRow {
                id: row.get(0)?,
                scenario_type: row.get(1)?,
                window_combinations: row.get(2)?,
                confidence_score: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;

        let mut result = Vec::new();
        for record in records {
            result.push(record?);
        }

        Ok(result)
    }

    pub fn cleanup_old_model_recommendations(&self, keep_count: usize) -> Result<u64, DatabaseError> {
        let conn = self.conn.lock().map_err(|_| DatabaseError::NotInitialized)?;

        let deleted = conn.execute(
            "DELETE FROM model_recommendations
             WHERE id NOT IN (
                 SELECT id FROM model_recommendations
                 ORDER BY created_at DESC
                 LIMIT ?1
             )",
            params![keep_count as i64],
        )?;

        Ok(deleted as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_creation() {
        let dir = std::env::temp_dir();
        let db_path = dir.join("test_windowflow.db");
        let db_path_str = db_path.to_str().unwrap();
        let _ = std::fs::remove_file(db_path_str);
        let db = DatabaseService::new(db_path_str);
        assert!(db.is_ok());
        let _ = std::fs::remove_file(db_path_str);
    }

    #[test]
    fn test_insert_and_query_event() {
        let dir = std::env::temp_dir();
        let db_path = dir.join("test_windowflow_insert.db");
        let db_path_str = db_path.to_str().unwrap();
        let _ = std::fs::remove_file(db_path_str);

        let db = DatabaseService::new(db_path_str).unwrap();

        let event = WindowFocusEvent {
            hwnd: WindowHandle(12345),
            pid: 6789,
            process_name: "test.exe".to_string(),
            window_title_hash: "abc123".to_string(),
            timestamp: Utc::now(),
            monitor_id: MonitorId(1),
        };

        db.insert_focus_event(&event).unwrap();

        let events = db.get_all_events().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].process_name, "test.exe");

        let _ = std::fs::remove_file(db_path_str);
    }
}
