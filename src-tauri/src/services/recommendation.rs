use crate::types::*;
use chrono::{DateTime, Duration, Utc};
use std::collections::{HashMap, HashSet};
use crate::services::algorithms;

#[derive(Debug, thiserror::Error)]
pub enum RecommendationError {
    #[error("No events available")]
    NoEvents,

    #[error("Insufficient data for recommendation")]
    InsufficientData,

    #[error("Database error: {0}")]
    DatabaseError(String),
}

/// 窗口停留记录
#[derive(Debug, Clone)]
struct DwellRecord {
    hwnd: WindowHandle,
    process_name: String,
    dwell_secs: u64,
    last_active: DateTime<Utc>,
    switch_count: u32,
    dwell_sessions: Vec<u64>,
}

pub struct RecommendationEngine {
    /// 所有焦点事件
    events: Vec<WindowFocusEvent>,
    /// 每个窗口（按 hwnd + process_name 组合）的累计停留时间
    /// 使用组合 key 避免 Windows 回收 hwnd 导致的重复记录
    dwell_records: HashMap<(i64, String), DwellRecord>,
    /// 共现矩阵: (app1, app2) -> 共现次数
    co_occurrence: HashMap<(String, String), u32>,
    /// 推荐设置
    settings: RecommendationSettings,
    /// 进程名缓存: pid -> process_name，减少 OpenProcess 调用频率
    process_name_cache: HashMap<u32, String>,
    /// [T3] 时间衰减加权共现值（近期共现权重更高）
    co_occurrence_weight: HashMap<(String, String), f64>,
    /// [T3] 完整时序事件（保留切换顺序，供 Apriori/PrefixSpan 工作流分析）
    ordered_events: Vec<WindowFocusEvent>,
}

