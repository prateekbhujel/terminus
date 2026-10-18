//! In-process task scheduling engine for Terminus (inspired by Laravel's Task Scheduler).
//!
//! Enables defining and executing recurring background tasks fluently in Rust
//! without configuring OS-level crontab services.

use chrono::{DateTime, Utc};
use std::sync::{Arc, Mutex};

/// Recurrence frequency for a scheduled task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frequency {
    EveryMinute,
    EveryFiveMinutes,
    EveryTenMinutes,
    EveryFifteenMinutes,
    EveryThirtyMinutes,
    Hourly,
    Daily,
    Weekly,
}

impl Frequency {
    pub fn interval_seconds(&self) -> i64 {
        match self {
            Self::EveryMinute => 60,
            Self::EveryFiveMinutes => 300,
            Self::EveryTenMinutes => 600,
            Self::EveryFifteenMinutes => 900,
            Self::EveryThirtyMinutes => 1800,
            Self::Hourly => 3600,
            Self::Daily => 86400,
            Self::Weekly => 604800,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::EveryMinute => "Every Minute",
            Self::EveryFiveMinutes => "Every 5 Minutes",
            Self::EveryTenMinutes => "Every 10 Minutes",
            Self::EveryFifteenMinutes => "Every 15 Minutes",
            Self::EveryThirtyMinutes => "Every 30 Minutes",
            Self::Hourly => "Hourly",
            Self::Daily => "Daily",
            Self::Weekly => "Weekly",
        }
    }
}

/// A recurring background task with execution metadata.
#[derive(Clone)]
pub struct ScheduledTask {
    pub name: String,
    pub command: Option<String>,
    pub frequency: Frequency,
    pub last_run: Arc<Mutex<Option<DateTime<Utc>>>>,
    handler: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl std::fmt::Debug for ScheduledTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScheduledTask")
            .field("name", &self.name)
            .field("command", &self.command)
            .field("frequency", &self.frequency)
            .finish()
    }
}

impl ScheduledTask {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            command: None,
            frequency: Frequency::Daily,
            last_run: Arc::new(Mutex::new(None)),
            handler: None,
        }
    }

    pub fn every_minute(mut self) -> Self {
        self.frequency = Frequency::EveryMinute;
        self
    }

    pub fn every_five_minutes(mut self) -> Self {
        self.frequency = Frequency::EveryFiveMinutes;
        self
    }

    pub fn every_ten_minutes(mut self) -> Self {
        self.frequency = Frequency::EveryTenMinutes;
        self
    }

    pub fn every_fifteen_minutes(mut self) -> Self {
        self.frequency = Frequency::EveryFifteenMinutes;
        self
    }

    pub fn every_thirty_minutes(mut self) -> Self {
        self.frequency = Frequency::EveryThirtyMinutes;
        self
    }

    pub fn hourly(mut self) -> Self {
        self.frequency = Frequency::Hourly;
        self
    }

    pub fn daily(mut self) -> Self {
        self.frequency = Frequency::Daily;
        self
    }

    pub fn weekly(mut self) -> Self {
        self.frequency = Frequency::Weekly;
        self
    }

    /// Determines if this task is due for execution.
    pub fn is_due(&self) -> bool {
        let now = Utc::now();
        if let Ok(guard) = self.last_run.lock() {
            if let Some(last) = *guard {
                (now - last).num_seconds() >= self.frequency.interval_seconds()
            } else {
                true
            }
        } else {
            true
        }
    }

    /// Executes the task callback if one was provided.
    pub fn execute(&self) -> bool {
        if let Ok(mut guard) = self.last_run.lock() {
            *guard = Some(Utc::now());
        }
        if let Some(h) = &self.handler {
            h();
            true
        } else {
            true
        }
    }
}

/// Central application task schedule registry.
#[derive(Debug, Default, Clone)]
pub struct Schedule {
    tasks: Arc<Mutex<Vec<ScheduledTask>>>,
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Registers a closure-based scheduled task.
    pub fn call<F>(&self, name: impl Into<String>, handler: F) -> TaskBuilder
    where
        F: Fn() + Send + Sync + 'static,
    {
        let task = ScheduledTask {
            name: name.into(),
            command: None,
            frequency: Frequency::Daily,
            last_run: Arc::new(Mutex::new(None)),
            handler: Some(Arc::new(handler)),
        };
        TaskBuilder {
            task,
            schedule: self.clone(),
        }
    }

    /// Registers a CLI command to be executed on a schedule.
    pub fn command(&self, cmd: impl Into<String>) -> TaskBuilder {
        let cmd_str = cmd.into();
        let task = ScheduledTask {
            name: cmd_str.clone(),
            command: Some(cmd_str),
            frequency: Frequency::Daily,
            last_run: Arc::new(Mutex::new(None)),
            handler: None,
        };
        TaskBuilder {
            task,
            schedule: self.clone(),
        }
    }

    /// Adds a configured task to the schedule.
    pub fn add(&self, task: ScheduledTask) {
        if let Ok(mut tasks) = self.tasks.lock() {
            tasks.push(task);
        }
    }

    /// Returns a list of all registered scheduled tasks.
    pub fn tasks(&self) -> Vec<ScheduledTask> {
        self.tasks.lock().map(|t| t.clone()).unwrap_or_default()
    }

    /// Executes all scheduled tasks that are currently due.
    pub fn run_due(&self) -> Vec<String> {
        let mut executed = Vec::new();
        if let Ok(tasks) = self.tasks.lock() {
            for task in tasks.iter() {
                if task.is_due() {
                    task.execute();
                    executed.push(task.name.clone());
                }
            }
        }
        executed
    }
}

/// Fluent builder for configuring recurrence frequency of a scheduled task.
pub struct TaskBuilder {
    task: ScheduledTask,
    schedule: Schedule,
}

impl TaskBuilder {
    pub fn every_minute(mut self) {
        self.task.frequency = Frequency::EveryMinute;
        self.schedule.add(self.task);
    }

    pub fn every_five_minutes(mut self) {
        self.task.frequency = Frequency::EveryFiveMinutes;
        self.schedule.add(self.task);
    }

    pub fn every_ten_minutes(mut self) {
        self.task.frequency = Frequency::EveryTenMinutes;
        self.schedule.add(self.task);
    }

    pub fn every_fifteen_minutes(mut self) {
        self.task.frequency = Frequency::EveryFifteenMinutes;
        self.schedule.add(self.task);
    }

    pub fn every_thirty_minutes(mut self) {
        self.task.frequency = Frequency::EveryThirtyMinutes;
        self.schedule.add(self.task);
    }

    pub fn hourly(mut self) {
        self.task.frequency = Frequency::Hourly;
        self.schedule.add(self.task);
    }

    pub fn daily(mut self) {
        self.task.frequency = Frequency::Daily;
        self.schedule.add(self.task);
    }

    pub fn weekly(mut self) {
        self.task.frequency = Frequency::Weekly;
        self.schedule.add(self.task);
    }
}
