pub mod app;
pub mod auth;
pub mod cache;
pub mod client;
pub mod debug;
pub mod document;
pub mod domain;
pub mod filter;
pub mod form;
pub mod helpers;
pub mod integrations;
pub mod mail;
pub mod module;
pub mod orm;
pub mod panel;
pub mod prompt;
pub mod query;
pub mod queue;
pub mod repository;
pub mod resource;
pub mod scaffold;
pub mod schedule;
pub mod session;
pub mod spec;
pub mod studio;
pub mod table;
pub mod testing;
pub mod theme;
pub mod token;
pub mod view;
pub mod ws;

pub mod db {
    pub use crate::orm::{
        Database, DatabaseConfig, DatabaseEngine, MongoCollection, Paginated, RedisStore, Seeder,
        SeederRegistry, SqlQuery,
    };
    pub use crate::query;
}

pub mod prelude {
    pub use axum::response::IntoResponse;
    pub use crate::app::{html_response, json_response, redirect_to, App};
    pub use crate::auth::Hash;
    pub use crate::cache::{CacheDriver, MemoryCache, RedisCache};
    pub use crate::client::{
        BarcodeScanEvent, DesktopMessage, DeviceRecord, DeviceRegistry, DeviceTelemetry,
        NativeDesktopBridge, OfflineSyncQueue, OfflineSyncRecord, Platform, PushDispatcher,
        PushNotification, SdkEndpoint, SdkGenerator,
    };
    pub use crate::document::{ExcelDocument, PdfDocument};
    pub use crate::domain::{
        matches_permission, AuditLog, Order, OrderStatus, PermissionCache, Role, User, UserStatus,
    };
    pub use crate::filter::{matches_filters, Filter, FilterPreset, FilterType};
    pub use crate::form::{FieldType, Form, FormField, FormSection, Section};
    pub use crate::helpers::{
        abort, abort_403, abort_404, abort_if, abort_unless, auth, can, can_all, can_any, cannot,
        config, config_set, env, flash, flash_error, flash_success, is_admin, is_superadmin, now,
        redirect, request, session, set_current_request, set_current_session, set_current_user,
        timestamp, today, user, user_id, with_request, with_user, AbortError, Redirect,
        RedirectResponse, RequestAccessor, RequestInfo, Result,
    };
    pub use crate::integrations::{
        OAuthProvider, PasskeyChallenge, PasskeyCredential, WebhookVerifier,
    };
    pub use crate::mail::{MailMessage, Mailer};
    pub use crate::module::{
        ApiResponse, AppModule, DynamicModule, JsonValidator, Valid, Validate, ValidatedRequest,
        ValidationErrors, Validator,
    };
    pub use crate::orm::{
        BelongsTo, Blueprint, Database, DatabaseConfig, DatabaseEngine, DynamicModel, HasMany,
        Model, ModelQuery, MongoCollection, Paginated, RedisStore, Schema, Seeder, SeederRegistry,
        SqlQuery,
    };
    pub use crate::query;
    pub use crate::db;
    pub use crate::panel::AdminPanel;
    pub use crate::prompt;
    pub use crate::query::{OrderDir, Query, WhereClause};
    pub use crate::queue::{Job, JobQueue, QueueStats};
    pub use crate::repository::{
        AuditRepository, InMemoryAuditRepository, InMemoryOrderRepository, InMemoryUserRepository,
        OrderRepository, SqliteAuditRepository, SqliteOrderRepository, SqliteUserRepository,
        UserRepository,
    };
    pub use crate::resource::{
        headline, to_camel_case, to_snake_case, AuditLogResource, FormMode, QueryState, Resource,
        RowData,
    };
    pub use crate::scaffold::{
        parse_targets, scaffold_project, DatabaseChoice, FrontendChoice, ProjectConfig,
        ProjectPreset, TargetPlatform,
    };
    pub use crate::schedule::{Frequency, Schedule, ScheduledTask};
    pub use crate::session::{FlashBag, Session, SessionStore};
    pub use crate::spec::{AppSpec, FieldSpec, ModelSpec, ResourceSpec, RouteSpec, ServiceSpec};
    pub use crate::studio::StudioApp;
    pub use crate::table::{
        Action, BulkAction, Column, ColumnType, Table, TableAction, TableFilter, TableStyle,
    };
    pub use crate::testing::{expect, test, test as terminus_test, Expectation, TestClient, TestResponse};
    pub use crate::theme::{FontFamily, PrimaryColor, ThemeConfig};
    pub use crate::token::{
        BearerToken, HasApiTokens, MemoryTokenStore, NewAccessToken, PersonalAccessToken,
        SqliteTokenStore, TokenRepository,
    };
    pub use crate::view::{raw_html, view, View};
    pub use crate::ws::{BroadcastEvent, Broadcaster};
}