impl RecommendationEngine {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            dwell_records: HashMap::new(),
            co_occurrence: HashMap::new(),
            settings: RecommendationSettings::default(),
            process_name_cache: HashMap::new(),
            co_occurrence_weight: HashMap::new(),
            ordered_events: Vec::new(),
        }
    }

    pub fn update_from_events(&mut self, events: &[WindowFocusEvent]) {
        // [CR 修复] PID 可能被复用，缓存超过阈值时清空重建，避免返回过期进程名
        const MAX_CACHE_SIZE: usize = 200;
        if self.process_name_cache.len() > MAX_CACHE_SIZE {
            self.process_name_cache.clear();
        }

        // 第一步：验证 + 过滤（保留完整时序，用于 dwell/co-occurrence 计算）
        // 只过滤掉无效 hwnd 和被回收的 hwnd，不破坏切换序列
        let mut valid_events: Vec<WindowFocusEvent> = Vec::new();

        for event in events.iter() {
            let hwnd_val = event.hwnd.0;

            let hwnd = windows::Win32::Foundation::HWND(hwnd_val as *mut _);
            let is_valid = unsafe {
                windows::Win32::UI::WindowsAndMessaging::IsWindow(Some(hwnd)).as_bool()
            };

            if !is_valid {
                continue;
            }

            let mut pid: u32 = 0;
            let _ = unsafe {
                windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(hwnd, Some(&mut pid))
            };

            if pid == 0 {
                continue;
            }

            // [ATT&CK 整改] 使用进程名缓存
            let current_process_name = if let Some(cached) = self.process_name_cache.get(&pid) {
                cached.clone()
            } else {
                let name = unsafe {
                    let handle = windows::Win32::System::Threading::OpenProcess(
                        windows::Win32::System::Threading::PROCESS_QUERY_INFORMATION |
                        windows::Win32::System::Threading::PROCESS_VM_READ,
                        false,
                        pid
                    );

                    match handle {
                        Ok(h) => {
                            let mut buffer = [0u16; 260];
                            let mut size: u32 = 260;
                            let result = windows::Win32::System::Threading::QueryFullProcessImageNameW(
                                h,
                                windows::Win32::System::Threading::PROCESS_NAME_FORMAT(0),
                                windows::core::PWSTR(buffer.as_mut_ptr()),
                                &mut size
                            );

                            let _ = windows::Win32::Foundation::CloseHandle(h);

                            if result.is_ok() {
                                let path = String::from_utf16_lossy(&buffer[..size as usize]);
                                std::path::Path::new(&path)
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("unknown")
                                    .to_string()
                            } else {
                                String::new()
                            }
                        }
                        Err(_) => String::new()
                    }
                };
                self.process_name_cache.insert(pid, name.clone());
                name
            };

            if current_process_name != event.process_name {
                log::debug!("update_from_events: skipping hwnd={} - recycled: expected={}, actual={}",
                    hwnd_val, event.process_name, current_process_name);
                continue;
            }

            valid_events.push(event.clone());
        }

        // 第二步：去重（每个 hwnd 只保留最新事件）— 仅供 self.events 使用
        // self.events 用于窗口组生成，需要唯一的窗口列表
        let mut latest_event_per_hwnd: HashMap<i64, &WindowFocusEvent> = HashMap::new();
        for event in valid_events.iter() {
            let hwnd_val = event.hwnd.0;
            match latest_event_per_hwnd.get(&hwnd_val) {
                Some(existing) => {
                    if event.timestamp > existing.timestamp {
                        latest_event_per_hwnd.insert(hwnd_val, event);
                    }
                }
                None => {
                    latest_event_per_hwnd.insert(hwnd_val, event);
                }
            }
        }

        let deduped_events: Vec<WindowFocusEvent> = latest_event_per_hwnd
            .into_values()
            .cloned()
            .collect();

        log::info!("update_from_events: {} raw -> {} valid -> {} deduped (per hwnd)",
            events.len(), valid_events.len(), deduped_events.len());

        // self.events 用去重后的（窗口组生成需要唯一窗口列表）
        self.events = deduped_events;

        // [T3] 保存完整时序事件用于 Apriori/PrefixSpan 工作流挖掘
        self.ordered_events = valid_events.clone();

        // dwell 和 co-occurrence 必须用完整时序事件（保留切换序列）
        // 这是频繁切换推荐算法的核心数据源
        self.calculate_dwell_times(&valid_events);
        self.build_co_occurrence_matrix(&valid_events);
    }

    pub fn update_settings(&mut self, settings: RecommendationSettings) {
        self.settings = settings;
    }

    pub fn get_settings(&self) -> &RecommendationSettings {
        &self.settings
    }

    /// 从数据库加载历史数据
    pub fn load_from_database(&mut self, db: &crate::services::database::DatabaseService) -> Result<(), RecommendationError> {
        // 加载 dwell_records
        match db.get_dwell_records() {
            Ok(records) => {
                for row in records {
                    let key = (row.hwnd, row.process_name.clone());
                    let record = DwellRecord {
                        hwnd: WindowHandle(row.hwnd),
                        process_name: row.process_name,
                        dwell_secs: row.dwell_secs as u64,
                        last_active: DateTime::from_timestamp(row.last_active, 0).unwrap_or_else(Utc::now),
                        switch_count: row.switch_count as u32,
                        dwell_sessions: Vec::new(), // 数据库中不存储会话详情
                    };
                    self.dwell_records.insert(key, record);
                }
                log::info!("Loaded {} dwell records from database", self.dwell_records.len());
            }
            Err(e) => {
                log::warn!("Failed to load dwell records from database: {}", e);
            }
        }

        // 加载 co_occurrence
        match db.get_co_occurrence() {
            Ok(records) => {
                for row in records {
                    let key = (row.process1, row.process2);
                    self.co_occurrence.insert(key, row.count as u32);
                }
                log::info!("Loaded {} co-occurrence records from database", self.co_occurrence.len());
            }
            Err(e) => {
                log::warn!("Failed to load co-occurrence from database: {}", e);
            }
        }

        // 加载推荐设置
        match db.get_latest_recommendation_settings() {
            Ok(Some(json)) => {
                match serde_json::from_str::<RecommendationSettings>(&json) {
                    Ok(settings) => {
                        self.settings = settings;
                        log::info!("Loaded recommendation settings from database");
                    }
                    Err(e) => {
                        log::warn!("Failed to parse recommendation settings: {}", e);
                    }
                }
            }
            Ok(None) => {
                log::info!("No recommendation settings in database, using defaults");
            }
            Err(e) => {
                log::warn!("Failed to load recommendation settings from database: {}", e);
            }
        }

        Ok(())
    }

    /// 保存当前统计数据到数据库
    pub fn save_to_database(&self, db: &crate::services::database::DatabaseService) -> Result<(), RecommendationError> {
        let now = Utc::now().timestamp();

        // 保存 dwell_records
        let dwell_rows: Vec<DwellRecordRow> = self.dwell_records.values().map(|r| {
            DwellRecordRow {
                id: 0, // 数据库自动生成
                hwnd: r.hwnd.0,
                process_name: r.process_name.clone(),
                dwell_secs: r.dwell_secs as i64,
                switch_count: r.switch_count as i32,
                last_active: r.last_active.timestamp(),
                created_at: now,
            }
        }).collect();

        if let Err(e) = db.save_dwell_records(&dwell_rows) {
            log::error!("Failed to save dwell records to database: {}", e);
            return Err(RecommendationError::DatabaseError(e.to_string()));
        }

        // 保存 co_occurrence
        for ((p1, p2), count) in &self.co_occurrence {
            if let Err(e) = db.save_co_occurrence(p1, p2, *count as i32) {
                log::error!("Failed to save co-occurrence to database: {}", e);
                return Err(RecommendationError::DatabaseError(e.to_string()));
            }
        }

        // 保存推荐设置
        match serde_json::to_string(&self.settings) {
            Ok(json) => {
                if let Err(e) = db.save_recommendation_settings(&json) {
                    log::error!("Failed to save recommendation settings to database: {}", e);
                    return Err(RecommendationError::DatabaseError(e.to_string()));
                }
            }
            Err(e) => {
                log::error!("Failed to serialize recommendation settings: {}", e);
                return Err(RecommendationError::DatabaseError(e.to_string()));
            }
        }

        log::info!("Saved recommendation data to database: {} dwell records, {} co-occurrence pairs", 
            dwell_rows.len(), self.co_occurrence.len());

        Ok(())
    }

    /// 计算每个窗口的停留时间，追踪切换次数和单次停留时间
    /// 按 (hwnd, process_name) 组合独立记录，避免 Windows 回收 hwnd 导致的重复
    fn calculate_dwell_times(&mut self, events: &[WindowFocusEvent]) {
        self.dwell_records.clear();

        if events.len() < 2 {
            return;
        }

        // 注意：事件已在 update_from_events 中验证过有效性，这里直接使用
        // 按时间排序事件
        let mut sorted_events: Vec<&WindowFocusEvent> = events.iter().collect();
        sorted_events.sort_by_key(|e| e.timestamp);

        let mut i = 0;
        while i < sorted_events.len() {
            let current = sorted_events[i];
            let record_key = (current.hwnd.0, current.process_name.clone());

            // 找到下一个不同窗口的事件（同一 hwnd 的连续事件视为一次停留）
            let mut dwell_secs = 0u64;
            let mut j = i + 1;
            while j < sorted_events.len() && sorted_events[j].hwnd == current.hwnd {
                j += 1;
            }

            if j < sorted_events.len() {
                let time_diff = sorted_events[j].timestamp - current.timestamp;
                dwell_secs = time_diff.num_seconds().max(0) as u64;
            } else if i + 1 < sorted_events.len() {
                // 最后一个事件，用下一个事件的时间差
                let time_diff = sorted_events[i + 1].timestamp - current.timestamp;
                dwell_secs = time_diff.num_seconds().max(0) as u64;
            }

            // 累加到该 (hwnd, process_name) 组合
            let record = self.dwell_records
                .entry(record_key)
                .or_insert_with(|| DwellRecord {
                    hwnd: current.hwnd,
                    process_name: current.process_name.clone(),
                    dwell_secs: 0,
                    last_active: current.timestamp,
                    switch_count: 0,
                    dwell_sessions: Vec::new(),
                });

            record.dwell_secs += dwell_secs;
            record.last_active = current.timestamp.max(record.last_active);
            record.hwnd = current.hwnd;
            record.switch_count += 1;
            record.dwell_sessions.push(dwell_secs);

            i = j;
        }
    }

    /// 构建共现矩阵 - 基于时间窗口内的频繁切换
    fn build_co_occurrence_matrix(&mut self, events: &[WindowFocusEvent]) {
        self.co_occurrence.clear();
        self.co_occurrence_weight.clear();

        if events.len() < 2 {
            return;
        }

        // [T3] 时间衰减半衰期：3 天（72 小时）
        const HALF_LIFE_HOURS: f64 = 72.0;
        let now = Utc::now();

        // 时间窗口：30 分钟
        let window = Duration::minutes(30);

        let mut i = 0;
        while i < events.len() {
            let current = &events[i];
            let mut window_apps: HashSet<String> = HashSet::new();
            window_apps.insert(current.process_name.clone());

            // 收集时间窗口内的所有应用
            let mut j = i + 1;
            while j < events.len() {
                let time_diff = events[j].timestamp - current.timestamp;
                if time_diff > window {
                    break;
                }
                if events[j].process_name != current.process_name {
                    window_apps.insert(events[j].process_name.clone());
                }
                j += 1;
            }

            // 生成共现对
            let apps: Vec<String> = window_apps.into_iter().collect();
            for a in 0..apps.len() {
                for b in (a + 1)..apps.len() {
                    let key = if apps[a] < apps[b] {
                        (apps[a].clone(), apps[b].clone())
                    } else {
                        (apps[b].clone(), apps[a].clone())
                    };
                    *self.co_occurrence.entry(key.clone()).or_insert(0) += 1;
                    // [T3] 叠加时间衰减权重
                    let age_hours = (now - current.timestamp).num_seconds() as f64 / 3600.0;
                    *self.co_occurrence_weight.entry(key).or_insert(0.0) +=
                        algorithms::time_decay_weight(age_hours, HALF_LIFE_HOURS);
                }
            }

            i = j.max(i + 1);
        }
    }

    /// 清理已销毁窗口的事件（内部方法，在生成推荐前调用）
    fn cleanup_destroyed_events_internal(&mut self, destroyed_hwnds: &std::collections::HashSet<i64>) {
        if destroyed_hwnds.is_empty() {
            return;
        }
        
        // 从 events 中移除已销毁窗口的事件
        let before_count = self.events.len();
        self.events.retain(|e| !destroyed_hwnds.contains(&e.hwnd.0));
        let removed_events = before_count - self.events.len();
        
        // 从 dwell_records 中移除已销毁窗口的记录
        let before_dwell_count = self.dwell_records.len();
        self.dwell_records.retain(|(hwnd, _), _| !destroyed_hwnds.contains(hwnd));
        let removed_dwell = before_dwell_count - self.dwell_records.len();
        
        if removed_events > 0 || removed_dwell > 0 {
            log::info!("Cleaned up {} events and {} dwell records for {} destroyed windows",
                removed_events, removed_dwell, destroyed_hwnds.len());
        }
    }

    pub fn generate_recommendations(
        &mut self,
        current_window: &WindowFocusEvent,
        max_count: usize,
        mouse_events: &[MouseActivityEvent],
        destroyed_hwnds: &std::collections::HashSet<i64>,
    ) -> Result<RecommendationResponse, RecommendationError> {
        if self.events.is_empty() {
            return Err(RecommendationError::NoEvents);
        }

        // 先清理已销毁窗口的事件
        self.cleanup_destroyed_events_internal(destroyed_hwnds);

        let ignore_set: HashSet<String> = self.settings.ignore_list.iter().cloned().collect();

        // 生成窗口组（共现频率最高的 2~5 个窗口）
        let groups = self.generate_window_groups(current_window, &ignore_set, mouse_events);

        // 构建推荐组列表
        let mut result_groups: Vec<RecommendationGroupInfo> = Vec::new();

        // 当前窗口进程 - 始终排除在"频繁切换"之外
        let current_process = &current_window.process_name;

        // 第一个组：常用组合 - 当前窗口始终包含，其他窗口需满足停留时间阈值
        let mut windows: Vec<RecommendationWindowInfo> = Vec::new();
        // 按 hwnd 去重，使同一进程的多个窗口（如多个 explorer.exe）都能显示
        let mut seen_hwnds: HashSet<i64> = HashSet::new();

        // 首先确保当前窗口被添加（即使事件列表中找不到，即使它在忽略清单中）
        // 当前窗口是用户正在使用的，应该始终显示
        let current_process_name = &current_window.process_name;
        let dwell = self.dwell_records.get(&(current_window.hwnd.0, current_window.process_name.clone()))
            .map(|r| r.dwell_secs)
            .unwrap_or(0);
        windows.push(RecommendationWindowInfo {
            hwnd: current_window.hwnd,
            process_name: current_process_name.clone(),
            dwell_time_secs: dwell,
        });
        seen_hwnds.insert(current_window.hwnd.0);

        log::info!("Added current window: {} hwnd={} (dwell: {}s)", current_process_name, current_window.hwnd.0, dwell);

        // 如果有共现组，添加其他窗口
        if let Some(group) = groups.first() {
            for hwnd in &group.windows {
                // 跳过已添加的窗口
                if seen_hwnds.contains(&hwnd.0) {
                    continue;
                }

                if let Some(event) = self.events.iter().find(|e| e.hwnd == *hwnd) {
                    let process = &event.process_name;
                    // 检查忽略清单
                    if ignore_set.contains(process) {
                        continue;
                    }
                    seen_hwnds.insert(hwnd.0);

                    let effective_dwell = self.calculate_effective_dwell(hwnd, process, mouse_events);

                    // 其他窗口准入条件：有效停留时间 >= 阈值
                    if effective_dwell < self.settings.common_combo_min_dwell_secs {
                        log::debug!("Window {} (hwnd={}) excluded from common combo: effective dwell {}s < {}s",
                            process, hwnd.0, effective_dwell, self.settings.common_combo_min_dwell_secs);
                        continue;
                    }

                    windows.push(RecommendationWindowInfo {
                        hwnd: *hwnd,
                        process_name: process.clone(),
                        dwell_time_secs: effective_dwell,
                    });
                }
            }
        }

        // 限制窗口数量
        windows.truncate(max_count);

        // 即使只有当前窗口也应该显示（至少有1个窗口）
        if !windows.is_empty() {
            result_groups.push(RecommendationGroupInfo {
                windows,
                label: "常用组合".to_string(),
            });
        }

        // 第二个组：频繁切换 - 排除当前窗口和已在常用组合中的窗口
        let frequent_switchers = self.generate_frequent_switcher_recommendations(current_window, &ignore_set);
        if !frequent_switchers.is_empty() {
            let mut windows: Vec<RecommendationWindowInfo> = Vec::new();
            let mut seen_hwnds: HashSet<i64> = HashSet::new();

            // 始终排除当前窗口
            seen_hwnds.insert(current_window.hwnd.0);

            // 排除已在常用组合中的窗口
            if let Some(first_group) = result_groups.first() {
                for w in &first_group.windows {
                    seen_hwnds.insert(w.hwnd.0);
                }
            }

            for singleton in frequent_switchers {
                if !seen_hwnds.contains(&singleton.hwnd.0) && windows.len() < max_count {
                    seen_hwnds.insert(singleton.hwnd.0);
                    windows.push(RecommendationWindowInfo {
                        hwnd: singleton.hwnd,
                        process_name: singleton.process_name,
                        dwell_time_secs: singleton.dwell_time_secs,
                    });
                }
            }

            if !windows.is_empty() {
                result_groups.push(RecommendationGroupInfo {
                    windows,
                    label: "频繁切换".to_string(),
                });
            }
        }

        // [T3] 高频工作流组（Apriori 频繁项集 + PrefixSpan 序列排序），additive
        if let Some(workflow) = self.generate_workflow_group(current_window, &ignore_set, max_count) {
            let existing: HashSet<i64> = result_groups
                .iter()
                .flat_map(|g| g.windows.iter().map(|w| w.hwnd.0))
                .collect();
            let new_hwnds: HashSet<i64> = workflow.windows.iter().map(|w| w.hwnd.0).collect();
            if new_hwnds != existing {
                result_groups.push(workflow);
            }
        }

        log::info!("generate_recommendations: {} groups returned", result_groups.len());

        Ok(RecommendationResponse { groups: result_groups })
    }

    /// 计算某个窗口在指定时间段内的有效工作时间（剔除鼠标静止超过阈值的时间）
    fn calculate_effective_dwell(&self, hwnd: &WindowHandle, process_name: &str, mouse_events: &[MouseActivityEvent]) -> u64 {
        let record = match self.dwell_records.get(&(hwnd.0, process_name.to_string())) {
            Some(r) => r,
            None => return 0,
        };

        if mouse_events.is_empty() {
            return record.dwell_secs;
        }

        let idle_threshold = self.settings.common_combo_mouse_idle_threshold_secs;
        let mut total_idle_time = 0u64;

        // 遍历该进程的所有停留会话
        for &session_dwell in &record.dwell_sessions {
            if session_dwell == 0 {
                continue;
            }

            // 找出该会话期间的鼠标事件
            // 简化处理：统计所有与该进程相关的鼠标静止时间
            let mut session_idle_time = 0u64;
            
            for i in 0..mouse_events.len().saturating_sub(1) {
                let curr = &mouse_events[i];
                let next = &mouse_events[i + 1];

                // 检查鼠标事件是否属于该进程的窗口
                if curr.hwnd != record.hwnd {
                    continue;
                }

                let time_diff = (next.timestamp - curr.timestamp).num_seconds().max(0) as u64;

                // 如果鼠标静止超过阈值，计入空闲时间
                if time_diff >= idle_threshold {
                    session_idle_time += time_diff;
                }
            }

            total_idle_time += session_idle_time;
        }

        // 返回总有效时间
        record.dwell_secs.saturating_sub(total_idle_time)
    }

    /// 生成频繁切换推荐 - 单次停留时间短但切换频繁的窗口
    fn generate_frequent_switcher_recommendations(
        &self,
        current_window: &WindowFocusEvent,
        ignore_set: &HashSet<String>,
    ) -> Vec<SingletonRecommendation> {
        let max_dwell = self.settings.recent_max_dwell_secs;
        let min_switches = self.settings.recent_min_switch_count;

        log::info!("generate_frequent_switcher_recommendations: max_dwell={}s, min_switches={}, total_records={}",
            max_dwell, min_switches, self.dwell_records.len());

        // 放宽条件：只要切换次数达到要求，且有短停留会话就显示
        let mut candidates: Vec<SingletonRecommendation> = Vec::new();
        let mut seen_processes: HashSet<String> = HashSet::new();

        for r in self.dwell_records.values() {
            // 跳过当前窗口
            if r.hwnd == current_window.hwnd {
                continue;
            }

            // 跳过忽略清单中的进程
            if ignore_set.contains(&r.process_name) {
                continue;
            }

            // 跳过切换次数不足的进程
            if r.switch_count < min_switches {
                continue;
            }

            // 进程名去重：每个进程名只保留一个窗口（优先保留停留时间最长的）
            if seen_processes.contains(&r.process_name) {
                // 检查是否应该替换已有的记录（保留停留时间更长的）
                if let Some(existing) = candidates.iter().find(|c| c.process_name == r.process_name) {
                    if r.dwell_secs <= existing.dwell_time_secs {
                        continue;
                    }
                    // 移除旧的，添加新的
                    candidates.retain(|c| c.process_name != r.process_name);
                } else {
                    continue;
                }
            }

            // 注意：窗口有效性已在 update_from_events 中验证，这里不再重复验证
            // 检查是否有短停留会话（放宽条件：至少有一个短停留会话）
            let short_sessions = r.dwell_sessions.iter().filter(|&&s| s < max_dwell).count();
            let has_short_sessions = short_sessions > 0;

            log::info!("  {} (hwnd={}) : switch_count={}, short_sessions={}/{} (sessions={:?}), has_short_sessions={}",
                r.process_name, r.hwnd.0, r.switch_count, short_sessions, r.dwell_sessions.len(), r.dwell_sessions, has_short_sessions);

            if has_short_sessions {
                seen_processes.insert(r.process_name.clone());
                candidates.push(SingletonRecommendation {
                    hwnd: r.hwnd,
                    process_name: r.process_name.clone(),
                    dwell_time_secs: r.dwell_secs,
                });
            }
        }

        log::info!("frequent_switcher candidates after filter: {}", candidates.len());

        // 按切换次数降序排序（频繁切换的优先）
        candidates.sort_by(|a, b| {
            let a_switches = self.dwell_records.get(&(a.hwnd.0, a.process_name.clone())).map(|r| r.switch_count).unwrap_or(0);
            let b_switches = self.dwell_records.get(&(b.hwnd.0, b.process_name.clone())).map(|r| r.switch_count).unwrap_or(0);
            b_switches.cmp(&a_switches)
        });

        candidates.truncate(4);
        candidates
    }

    /// 生成窗口组推荐 - 基于共现频率，2~5 个窗口
    fn generate_window_groups(
        &mut self,
        current_window: &WindowFocusEvent,
        ignore_set: &HashSet<String>,
        _mouse_events: &[MouseActivityEvent],
    ) -> Vec<RecommendationGroup> {
        let mut groups = Vec::new();
        let current_process = &current_window.process_name;

        // 找到与当前窗口共现最多的应用
        let mut co_occurring_apps: Vec<(String, u32)> = Vec::new();

        for ((app1, app2), count) in &self.co_occurrence {
            if app1 == current_process && app2 != current_process {
                // 检查忽略清单
                if !ignore_set.contains(app2.as_str()) {
                    co_occurring_apps.push((app2.clone(), *count));
                }
            } else if app2 == current_process && app1 != current_process {
                // 检查忽略清单
                if !ignore_set.contains(app1.as_str()) {
                    co_occurring_apps.push((app1.clone(), *count));
                }
            }
        }

        // [T3] 按时间衰减加权的共现值排序（近期共现权重更高）
        co_occurring_apps.sort_by(|a, b| {
            let ka = if current_process < &a.0 {
                (current_process.clone(), a.0.clone())
            } else {
                (a.0.clone(), current_process.clone())
            };
            let kb = if current_process < &b.0 {
                (current_process.clone(), b.0.clone())
            } else {
                (b.0.clone(), current_process.clone())
            };
            let wa = self.co_occurrence_weight.get(&ka).copied().unwrap_or(a.1 as f64);
            let wb = self.co_occurrence_weight.get(&kb).copied().unwrap_or(b.1 as f64);
            wb.partial_cmp(&wa).unwrap_or(std::cmp::Ordering::Equal)
        });

        // 生成窗口组：当前窗口 + 共现最多的应用（2~5 个）
        // 即使没有共现数据，也要生成包含当前窗口的组
        let mut group_windows = vec![current_window.hwnd];
        let group_label = "常用组合".to_string();
        let mut added_processes: HashSet<String> = HashSet::new();
        added_processes.insert(current_process.clone());

        for (app, _count) in &co_occurring_apps {
            if group_windows.len() >= 5 {
                break;
            }

            // 跳过已添加的进程（防止重复）
            if added_processes.contains(app) {
                continue;
            }

            // 找到该应用的有效 hwnd（从最新到最旧遍历，找到第一个有效的）
            // 注意：事件已在 update_from_events 中验证过有效性，这里直接使用
            let mut found_valid_hwnd = false;
            for event in self.events.iter().rev() {
                if event.process_name == *app {
                    // 找到有效的 hwnd
                    if !group_windows.contains(&event.hwnd) {
                        group_windows.push(event.hwnd);
                        added_processes.insert(app.clone());
                        found_valid_hwnd = true;
                        log::debug!("Added co-occurring window {} (hwnd={})", app, event.hwnd.0);
                    }
                    break;
                }
            }

            if !found_valid_hwnd {
                log::debug!("No valid hwnd found for co-occurring app {}", app);
            }
        }

        // 如果没有共现数据，尝试添加其他最近使用的窗口
        if group_windows.len() < 2 {
            let mut recent_windows: Vec<(&DwellRecord)> = self
                .dwell_records
                .values()
                .filter(|r| r.hwnd != current_window.hwnd && !ignore_set.contains(&r.process_name))
                .collect();

            // 按最后活跃时间排序
            recent_windows.sort_by(|a, b| b.last_active.cmp(&a.last_active));

            for record in recent_windows {
                if group_windows.len() >= 5 {
                    break;
                }
                
                // 跳过已添加的进程（防止同一进程名重复添加）
                if added_processes.contains(&record.process_name) {
                    continue;
                }
                
                // 验证窗口是否仍然存在且进程名匹配
                let hwnd = windows::Win32::Foundation::HWND(record.hwnd.0 as *mut _);
                let is_valid = unsafe {
                    windows::Win32::UI::WindowsAndMessaging::IsWindow(Some(hwnd)).as_bool()
                };
                
                if !is_valid {
                    log::debug!("Skipping recent window {} (hwnd={}) - no longer exists",
                        record.process_name, record.hwnd.0);
                    continue;
                }
                
                // 验证进程名是否匹配（防止 hwnd 被回收）
                let mut pid: u32 = 0;
                let _ = unsafe {
                    windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(hwnd, Some(&mut pid))
                };
                
                if pid == 0 {
                    continue;
                }
                
                // [ATT&CK 整改] 使用进程名缓存，避免重复 OpenProcess 调用
                let current_process_name = if let Some(cached) = self.process_name_cache.get(&pid) {
                    cached.clone()
                } else {
                    let name = unsafe {
                        let handle = windows::Win32::System::Threading::OpenProcess(
                            windows::Win32::System::Threading::PROCESS_QUERY_INFORMATION | 
                            windows::Win32::System::Threading::PROCESS_VM_READ,
                            false,
                            pid
                        );
                        
                        match handle {
                            Ok(h) => {
                                let mut buffer = [0u16; 260];
                                let mut size: u32 = 260;
                                let result = windows::Win32::System::Threading::QueryFullProcessImageNameW(
                                    h,
                                    windows::Win32::System::Threading::PROCESS_NAME_FORMAT(0),
                                    windows::core::PWSTR(buffer.as_mut_ptr()),
                                    &mut size
                                );
                                
                                let _ = windows::Win32::Foundation::CloseHandle(h);
                                
                                if result.is_ok() {
                                    let path = String::from_utf16_lossy(&buffer[..size as usize]);
                                    std::path::Path::new(&path)
                                        .file_name()
                                        .and_then(|n| n.to_str())
                                        .unwrap_or("unknown")
                                        .to_string()
                                } else {
                                    String::new()
                                }
                            }
                            Err(_) => String::new()
                        }
                    };
                    self.process_name_cache.insert(pid, name.clone());
                    name
                };
                
                if current_process_name != record.process_name {
                    log::debug!("Skipping recent window {} (hwnd={}) - hwnd recycled: actual={}",
                        record.process_name, record.hwnd.0, current_process_name);
                    continue;
                }
                
                if !group_windows.contains(&record.hwnd) {
                    group_windows.push(record.hwnd);
                    added_processes.insert(record.process_name.clone());
                    log::debug!("Added recent window {} (hwnd={})", record.process_name, record.hwnd.0);
                }
            }
        }

        // 始终生成包含当前窗口的组（即使只有1个窗口）
        if group_windows.len() >= 1 {
            let score = if co_occurring_apps.is_empty() {
                0.0
            } else {
                self.calculate_group_score(&co_occurring_apps)
            };
            groups.push(RecommendationGroup {
                windows: group_windows,
                score,
                label: group_label,
            });
        }

        // 也可以生成纯共现组（不包含当前窗口）
        let mut seen_pairs: HashSet<(String, String)> = HashSet::new();

        for ((app1, app2), count) in &self.co_occurrence {
            // 检查忽略清单
            if ignore_set.contains(app1) || ignore_set.contains(app2) {
                continue;
            }

            let key = (app1.clone(), app2.clone());
            if seen_pairs.contains(&key) {
                continue;
            }
            seen_pairs.insert(key);

            if *count >= 3 {
                let mut group_windows = Vec::new();

                for app in [app1, app2] {
                    for event in &self.events {
                        if event.process_name == *app {
                            if !group_windows.contains(&event.hwnd) {
                                group_windows.push(event.hwnd);
                            }
                            break;
                        }
                    }
                }

                if group_windows.len() >= 2 && group_windows.len() <= 5 {
                    let score = (*count as f32).ln() / 5.0;
                    groups.push(RecommendationGroup {
                        windows: group_windows,
                        score,
                        label: format!("常用组合 (共现 {} 次)", count),
                    });
                }
            }
        }

        groups.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        groups.truncate(3);

        groups
    }

    /// [T3] 基于 Apriori 频繁项集 + PrefixSpan 序列模式生成"高频工作流"推荐组（additive）
    fn generate_workflow_group(
        &self,
        current_window: &WindowFocusEvent,
        ignore_set: &HashSet<String>,
        max_count: usize,
    ) -> Option<RecommendationGroupInfo> {
        let transactions = self.build_session_transactions();
        if transactions.len() < 2 {
            return None;
        }
        let itemsets = algorithms::apriori(&transactions, 2);
        let current_process = &current_window.process_name;

        // 选取包含当前应用、长度>=2 且不含忽略项的最高支持度项集
        let best = itemsets.into_iter().find(|s| {
            s.items.len() >= 2
                && s.items.contains(current_process)
                && s.items.iter().all(|it| !ignore_set.contains(it))
        })?;

        // PrefixSpan：按"紧随当前应用出现的可能性"对项集内应用排序
        let sequences = self.build_switch_sequences();
        let patterns = algorithms::prefixspan(&sequences, 2, 3);
        let predicted = algorithms::predict_next(&patterns, current_process);
        let rank_of = |app: &str| -> usize {
            predicted.iter().position(|(a, _)| a == app).unwrap_or(usize::MAX)
        };

        let mut other_apps: Vec<String> = best
            .items
            .iter()
            .filter(|a| a.as_str() != current_process.as_str())
            .cloned()
            .collect();
        other_apps.sort_by_key(|a| rank_of(a));

        let mut windows: Vec<RecommendationWindowInfo> = Vec::new();
        let mut seen: HashSet<i64> = HashSet::new();
        let dwell = self
            .dwell_records
            .get(&(current_window.hwnd.0, current_process.clone()))
            .map(|r| r.dwell_secs)
            .unwrap_or(0);
        windows.push(RecommendationWindowInfo {
            hwnd: current_window.hwnd,
            process_name: current_process.clone(),
            dwell_time_secs: dwell,
        });
        seen.insert(current_window.hwnd.0);

        for app in other_apps {
            if windows.len() >= max_count {
                break;
            }
            if let Some(ev) = self
                .events
                .iter()
                .rev()
                .find(|e| e.process_name == app && !seen.contains(&e.hwnd.0))
            {
                seen.insert(ev.hwnd.0);
                let d = self
                    .dwell_records
                    .get(&(ev.hwnd.0, ev.process_name.clone()))
                    .map(|r| r.dwell_secs)
                    .unwrap_or(0);
                windows.push(RecommendationWindowInfo {
                    hwnd: ev.hwnd,
                    process_name: app,
                    dwell_time_secs: d,
                });
            }
        }

        if windows.len() >= 2 {
            Some(RecommendationGroupInfo {
                windows,
                label: format!("高频工作流 (共现 {} 次)", best.support),
            })
        } else {
            None
        }
    }

    /// 按 30 分钟会话切分，构建 Apriori 事务（每个事务=会话内应用集合）
    fn build_session_transactions(&self) -> Vec<Vec<String>> {
        if self.ordered_events.is_empty() {
            return Vec::new();
        }
        let mut sorted: Vec<&WindowFocusEvent> = self.ordered_events.iter().collect();
        sorted.sort_by_key(|e| e.timestamp);
        let gap = Duration::minutes(30);
        let mut transactions: Vec<Vec<String>> = Vec::new();
        let mut current_set: HashSet<String> = HashSet::new();
        let mut session_start = sorted[0].timestamp;
        for ev in &sorted {
            if ev.timestamp - session_start > gap {
                if current_set.len() >= 2 {
                    transactions.push(current_set.iter().cloned().collect());
                }
                current_set.clear();
                session_start = ev.timestamp;
            }
            current_set.insert(ev.process_name.clone());
        }
        if current_set.len() >= 2 {
            transactions.push(current_set.into_iter().collect());
        }
        transactions
    }

    /// 按 30 分钟会话切分，构建 PrefixSpan 有序切换序列（合并连续相同应用）
    fn build_switch_sequences(&self) -> Vec<Vec<String>> {
        if self.ordered_events.is_empty() {
            return Vec::new();
        }
        let mut sorted: Vec<&WindowFocusEvent> = self.ordered_events.iter().collect();
        sorted.sort_by_key(|e| e.timestamp);
        let gap = Duration::minutes(30);
        let mut sequences: Vec<Vec<String>> = Vec::new();
        let mut seq: Vec<String> = Vec::new();
        let mut session_start = sorted[0].timestamp;
        for ev in &sorted {
            if ev.timestamp - session_start > gap {
                if seq.len() >= 2 {
                    sequences.push(std::mem::take(&mut seq));
                } else {
                    seq.clear();
                }
                session_start = ev.timestamp;
            }
            if seq.last().map(|l| l != &ev.process_name).unwrap_or(true) {
                seq.push(ev.process_name.clone());
            }
        }
        if seq.len() >= 2 {
            sequences.push(seq);
        }
        sequences
    }

    fn calculate_group_score(&self, co_occurring_apps: &[(String, u32)]) -> f32 {
        let total_count: u32 = co_occurring_apps.iter().map(|(_, c)| c).sum();
        (total_count as f32).ln() / 5.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{MonitorId, WindowFocusEvent, WindowHandle};
    use chrono::TimeDelta;

    fn ev(hwnd: i64, proc: &str, secs_ago: i64) -> WindowFocusEvent {
        WindowFocusEvent {
            hwnd: WindowHandle(hwnd),
            pid: 1,
            process_name: proc.to_string(),
            window_title_hash: "h".to_string(),
            timestamp: Utc::now() - TimeDelta::seconds(secs_ago),
            monitor_id: MonitorId(0),
        }
    }

    #[test]
    fn test_co_occurrence_matrix() {
        let mut engine = RecommendationEngine::new();
        let events = vec![
            ev(1, "code.exe", 300),
            ev(2, "chrome.exe", 200),
            ev(3, "terminal.exe", 100),
        ];
        engine.build_co_occurrence_matrix(&events);
        // 3 个应用两两共现 => 3 对
        assert_eq!(engine.co_occurrence.len(), 3);
        let key = ("chrome.exe".to_string(), "code.exe".to_string());
        assert_eq!(engine.co_occurrence.get(&key).copied().unwrap_or(0), 1);
        // 时间衰减加权应为正
        assert!(engine.co_occurrence_weight.get(&key).copied().unwrap_or(0.0) > 0.0);
    }

    #[test]
    fn test_co_occurrence_empty() {
        let mut engine = RecommendationEngine::new();
        engine.build_co_occurrence_matrix(&[]);
        assert!(engine.co_occurrence.is_empty());
        assert!(engine.co_occurrence_weight.is_empty());
    }

    #[test]
    fn test_dwell_times_switch_count() {
        let mut engine = RecommendationEngine::new();
        let events = vec![
            ev(1, "a.exe", 500),
            ev(2, "b.exe", 400),
            ev(1, "a.exe", 300),
            ev(2, "b.exe", 200),
        ];
        engine.calculate_dwell_times(&events);
        let rec = engine.dwell_records.get(&(1, "a.exe".to_string()));
        assert!(rec.is_some());
        assert_eq!(rec.unwrap().switch_count, 2);
    }

    #[test]
    fn test_workflow_transactions() {
        let mut engine = RecommendationEngine::new();
        engine.ordered_events = vec![
            ev(1, "code.exe", 300),
            ev(2, "chrome.exe", 200),
            ev(3, "terminal.exe", 100),
        ];
        let tx = engine.build_session_transactions();
        assert_eq!(tx.len(), 1);
        assert_eq!(tx[0].len(), 3);
    }
}
