use terminus::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

struct TestModalResource;
impl Resource for TestModalResource {
    fn name(&self) -> &str { "Item" }
    fn plural_name(&self) -> &str { "Items" }
    fn slug(&self) -> &str { "items" }
    fn table(&self) -> Table { Table::new() }
    fn form(&self) -> Form { Form::new() }
    fn fetch_rows(&self, _query: &QueryState) -> (Vec<RowData>, usize) { (vec![], 0) }
}

struct TestPageResource;
impl Resource for TestPageResource {
    fn name(&self) -> &str { "Article" }
    fn plural_name(&self) -> &str { "Articles" }
    fn slug(&self) -> &str { "articles" }
    fn form_mode(&self) -> FormMode { FormMode::Page }
    fn table(&self) -> Table {
        Table::new()
            .striped()
            .filter(TableFilter::select("status", "Status", vec![("draft", "Draft"), ("published", "Published")]))
    }
    fn form(&self) -> Form {
        Form::new()
            .field(FormField::searchable_select("category", vec![("tech", "Technology"), ("life", "Lifestyle")]))
    }
    fn fetch_rows(&self, _query: &QueryState) -> (Vec<RowData>, usize) { (vec![], 0) }
}

#[test]
fn test_form_mode_defaults_and_customization() {
    let modal_res = TestModalResource;
    assert_eq!(modal_res.form_mode(), FormMode::Modal);

    let page_res = TestPageResource;
    assert_eq!(page_res.form_mode(), FormMode::Page);
}

#[test]
fn test_table_styles_and_filters() {
    let table = Table::new()
        .striped()
        .compact()
        .filter(TableFilter::select("role", "Role", vec![("admin", "Admin"), ("user", "User")]));

    assert_eq!(table.style, TableStyle::Compact);
    assert_eq!(table.filters.len(), 1);
    assert_eq!(table.filters[0].name, "role");
    assert_eq!(table.filters[0].label, "Role");
    assert_eq!(table.filters[0].options.len(), 2);
}

#[test]
fn test_searchable_select_form_field() {
    let field = FormField::searchable_select(
        "assignee",
        vec![("1", "Pratik"), ("2", "Dharma Raj"), ("3", "Elena Rostova")],
    );

    assert_eq!(field.name, "assignee");
    assert_eq!(field.label, "Assignee");
    match field.field_type {
        terminus::form::FieldType::SearchableSelect { options } => {
            assert_eq!(options.len(), 3);
            assert_eq!(options[0], ("1".to_string(), "Pratik".to_string()));
        }
        _ => panic!("Expected SearchableSelect field type"),
    }
}

#[test]
fn test_query_state_with_filters() {
    let mut filters = HashMap::new();
    filters.insert("role".to_string(), "Superadmin".to_string());
    filters.insert("status".to_string(), "Active".to_string());

    let query = QueryState {
        page: 1,
        per_page: 8,
        search: "pratik".to_string(),
        sort_by: Some("name".to_string()),
        sort_desc: false,
        filters,
    };

    assert_eq!(query.filters.get("role").unwrap(), "Superadmin");
    assert_eq!(query.filters.get("status").unwrap(), "Active");
}

#[test]
fn test_camel_case_and_headline_utilities() {
    assert_eq!(to_camel_case("created_at"), "createdAt");
    assert_eq!(to_camel_case("order_number"), "orderNumber");
    assert_eq!(to_snake_case("createdAt"), "created_at");
    assert_eq!(to_snake_case("orderNumber"), "order_number");

    assert_eq!(headline("createdAt"), "Created At");
    assert_eq!(headline("created_at"), "Created At");
    assert_eq!(headline("userName"), "User Name");
    assert_eq!(headline("user_name"), "User Name");
    assert_eq!(headline("id"), "ID");
}

#[test]
fn test_row_data_bidirectional_case_fallback() {
    let row1 = RowData::new("1").insert("createdAt", "2026-10-08");
    assert_eq!(row1.get("createdAt"), Some("2026-10-08"));
    assert_eq!(row1.get("created_at"), Some("2026-10-08"));

    let row2 = RowData::new("2").insert("user_name", "Pratik");
    assert_eq!(row2.get("userName"), Some("Pratik"));
    assert_eq!(row2.get("user_name"), Some("Pratik"));
}

#[test]
fn test_fluent_builder_methods() {
    let page_res = TestPageResource;
    assert_eq!(page_res.plural_name(), "Articles");
    assert_eq!(page_res.form_mode(), FormMode::Page);

    let table = Table::new().page_size(25).default_sort_by("createdAt");
    assert_eq!(table.default_page_size, 25);
    assert_eq!(table.default_sort_by, Some("createdAt".to_string()));

    let field = FormField::searchable_select("role", vec![("admin", "Admin")]);
    assert_eq!(field.name, "role");
}

#[test]
fn test_domain_json_camel_case_serialization() {
    let user = User {
        id: "1".into(),
        name: "Pratik".into(),
        email: "pratik@example.com".into(),
        role: Role::Superadmin,
        status: UserStatus::Active,
        created_at: "2026-10-08".into(),
        ..Default::default()
    };

    let user_json = serde_json::to_string(&user).unwrap();
    assert!(user_json.contains("\"createdAt\":\"2026-10-08\""));
    assert!(!user_json.contains("\"created_at\""));

    let log = AuditLog {
        id: "1".into(),
        user_name: "Pratik".into(),
        action: "CREATE".into(),
        resource: "users".into(),
        record_id: "2".into(),
        details: "Created record".into(),
        timestamp: "2026-10-08 12:00:00".into(),
    };

    let log_json = serde_json::to_string(&log).unwrap();
    assert!(log_json.contains("\"userName\":\"Pratik\""));
    assert!(log_json.contains("\"recordId\":\"2\""));
}

struct PolicyTestResource;
impl Resource for PolicyTestResource {
    fn name(&self) -> &str { "Invoice" }
    fn plural_name(&self) -> &str { "Invoices" }
    fn slug(&self) -> &str { "invoices" }
    fn table(&self) -> Table {
        Table::new().column(Column::text("status"))
    }
    fn form(&self) -> Form { Form::new() }
    fn fetch_rows(&self, _query: &QueryState) -> (Vec<RowData>, usize) {
        let r1 = RowData::new("1").insert("status", "Draft");
        let r2 = RowData::new("2").insert("status", "Settled");
        (vec![r1, r2], 2)
    }

    fn can_edit_row(&self, user: &User, row: &RowData) -> bool {
        if !self.can_edit(user) {
            return false;
        }
        row.get("status") != Some("Settled")
    }

    fn can_delete_row(&self, user: &User, row: &RowData) -> bool {
        if !self.can_delete(user) {
            return false;
        }
        row.get("status") != Some("Settled")
    }
}

#[test]
fn test_record_level_authorization_hooks() {
    let res = PolicyTestResource;
    let admin_user = User {
        id: "1".into(),
        name: "Admin".into(),
        email: "admin@example.com".into(),
        role: Role::Superadmin,
        status: UserStatus::Active,
        created_at: "2026-10-08".into(),
        ..Default::default()
    };
    let member_user = User {
        id: "2".into(),
        name: "Member".into(),
        email: "member@example.com".into(),
        role: Role::Member,
        status: UserStatus::Active,
        created_at: "2026-10-08".into(),
        ..Default::default()
    };

    let draft_row = RowData::new("1").insert("status", "Draft");
    let settled_row = RowData::new("2").insert("status", "Settled");

    // Member cannot edit or delete invoices at all
    assert!(!res.can_edit(&member_user));
    assert!(!res.can_edit_row(&member_user, &draft_row));
    assert!(!res.can_delete_row(&member_user, &draft_row));

    // Admin has invoices.edit & invoices.delete, but Settled records are locked
    assert!(res.can_edit(&admin_user));
    assert!(res.can_delete(&admin_user));

    // Draft row is editable and deletable
    assert!(res.can_edit_row(&admin_user, &draft_row));
    assert!(res.can_delete_row(&admin_user, &draft_row));

    // Settled row is locked by record-level policy
    assert!(!res.can_edit_row(&admin_user, &settled_row));
    assert!(!res.can_delete_row(&admin_user, &settled_row));
}

#[test]
fn test_policy_table_view_action_buttons() {
    let res = PolicyTestResource;
    let admin_user = User {
        id: "1".into(),
        name: "Admin".into(),
        email: "admin@example.com".into(),
        role: Role::Superadmin,
        status: UserStatus::Active,
        created_at: "2026-10-08".into(),
        ..Default::default()
    };

    let query = QueryState::default();
    let html = terminus::view::table_view::render_table_partial(&res, &query, &admin_user);

    // Row 1 (Draft) should have edit and delete dialog triggers
    assert!(html.contains("openEditDialog('invoices', '1'"));
    assert!(html.contains("openDeleteDialog('invoices', '1'"));

    // Row 2 (Settled) should NOT have edit or delete dialog triggers
    assert!(!html.contains("openEditDialog('invoices', '2'"));
    assert!(!html.contains("openDeleteDialog('invoices', '2'"));
    assert!(html.contains("—")); // Fallback dash for disabled row actions
}

#[test]
fn test_theme_and_brand_customization() {
    let theme = ThemeConfig::new()
        .brand_name("Acme Studio")
        .primary_color(PrimaryColor::Violet)
        .font_family(FontFamily::Inter);

    assert_eq!(theme.brand_name, "Acme Studio");
    assert_eq!(theme.primary_color.name(), "violet");
    assert_eq!(theme.primary_color.hex(), "#8b5cf6");
    assert_eq!(theme.font_family.font_family_css(), "'Inter', system-ui, -apple-system, sans-serif");
    assert!(theme.font_family.css_url().contains("family=Inter"));

    let user = User {
        id: "1".into(),
        name: "Admin".into(),
        email: "admin@acme.com".into(),
        role: Role::Superadmin,
        status: UserStatus::Active,
        created_at: "2026-10-08".into(),
        ..Default::default()
    };

    let html = terminus::view::layout::render_page(
        "Products",
        "products",
        &[],
        "<div>Content</div>",
        "",
        None,
        &user,
        &theme,
    );

    assert!(html.contains("<title>Products · Acme Studio</title>"));
    assert!(html.contains("Acme Studio"));
    assert!(html.contains("#8b5cf6"));
    assert!(html.contains("family=Inter"));
}

#[test]
fn test_job_queue_and_background_worker() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let queue = JobQueue::new();
    let ran_flag = Arc::new(AtomicBool::new(false));
    let ran_flag_clone = ran_flag.clone();

    queue.dispatch_fn("send_slack_alert", move || {
        ran_flag_clone.store(true, Ordering::SeqCst);
        Ok(())
    });

    queue.dispatch_fn("generate_invoice_pdf", || {
        Ok(())
    });

    assert_eq!(queue.stats().pending, 2);
    assert_eq!(queue.stats().processed, 0);

    let count = queue.work_all();
    assert_eq!(count, 2);
    assert_eq!(queue.stats().pending, 0);
    assert_eq!(queue.stats().processed, 2);
    assert_eq!(queue.stats().failed, 0);
    assert!(ran_flag.load(Ordering::SeqCst));
}

#[test]
fn test_transactional_mailer() {
    let mailer = Mailer::new();
    let msg = MailMessage::to("founder@startup.io")
        .subject("Welcome to Terminus")
        .line("Your enterprise admin kit is ready.")
        .action("Access Panel", "/admin");

    assert_eq!(msg.to, "founder@startup.io");
    assert_eq!(msg.subject, "Welcome to Terminus");
    assert_eq!(msg.lines.len(), 1);

    msg.send_via(&mailer).unwrap();

    let outbox = mailer.sent_messages();
    assert_eq!(outbox.len(), 1);
    assert_eq!(outbox[0].to, "founder@startup.io");
    assert_eq!(outbox[0].subject, "Welcome to Terminus");
}

#[test]
fn test_fluent_query_builder_sql_and_bindings() {
    let query = Query::table("orders")
        .select(vec!["id", "customer", "amount", "status"])
        .where_eq("status", "Paid")
        .where_gt("amount", "100")
        .order_by("created_at", "DESC")
        .paginate(1, 10);

    let (sql, bindings) = query.to_sql();
    assert_eq!(
        sql,
        "SELECT id, customer, amount, status FROM orders WHERE status = ? AND amount > ? ORDER BY created_at DESC LIMIT 10 OFFSET 0"
    );
    assert_eq!(bindings, vec!["Paid", "100"]);

    // Test filtered users query
    let query2 = Query::table("users")
        .where_eq("role", "admin")
        .where_like("email", "%@company.com")
        .order_by("id", "ASC")
        .limit(5);

    let (sql2, bindings2) = query2.to_sql();
    assert_eq!(
        sql2,
        "SELECT * FROM users WHERE role = ? AND email LIKE ? ORDER BY id ASC LIMIT 5"
    );
    assert_eq!(bindings2, vec!["admin", "%@company.com"]);
}

#[test]
fn test_cache_remember_and_ttl_expiration() {
    let cache = MemoryCache::new();

    // Cache miss followed by remember
    let val1 = cache.remember("stats:daily_sales", 300, || {
        "42000".to_string()
    });
    assert_eq!(val1, "42000");

    // Cache hit: callback not invoked
    let val2 = cache.remember("stats:daily_sales", 300, || {
        "99999".to_string()
    });
    assert_eq!(val2, "42000");

    // Direct get
    assert_eq!(cache.get("stats:daily_sales"), Some("42000".to_string()));

    // Forget
    assert!(cache.forget("stats:daily_sales"));
    assert_eq!(cache.get("stats:daily_sales"), None);

    // Flush
    cache.put("key1", "val1", None);
    cache.put("key2", "val2", None);
    cache.flush();
    assert_eq!(cache.get("key1"), None);
    assert_eq!(cache.get("key2"), None);
}

#[test]
fn test_fluent_expectations() {
    use terminus::prelude::*;

    terminus_test("it validates user properties with fluent syntax", || {
        let user = User {
            id: "u_42".to_string(),
            name: "Engineer".to_string(),
            email: "dev@terminus.dev".to_string(),
            role: Role::Superadmin,
            status: UserStatus::Active,
            created_at: "2026-10-08".to_string(),
            ..Default::default()
        };

        // Chainable assertions
        expect(&user.name).to_be("Engineer")
            .and(&user.email).to_contain("@terminus.dev")
            .and(&user.role).to_be(&Role::Superadmin)
            .and(user.status.is_active()).to_be_true();

        // Comparison and collections
        expect(100).to_be_greater_than(50);
        expect(25).to_be_less_than(50);
        expect("").to_be_empty();
        expect("non-empty").to_not_be_empty();

        // Options and Results
        let some_val: Option<i32> = Some(42);
        let none_val: Option<i32> = None;
        expect(some_val).to_be_some();
        expect(none_val).to_be_none();

        let ok_val: Result<&str, &str> = Ok("success");
        let err_val: Result<&str, &str> = Err("failure");
        expect(ok_val).to_be_ok();
        expect(err_val).to_be_err();
    });
}

#[test]
fn test_debug_dump_macro() {
    use terminus::dump;
    let data = vec!["terminus", "admin", "fast"];
    // Verify dump! macro executes smoothly without errors
    dump!(data);
}

#[tokio::test]
async fn test_websocket_broadcaster_pubsub() {
    use terminus::prelude::*;

    let broadcaster = Broadcaster::new(16);
    assert_eq!(broadcaster.subscriber_count(), 0);

    // Subscribe client 1 & 2
    let mut rx1 = broadcaster.subscribe();
    let mut rx2 = broadcaster.subscribe();
    assert_eq!(broadcaster.subscriber_count(), 2);

    // Broadcast event
    let payload = serde_json::json!({
        "order_id": "ORD-999",
        "amount": 450,
        "status": "Paid"
    });
    let receiver_count = broadcaster.broadcast("orders", "order:created", payload.clone());
    assert_eq!(receiver_count, 2);

    // Both clients receive identical event
    let msg1 = rx1.recv().await.unwrap();
    let msg2 = rx2.recv().await.unwrap();

    expect(&msg1.channel).to_be("orders");
    expect(&msg1.event).to_be("order:created");
    expect(msg1.data["order_id"].as_str().unwrap()).to_be("ORD-999");

    expect(&msg2.channel).to_be("orders");
    expect(&msg2.event).to_be("order:created");
    expect(msg2.data["amount"].as_i64().unwrap()).to_be(450);
}

struct ActionTestResource {
    records: std::sync::Mutex<HashMap<String, HashMap<String, String>>>,
}

impl ActionTestResource {
    fn new() -> Self {
        let mut map = HashMap::new();
        let mut r1 = HashMap::new();
        r1.insert("name".to_string(), "MacBook Pro".to_string());
        r1.insert("price".to_string(), "1999".to_string());
        let mut r2 = HashMap::new();
        r2.insert("name".to_string(), "Studio Display".to_string());
        r2.insert("price".to_string(), "1599".to_string());
        map.insert("1".to_string(), r1);
        map.insert("2".to_string(), r2);
        Self {
            records: std::sync::Mutex::new(map),
        }
    }
}

impl Resource for ActionTestResource {
    fn name(&self) -> &str { "Product" }
    fn plural_name(&self) -> &str { "Products" }
    fn slug(&self) -> &str { "products" }

    fn table(&self) -> Table {
        Table::new()
            .columns(vec![
                Column::text("name").label("Product Name").sortable(),
                Column::numeric("price").label("Price").sortable(),
            ])
            .actions(vec![
                Action::view(),
                Action::edit(),
                Action::replicate(),
                Action::delete(),
                Action::make("publish")
                    .label("Publish Now")
                    .icon("check")
                    .color("emerald")
                    .requires_confirmation(true)
                    .modal_heading("Publish Product")
                    .modal_description("Are you sure you want to make this product live?"),
            ])
            .bulk_actions(vec![
                BulkAction::export(),
                BulkAction::delete(),
                BulkAction::make("archive")
                    .label("Archive Selected")
                    .color("amber")
                    .requires_confirmation(true)
                    .modal_heading("Archive Records")
                    .modal_description("Archive all selected items?"),
            ])
            .empty_state_heading("No products in inventory")
            .empty_state_description("Add your first product to get started.")
    }

    fn fetch_rows(&self, _query: &QueryState) -> (Vec<RowData>, usize) {
        let guard = self.records.lock().unwrap();
        let mut rows = Vec::new();
        let mut ids: Vec<_> = guard.keys().cloned().collect();
        ids.sort();
        for id in ids {
            if let Some(map) = guard.get(&id) {
                let mut row = RowData::new(&id);
                for (k, v) in map {
                    row.values.insert(k.clone(), v.clone());
                }
                rows.push(row);
            }
        }
        let len = rows.len();
        (rows, len)
    }

    fn get_row(&self, id: &str) -> Option<RowData> {
        let guard = self.records.lock().unwrap();
        guard.get(id).map(|map| {
            let mut row = RowData::new(id);
            for (k, v) in map {
                row.values.insert(k.clone(), v.clone());
            }
            row
        })
    }

    fn create_row(&self, values: HashMap<String, String>) -> Result<String, String> {
        let mut guard = self.records.lock().unwrap();
        let next_id = (guard.len() + 1).to_string();
        guard.insert(next_id.clone(), values);
        Ok(next_id)
    }

    fn delete_row(&self, id: &str) -> Result<(), String> {
        let mut guard = self.records.lock().unwrap();
        guard.remove(id);
        Ok(())
    }
}

#[test]
fn test_actions_and_replicate_row() {
    let res = ActionTestResource::new();

    // Verify Action builder with fluent methods
    let custom_action = Action::make("duplicate")
        .label("Clone Product")
        .icon("copy")
        .color("indigo")
        .requires_confirmation(true)
        .modal_heading("Clone this item")
        .modal_description("Are you sure you want to clone this item?")
        .action_url("/custom/url");

    expect(&custom_action.id).to_be("duplicate");
    expect(&custom_action.label).to_be("Clone Product");
    expect(&custom_action.color).to_be("indigo");
    assert!(custom_action.requires_confirmation);
    expect(custom_action.modal_heading.as_deref().unwrap()).to_be("Clone this item");
    expect(custom_action.action_url.as_deref().unwrap()).to_be("/custom/url");

    // Test record replication hook
    let new_id = res.replicate_row("1").expect("Replication should succeed");
    let cloned_record = res.get_row(&new_id).expect("Cloned record should exist");
    expect(cloned_record.get("name").unwrap()).to_be("MacBook Pro (Copy)");
    expect(cloned_record.get("price").unwrap()).to_be("1999");

    // Test row action dispatcher
    let action_msg = res.handle_action("replicate", "2").expect("Dispatcher replicate should succeed");
    assert!(action_msg.contains("replicated successfully"));

    let (_, total) = res.fetch_rows(&QueryState::default());
    assert_eq!(total, 4); // 2 original + 2 replicated
}

#[test]
fn test_bulk_delete_and_streaming_export() {
    let res = ActionTestResource::new();

    // Verify BulkAction builder
    let bulk_act = BulkAction::make("archive")
        .label("Archive Selected")
        .color("amber")
        .requires_confirmation(true)
        .modal_heading("Confirm Archive")
        .modal_description("Selected records will be archived.");

    expect(&bulk_act.id).to_be("archive");
    expect(&bulk_act.label).to_be("Archive Selected");
    expect(&bulk_act.color).to_be("amber");
    assert!(bulk_act.requires_confirmation);
    expect(bulk_act.modal_heading.as_deref().unwrap()).to_be("Confirm Archive");

    // High-performance streaming CSV export
    let csv_output = res.bulk_export(&["1".to_string(), "2".to_string()], "csv").expect("CSV export should succeed");
    assert!(csv_output.contains("name,price") || csv_output.contains("price,name"));
    assert!(csv_output.contains("MacBook Pro"));
    assert!(csv_output.contains("Studio Display"));

    // High-performance streaming JSON export
    let json_output = res.bulk_export(&["1".to_string(), "2".to_string()], "json").expect("JSON export should succeed");
    assert!(json_output.contains("MacBook Pro"));
    assert!(json_output.contains("Studio Display"));
    assert!(json_output.starts_with('[') && json_output.ends_with(']'));

    // High-concurrency in-process batch delete
    let deleted_count = res.bulk_delete(&["1".to_string(), "2".to_string()]).expect("Bulk delete should succeed");
    assert_eq!(deleted_count, 2);

    let (_, remaining) = res.fetch_rows(&QueryState::default());
    assert_eq!(remaining, 0);
}

#[test]
fn test_form_section_and_field_builder() {
    // Section layout with columns and schema
    let section = Section::make("General Information")
        .description("Essential product attributes")
        .columns(2)
        .schema(vec![
            FormField::text("title")
                .helper_text("Enter full product title")
                .prefix("https://")
                .suffix(".io")
                .column_span(2),
            FormField::number("inventory")
                .default_value("50")
                .min("0")
                .max("1000")
                .step("1"),
            FormField::password("secret_key")
                .helper_text("API secret token")
                .disabled(),
            FormField::textarea("description")
                .rows(5)
                .placeholder("Markdown supported")
                .column_span(2),
            FormField::toggle("is_published")
                .helper_text("Toggle live visibility on storefront"),
            FormField::datetime("scheduled_release")
                .read_only(),
        ]);

    expect(section.title()).to_be("General Information");
    expect(section.description.as_deref().unwrap()).to_be("Essential product attributes");
    assert_eq!(section.columns, 2);
    assert_eq!(section.fields.len(), 6);

    // Verify field builders
    let title_field = &section.fields[0];
    expect(title_field.helper_text.as_deref().unwrap()).to_be("Enter full product title");
    expect(title_field.prefix.as_deref().unwrap()).to_be("https://");
    expect(title_field.suffix.as_deref().unwrap()).to_be(".io");
    assert_eq!(title_field.column_span, 2);

    let num_field = &section.fields[1];
    expect(num_field.default_value.as_deref().unwrap()).to_be("50");
    expect(num_field.min.as_deref().unwrap()).to_be("0");
    expect(num_field.max.as_deref().unwrap()).to_be("1000");

    let pwd_field = &section.fields[2];
    assert!(pwd_field.disabled);

    let desc_field = &section.fields[3];
    assert_eq!(desc_field.rows, Some(5));

    let dt_field = &section.fields[5];
    assert!(dt_field.read_only);
}

#[test]
fn test_table_view_renders_bulk_actions_and_empty_state() {
    let res = ActionTestResource::new();
    let admin_user = User {
        id: "1".into(),
        name: "Admin".into(),
        email: "admin@example.com".into(),
        role: Role::Superadmin,
        status: UserStatus::Active,
        created_at: "2026-10-08".into(),
        ..Default::default()
    };

    let query = QueryState::default();
    let html = terminus::view::table_view::render_table_partial(&res, &query, &admin_user);

    // Checkbox master and row selection
    assert!(html.contains("id=\"select-all-checkbox\""));
    assert!(html.contains("class=\"row-select-checkbox"));

    // Bulk actions bar
    assert!(html.contains("id=\"bulk-actions-toolbar\""));
    assert!(html.contains("executeBulkExport('products', 'csv')"));
    assert!(html.contains("executeBulkExport('products', 'json')"));
    assert!(html.contains("Delete Selected"));

    // Replicate action button
    assert!(html.contains("executeRowAction('products', 'replicate'"));

    // Custom row action
    assert!(html.contains("Publish Now"));

    // Empty state test
    let empty_query = QueryState {
        search: "Nonexistent".into(),
        ..Default::default()
    };
    struct EmptyResource;
    impl Resource for EmptyResource {
        fn name(&self) -> &str { "Order" }
        fn plural_name(&self) -> &str { "Orders" }
        fn slug(&self) -> &str { "orders" }
        fn table(&self) -> Table {
            Table::new()
                .empty_state_heading("No orders found")
                .empty_state_description("No customer orders recorded yet.")
        }
        fn fetch_rows(&self, _query: &QueryState) -> (Vec<RowData>, usize) { (vec![], 0) }
    }
    let empty_html = terminus::view::table_view::render_table_partial(&EmptyResource, &empty_query, &admin_user);
    assert!(empty_html.contains("No orders found"));
    assert!(empty_html.contains("No customer orders recorded yet."));
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct TestProduct {
    id: String,
    title: String,
    price: String,
    category_id: String,
    created_at: String,
    updated_at: String,
}

impl Model for TestProduct {
    fn table_name() -> &'static str {
        "test_products"
    }

    fn get_id(&self) -> Option<&str> {
        Some(&self.id)
    }

    fn set_id(&mut self, id: String) {
        self.id = id;
    }

    fn to_map(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert("id".to_string(), self.id.clone());
        map.insert("title".to_string(), self.title.clone());
        map.insert("price".to_string(), self.price.clone());
        map.insert("category_id".to_string(), self.category_id.clone());
        map.insert("created_at".to_string(), self.created_at.clone());
        map.insert("updated_at".to_string(), self.updated_at.clone());
        map
    }

    fn from_map(map: HashMap<String, String>) -> Self {
        Self {
            id: map.get("id").cloned().unwrap_or_default(),
            title: map.get("title").cloned().unwrap_or_default(),
            price: map.get("price").cloned().unwrap_or_default(),
            category_id: map.get("category_id").cloned().unwrap_or_default(),
            created_at: map.get("created_at").cloned().unwrap_or_default(),
            updated_at: map.get("updated_at").cloned().unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct TestCategory {
    id: String,
    name: String,
    created_at: String,
    updated_at: String,
}

impl Model for TestCategory {
    fn table_name() -> &'static str {
        "test_categories"
    }

    fn get_id(&self) -> Option<&str> {
        Some(&self.id)
    }

    fn set_id(&mut self, id: String) {
        self.id = id;
    }

    fn to_map(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert("id".to_string(), self.id.clone());
        map.insert("name".to_string(), self.name.clone());
        map.insert("created_at".to_string(), self.created_at.clone());
        map.insert("updated_at".to_string(), self.updated_at.clone());
        map
    }

    fn from_map(map: HashMap<String, String>) -> Self {
        Self {
            id: map.get("id").cloned().unwrap_or_default(),
            name: map.get("name").cloned().unwrap_or_default(),
            created_at: map.get("created_at").cloned().unwrap_or_default(),
            updated_at: map.get("updated_at").cloned().unwrap_or_default(),
        }
    }
}

#[test]
fn test_token_issuance_and_abilities() {
    use terminus::prelude::*;

    let store = MemoryTokenStore::new();
    let user = User {
        id: "usr_100".to_string(),
        name: "Developer".to_string(),
        email: "dev@terminus.rs".to_string(),
        role: Role::Admin,
        status: UserStatus::Active,
        created_at: "2026-10-10".to_string(),
        ..Default::default()
    };

    // Issue token with specific abilities
    let new_token = user.create_token(&store, "cli-app", vec!["orders:read", "orders:create"]).unwrap();
    assert!(new_token.plain_text_token.contains('|'));

    // Verify abilities
    assert!(new_token.access_token.can("orders:read"));
    assert!(new_token.access_token.can("orders:create"));
    assert!(new_token.access_token.cant("orders:delete"));
    assert!(!new_token.access_token.can("users:manage"));

    // Find token by plain text secret
    let retrieved = store.find_token(&new_token.plain_text_token);
    assert!(retrieved.is_some());
    let token = retrieved.unwrap();
    assert_eq!(token.name, "cli-app");
    assert_eq!(token.tokenable_id, "usr_100");

    // Wildcard ability token
    let full_access = user.create_token(&store, "root-agent", vec!["*"]).unwrap();
    assert!(full_access.access_token.can("any:ability"));
    assert!(full_access.access_token.can("database:destroy"));

    // Expiration test
    let expired_token = user.create_token_with_expiry(
        &store,
        "temporary",
        vec!["*"],
        Some("2020-01-01T00:00:00Z".to_string()),
    ).unwrap();
    assert!(expired_token.access_token.is_expired());
    assert!(store.find_token(&expired_token.plain_text_token).is_none());
}

#[test]
fn test_sqlite_token_store() {
    use terminus::prelude::*;
    use std::sync::{Arc, Mutex};

    let conn = Arc::new(Mutex::new(rusqlite::Connection::open_in_memory().unwrap()));
    let store = SqliteTokenStore::new(conn).unwrap();

    let new_token = store.create_token("User", "u_42", "mobile-app", vec!["read".into(), "write".into()], None).unwrap();
    let plain_secret = &new_token.plain_text_token;

    let found = store.find_token(plain_secret);
    assert!(found.is_some());
    let pat = found.unwrap();
    assert_eq!(pat.name, "mobile-app");
    assert_eq!(pat.tokenable_id, "u_42");
    assert!(pat.can("read"));
    assert!(pat.can("write"));
    assert!(pat.cant("delete"));

    // Touch last used
    store.touch_last_used(&pat.id).unwrap();
    let list = store.list_for("User", "u_42");
    assert_eq!(list.len(), 1);
    assert!(list[0].last_used_at.is_some());

    // Revoke
    store.revoke_token(&pat.id).unwrap();
    assert!(store.find_token(plain_secret).is_none());
}

#[test]
fn test_orm_active_record_and_query_builder() {
    use terminus::prelude::*;

    let conn = rusqlite::Connection::open_in_memory().unwrap();

    // 1. Schema migration builder
    Schema::create(&conn, "test_categories", |table| {
        table.id();
        table.string("name");
        table.timestamps();
    }).unwrap();

    Schema::create(&conn, "test_products", |table| {
        table.id();
        table.string("title");
        table.string("price");
        table.string("category_id");
        table.timestamps();
    }).unwrap();

    // 2. Dynamic Model Active Record dirty tracking
    let mut dyn_model = DynamicModel::new("test_products");
    dyn_model.set("title", "Mechanical Keyboard");
    dyn_model.set("price", "150");
    dyn_model.set("category_id", "cat_1");

    assert!(dyn_model.is_dirty());
    assert_eq!(dyn_model.get_dirty().get("title").map(|s| s.as_str()), Some("Mechanical Keyboard"));

    dyn_model.save(&conn).unwrap();
    assert!(!dyn_model.is_dirty());
    assert!(dyn_model.get("id").is_some());

    // 3. Typed Model Active Record
    let mut cat_attrs = HashMap::new();
    cat_attrs.insert("id", "cat_1");
    cat_attrs.insert("name", "Hardware");
    TestCategory::create(cat_attrs, &conn).unwrap();

    let mut p1_attrs = HashMap::new();
    p1_attrs.insert("id", "prod_1");
    p1_attrs.insert("title", "Rust Programming");
    p1_attrs.insert("price", "45");
    p1_attrs.insert("category_id", "cat_1");
    TestProduct::create(p1_attrs, &conn).unwrap();

    let mut p2_attrs = HashMap::new();
    p2_attrs.insert("id", "prod_2");
    p2_attrs.insert("title", "Axum Deep Dive");
    p2_attrs.insert("price", "99");
    p2_attrs.insert("category_id", "cat_1");
    TestProduct::create(p2_attrs, &conn).unwrap();

    // Model::all
    let all_prods = TestProduct::all(&conn).unwrap();
    assert_eq!(all_prods.len(), 3); // 1 dynamic + 2 typed

    // Model::find
    let found = TestProduct::find("prod_1", &conn).unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().title, "Rust Programming");

    // ModelQuery builder
    let expensive = TestProduct::query()
        .where_eq("category_id", "cat_1")
        .where_gt("price", "40")
        .order_by("price", "desc")
        .get(&conn)
        .unwrap();
    assert!(expensive.len() >= 2);

    let count = TestProduct::query().where_eq("category_id", "cat_1").count(&conn).unwrap();
    assert_eq!(count, 3);

    let first = TestProduct::query().where_eq("id", "prod_2").first(&conn).unwrap();
    assert!(first.is_some());
    assert_eq!(first.unwrap().title, "Axum Deep Dive");

    // 4. HasMany and BelongsTo relationships
    let has_many_products = HasMany::<TestProduct>::new("cat_1", "category_id");
    let cat_products = has_many_products.get(&conn).unwrap();
    assert_eq!(cat_products.len(), 3);

    let belongs_to_cat = BelongsTo::<TestCategory>::new("cat_1", "id");
    let parent_cat = belongs_to_cat.get(&conn).unwrap();
    assert!(parent_cat.is_some());
    assert_eq!(parent_cat.unwrap().name, "Hardware");
}

#[test]
fn test_terminus_prompts_terminal_table_and_callouts() {
    use terminus::prelude::*;

    // Spin closure execution
    let computed = prompt::spin("Calculating database metrics", || {
        let mut sum = 0;
        for i in 1..=100 {
            sum += i;
        }
        sum
    });
    assert_eq!(computed, 5050);

    // Prompt table
    let headers = vec!["ID", "NAME", "ROLE"];
    let rows = vec![
        vec!["1".into(), "Alice".into(), "Admin".into()],
        vec!["2".into(), "Bob".into(), "Editor".into()],
    ];
    prompt::table(&headers, &rows);

    // Callout messages
    prompt::info("Terminus engine initialized");
    prompt::warning("Cache nearing capacity");
    prompt::error("Database connection timeout");
    prompt::note("Tip", "Run 'terminus migrate' to apply pending schemas");
}

#[tokio::test]
async fn test_enterprise_app_module_and_api_responses() {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use axum::routing::get;
    use terminus::prelude::*;

    // 1. ValidatedRequest
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct CreateInvoiceRequest {
        customer: String,
        amount: i64,
    }

    impl ValidatedRequest for CreateInvoiceRequest {
        fn validate(&self) -> Result<(), Vec<String>> {
            let mut errs = Vec::new();
            if self.customer.trim().is_empty() {
                errs.push("Customer cannot be empty".into());
            }
            if self.amount <= 0 {
                errs.push("Amount must be greater than zero".into());
            }
            if errs.is_empty() {
                Ok(())
            } else {
                Err(errs)
            }
        }
    }

    let bad_req = CreateInvoiceRequest {
        customer: "".into(),
        amount: -10,
    };
    assert!(bad_req.validate().is_err());
    assert_eq!(bad_req.validate().unwrap_err().len(), 2);

    let good_req = CreateInvoiceRequest {
        customer: "Acme Corp".into(),
        amount: 500,
    };
    assert!(good_req.validate().is_ok());

    // 2. ApiResponse envelope and HTTP status codes
    let ok_resp = ApiResponse::ok("invoice_created");
    assert_eq!(ok_resp.into_response().status(), StatusCode::OK);

    let err_resp = ApiResponse::error("Payment gateway failure");
    assert_eq!(err_resp.into_response().status(), StatusCode::BAD_REQUEST);

    let val_err_resp = ApiResponse::validation_errors(vec!["Invalid currency".into()]);
    assert_eq!(
        val_err_resp.into_response().status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // 3. AppModule and AdminPanel integration
    struct InvoiceResource;
    impl Resource for InvoiceResource {
        fn name(&self) -> &str {
            "Invoice"
        }
        fn plural_name(&self) -> &str {
            "Invoices"
        }
        fn slug(&self) -> &str {
            "invoices"
        }
        fn table(&self) -> Table {
            Table::new()
        }
        fn form(&self) -> Form {
            Form::new()
        }
        fn fetch_rows(&self, _q: &QueryState) -> (Vec<RowData>, usize) {
            (vec![], 0)
        }
    }

    struct BillingModule;
    impl AppModule for BillingModule {
        fn name(&self) -> &str {
            "Billing"
        }
        fn slug(&self) -> &str {
            "billing"
        }
        fn description(&self) -> Option<&str> {
            Some("Enterprise invoice and billing management")
        }
        fn routes(&self) -> axum::Router {
            axum::Router::new().route("/summary", get(|| async { ApiResponse::ok("summary_ok") }))
        }
        fn resource(&self) -> Option<std::sync::Arc<dyn Resource>> {
            Some(std::sync::Arc::new(InvoiceResource))
        }
    }

    let panel = AdminPanel::new().module(BillingModule);
    assert_eq!(panel.get_modules().len(), 1);
    assert_eq!(panel.get_modules()[0].name(), "Billing");
    assert_eq!(panel.get_modules()[0].slug(), "billing");

    // Router mounts both the admin UI routes and the nested /api/billing module routes
    let router = panel.into_router();
    let _ = router;

    // 4. DynamicModule programmatic builder
    let dyn_mod = DynamicModule::new("Shipping", "shipping")
        .with_description("Logistics tracking module")
        .with_routes(axum::Router::new().route(
            "/tracking",
            get(|| async { ApiResponse::ok("in_transit") }),
        ));

    assert_eq!(dyn_mod.name(), "Shipping");
    assert_eq!(dyn_mod.slug(), "shipping");
    assert_eq!(dyn_mod.description(), Some("Logistics tracking module"));
}

#[test]
fn test_sqlite_cache_persistence_and_ttl() {
    use std::sync::{Arc, Mutex};
    let conn = Arc::new(Mutex::new(rusqlite::Connection::open_in_memory().unwrap()));
    let cache = SqliteCache::from_arc(conn.clone()).unwrap();

    // Store value with 3600s TTL
    cache.put("user:session:1", "session_payload_data", Some(3600));
    assert_eq!(cache.get("user:session:1"), Some("session_payload_data".to_string()));

    // Store value with None (infinite) TTL
    cache.put("app:config:name", "Terminus", None);
    assert_eq!(cache.get("app:config:name"), Some("Terminus".to_string()));

    // Forget key
    let forgotten = cache.forget("user:session:1");
    assert!(forgotten);
    assert_eq!(cache.get("user:session:1"), None);

    // Flush entire cache
    cache.flush();
    assert_eq!(cache.get("app:config:name"), None);
}

#[test]
fn test_sqlite_job_queue_persistence_and_retries() {
    let queue = SqliteJobQueue::new(":memory:").unwrap();
    let stats0 = queue.stats();
    assert_eq!(stats0.pending, 0);

    // Push successful job
    let job_id = queue.push("send_invoice", r#"{"order_id": 9021}"#).unwrap();
    assert!(job_id > 0);
    assert_eq!(queue.stats().pending, 1);

    // Process job successfully
    let worked = queue.work_one(|name, payload| {
        assert_eq!(name, "send_invoice");
        assert!(payload.contains("9021"));
        Ok(())
    }).unwrap();
    assert_eq!(worked, Some(true));
    assert_eq!(queue.stats().pending, 0);
    assert_eq!(queue.stats().processed, 1);

    // Push failing job with max_attempts = 1
    queue.push_with_max_attempts("flaky_service", "{}", 1).unwrap();
    let failed = queue.work_one(|_, _| Err("Service unavailable".to_string())).unwrap();
    assert_eq!(failed, Some(false));
    assert_eq!(queue.stats().failed, 1);
}

#[test]
fn test_rich_form_fields_and_media_builders() {
    use terminus::form::{FieldType, Form, FormField};
    use terminus::table::{Column, ColumnType};

    let rt1 = FormField::rich_text("content");
    assert_eq!(rt1.field_type, FieldType::RichText);

    let img = FormField::image("cover");
    let gal = FormField::gallery("photos");
    assert_eq!(img.field_type, FieldType::Image);
    assert_eq!(gal.field_type, FieldType::Gallery);

    let barcode = FormField::barcode("sku");
    assert_eq!(barcode.field_type, FieldType::Barcode);

    let color = FormField::color("accent_color");
    assert_eq!(color.field_type, FieldType::Color);

    let kv1 = FormField::key_value("metadata");
    assert_eq!(kv1.field_type, FieldType::KeyValue);

    let col_img = Column::image("avatar");
    assert_eq!(col_img.name, "avatar");
    assert_eq!(col_img.column_type, ColumnType::Image);

    let form = Form::new()
        .field(rt1)
        .field(img)
        .field(barcode)
        .field(color);
    assert_eq!(form.fields.len(), 4);
}

#[test]
fn test_rich_fields_and_image_columns_rendering() {
    use terminus::form::{Form, FormField};
    use terminus::table::Column;

    struct ProductResource;
    impl terminus::resource::Resource for ProductResource {
        fn name(&self) -> &'static str { "Product" }
        fn plural_name(&self) -> &'static str { "Products" }
        fn slug(&self) -> &'static str { "products" }
        fn table(&self) -> terminus::table::Table {
            terminus::table::Table::new()
                .column(Column::text("title"))
                .column(Column::image("thumbnail"))
        }
        fn form(&self) -> Form {
            Form::new()
                .field(FormField::text("title"))
                .field(FormField::rich_text("description"))
                .field(FormField::image("thumbnail"))
                .field(FormField::barcode("sku"))
                .field(FormField::color("theme_color"))
        }
        fn fetch_rows(&self, _: &terminus::resource::QueryState) -> (Vec<terminus::resource::RowData>, usize) {
            (
                vec![
                    terminus::resource::RowData::new("1")
                        .insert("title", "Ergonomic Mechanical Keyboard")
                        .insert("thumbnail", "https://images.unsplash.com/photo-1587829741301-dc798b83add3")
                        .insert("sku", "KB-9002-RGB")
                        .insert("theme_color", "#3b82f6"),
                ],
                1,
            )
        }
    }

    let resource = ProductResource;
    let user = User::default_admin();

    // 1. Test table view rendering for ColumnType::Image
    let query = terminus::resource::QueryState::default();
    let table_html = terminus::view::table_view::render_table_partial(&resource, &query, &user);
    assert!(table_html.contains("img src=\"https://images.unsplash.com/photo-1587829741301-dc798b83add3\""));
    assert!(table_html.contains("alt=\"Thumbnail\""));

    // 2. Test dialog form rendering for rich fields
    let dialogs_html = terminus::view::dialogs::render_dialogs(&resource);
    assert!(dialogs_html.contains("formatRichText('create-field-description'"));
    assert!(dialogs_html.contains("title=\"Bold\">B</button>"));
    assert!(dialogs_html.contains("title=\"Code\">&lt;&gt;</button>"));
    assert!(dialogs_html.contains("updateImagePreview('create-field-thumbnail'"));
    assert!(dialogs_html.contains("placeholder=\"Scan barcode or enter SKU...\""));
    assert!(dialogs_html.contains("type=\"color\" id=\"picker-create-field-theme_color\""));

    // 3. Test full-page form rendering for rich fields with populated values
    let mut values = std::collections::HashMap::new();
    values.insert("title".to_string(), "Ergonomic Mechanical Keyboard".to_string());
    values.insert("description".to_string(), "## Specs\n- Hot-swappable switches".to_string());
    values.insert("thumbnail".to_string(), "https://images.unsplash.com/photo-1587829741301-dc798b83add3".to_string());
    values.insert("sku".to_string(), "KB-9002-RGB".to_string());
    values.insert("theme_color".to_string(), "#3b82f6".to_string());

    let page_html = terminus::view::form_page::render_form_page(&resource, Some("1"), Some(&values));
    assert!(page_html.contains("## Specs\n- Hot-swappable switches"));
    assert!(page_html.contains("formatRichText('page-field-description'"));
    assert!(page_html.contains("value=\"https://images.unsplash.com/photo-1587829741301-dc798b83add3\""));
    assert!(page_html.contains("value=\"KB-9002-RGB\""));
    assert!(page_html.contains("value=\"#3b82f6\""));
}

#[test]
fn test_global_helpers_and_ergonomics() {
    use chrono::Datelike;
    use terminus::prelude::*;

    // 1. Without authenticated user
    set_current_user(None);
    assert!(auth().is_none());
    assert!(user().is_none());
    assert!(!can("orders.view"));
    assert!(cannot("orders.view"));

    // 2. With with_user scoped helper
    let admin = User::new("u_99", "Sarah Admin", "sarah@example.com", Role::Admin);
    with_user(admin.clone(), || {
        assert!(auth().is_some());
        assert_eq!(user_id(), Some("u_99".to_string()));
        assert_eq!(user().unwrap().name, "Sarah Admin");
        assert!(can("orders.view"));
        assert!(cannot("unauthorized.action"));
        assert!(can_any(&["orders.view", "unknown"]));
        assert!(!can_all(&["orders.view", "unknown"]));
        assert!(is_admin());
        assert!(!is_superadmin());
    });

    // 3. Thread/Task context restoration
    assert!(auth().is_none());

    // 4. Request inspection helpers
    let req_info = RequestInfo::new("GET", "/admin/orders")
        .with_query("search", "Mechanical")
        .with_query("page", "2")
        .with_header("Accept", "application/json")
        .with_header("X-Requested-With", "XMLHttpRequest")
        .with_ip("192.168.1.50");

    with_request(req_info, || {
        assert_eq!(request().method(), "GET");
        assert_eq!(request().path(), "/admin/orders");
        assert_eq!(request().input("search"), Some("Mechanical".to_string()));
        assert_eq!(request().input("page"), Some("2".to_string()));
        assert!(request().has("search"));
        assert!(!request().has("missing_filter"));
        assert_eq!(request().ip(), "192.168.1.50");
        assert!(request().wants_json());
        assert!(request().is_ajax());
    });

    // 5. Abort flow control helpers
    assert!(abort_if(false, 403, "Forbidden").is_ok());
    assert!(abort_if(true, 403, "Forbidden").is_err());
    assert_eq!(abort_if(true, 403, "Access Denied").unwrap_err().status, 403);
    assert!(abort_unless(true, 404, "Not Found").is_ok());
    assert!(abort_unless(false, 404, "Not Found").is_err());
    assert_eq!(abort_404("Missing Record").unwrap_err().status, 404);
    assert_eq!(abort_403("Forbidden Area").unwrap_err().status, 403);

    // 6. DateTime and config utilities
    assert!(!today().is_empty());
    assert!(today().contains('-'));
    assert!(timestamp() > 1_700_000_000);
    let n = now();
    assert_eq!(n.year(), 2026);

    config_set("app.name", "Terminus Platform");
    assert_eq!(config("app.name"), Some("Terminus Platform".to_string()));
}

#[test]
fn test_app_bootstrap_and_routing() {
    use terminus::prelude::*;

    let app = App::new()
        .name("Terminus Shop")
        .port(8080)
        .host("0.0.0.0")
        .cors(true);

    assert_eq!(app.name, "Terminus Shop");
    assert_eq!(app.port, 8080);
    assert_eq!(app.host, "0.0.0.0");
    assert!(app.cors_enabled);

    let router = app.build();
    // Verify router builds without panic
    let _ = router;

    let res = json_response(serde_json::json!({"status": "ok", "app": "Terminus"}));
    assert_eq!(res.status(), axum::http::StatusCode::OK);

    let html = html_response("<h1>Welcome</h1>");
    assert!(html.0.contains("Welcome"));

    let red = redirect_to("/admin");
    let _ = red;
}

#[test]
fn test_advanced_filters_and_presets() {
    use terminus::prelude::*;
    use std::collections::HashMap;

    let filter_date = Filter::date_range("created_at").label("Creation Date");
    assert_eq!(filter_date.key, "created_at");
    assert_eq!(filter_date.label, "Creation Date");

    let filter_slider = Filter::range_slider("price", 0.0, 500.0, 10.0);
    assert_eq!(filter_slider.key, "price");

    let filter_multi = Filter::multi_select("status", vec![("paid", "Paid"), ("pending", "Pending")]);
    assert_eq!(filter_multi.key, "status");

    let preset = FilterPreset::make("vip_orders", "VIP High Spend Orders")
        .with("total_min", "500")
        .with("status", "paid");
    assert_eq!(preset.criteria.get("total_min"), Some(&"500".to_string()));

    // Test filter evaluation engine
    let mut row_values = HashMap::new();
    row_values.insert("price".to_string(), "150".to_string());
    row_values.insert("status".to_string(), "paid".to_string());
    row_values.insert("created_at".to_string(), "2026-05-10".to_string());

    let mut active_filters = HashMap::new();
    active_filters.insert("price_min".to_string(), "100".to_string());
    active_filters.insert("price_max".to_string(), "200".to_string());
    active_filters.insert("status".to_string(), "paid,shipped".to_string());
    active_filters.insert("created_at_from".to_string(), "2026-01-01".to_string());

    assert!(matches_filters(&row_values, &active_filters));

    // Test non-matching filter
    let mut non_matching = active_filters.clone();
    non_matching.insert("price_max".to_string(), "120".to_string());
    assert!(!matches_filters(&row_values, &non_matching));
}

#[test]
fn test_auth_user_extractor_and_helpers() {
    use terminus::prelude::*;

    let test_user = User {
        id: "usr_42".to_string(),
        name: "Pratik".to_string(),
        email: "pratik@example.com".to_string(),
        role: Role::Superadmin,
        status: UserStatus::Active,
        ..Default::default()
    };

    // Test with_user helper
    let res = with_user(test_user.clone(), || {
        let u = user().expect("User should be in thread context");
        assert_eq!(u.id, "usr_42");
        assert_eq!(u.name, "Pratik");
        assert!(is_superadmin());
        assert!(can("users.manage"));
        assert!(!cannot("users.manage"));
        "ok"
    });
    assert_eq!(res, "ok");

    // Outside task context
    assert!(user().is_none());
}

#[test]
fn test_strict_webhook_hmac_sha256_verification() {
    use terminus::prelude::*;

    // 1. OAuth Provider
    let github = OAuthProvider::GitHub {
        client_id: "gh_client_123".to_string(),
        client_secret: "gh_secret_456".to_string(),
    };
    let auth_url = github.authorization_url("https://terminus.dev/auth/callback", "state_abc");
    assert!(auth_url.contains("https://github.com/login/oauth/authorize"));
    assert!(auth_url.contains("client_id=gh_client_123"));
    assert!(auth_url.contains("state=state_abc"));

    // 2. Webhook Signature Verification with real HMAC-SHA256
    let raw_payload = r#"{"event":"order.completed","order_id":"ord_101"}"#;
    let secret = "whsec_live_top_secret_key";
    let computed_sig = WebhookVerifier::compute_signature(raw_payload, secret);

    // Direct hex signature matches
    assert!(WebhookVerifier::verify_signature(raw_payload, &computed_sig, secret));

    // sha256= prefix matches
    let sha256_sig = format!("sha256={}", computed_sig);
    assert!(WebhookVerifier::verify_signature(raw_payload, &sha256_sig, secret));

    // Timestamped Stripe/GitHub format: t=timestamp,v1=sig
    let timestamp = "1729000000";
    let signed_payload = format!("{}.{}", timestamp, raw_payload);
    let v1_sig = WebhookVerifier::compute_signature(&signed_payload, secret);
    let header = format!("t={},v1={}", timestamp, v1_sig);
    assert!(WebhookVerifier::verify_signature(raw_payload, &header, secret));

    // Rejection on invalid signature, secret, or tampering
    assert!(!WebhookVerifier::verify_signature(raw_payload, "invalid_sig", secret));
    assert!(!WebhookVerifier::verify_signature(raw_payload, &computed_sig, "wrong_secret"));
    assert!(!WebhookVerifier::verify_signature(r#"{"event":"tampered"}"#, &computed_sig, secret));
    assert!(!WebhookVerifier::verify_signature(raw_payload, "", secret));
    assert!(!WebhookVerifier::verify_signature(raw_payload, &computed_sig, ""));
}

#[test]
fn test_sortable_tables_and_draggable_blocks() {
    use terminus::prelude::*;

    // 1. Editable Columns & Sortable Rows in Table
    let table = Table::new()
        .column(Column::text("title").editable())
        .column(Column::text("status").editable_select(vec!["draft", "published", "archived"]))
        .sortable_rows();

    assert!(table.reorderable);
    assert!(table.columns[0].editable);
    assert!(table.columns[1].editable);
    assert_eq!(
        table.columns[1].editable_options,
        Some(vec!["draft".to_string(), "published".to_string(), "archived".to_string()])
    );

    // 2. Draggable UI blocks in Form
    let field = FormField::draggable_blocks("page_content", vec!["hero", "features", "pricing", "cta"]);
    match field.field_type {
        FieldType::DraggableBlocks { block_types } => {
            assert_eq!(block_types.len(), 4);
            assert_eq!(block_types[0], "hero");
            assert_eq!(block_types[2], "pricing");
        }
        _ => panic!("Expected DraggableBlocks"),
    }
}

#[test]
fn test_platform_adapters_and_sdk_generator() {
    use terminus::prelude::*;

    // 1. Platform detection from user-agent
    assert_eq!(Platform::from_user_agent("Mozilla/5.0 (Linux; Android 14)"), Platform::Mobile);
    assert_eq!(Platform::from_user_agent("Mozilla/5.0 (iPhone; CPU iPhone OS 17)"), Platform::Mobile);
    assert_eq!(Platform::from_user_agent("Tauri/2.0 Desktop"), Platform::Desktop);
    assert_eq!(Platform::from_user_agent("Electron/28.0 Desktop"), Platform::Desktop);
    assert_eq!(Platform::from_user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X)"), Platform::Web);

    // 2. Typed SDK Generator (TypeScript & OpenAPI 3.0)
    let gen = SdkGenerator::new("Acme Logistics", "1.0")
        .type_schema("Item", vec![("id", "string"), ("stock", "number")])
        .endpoint("GET", "/items", None, "Item[]", "Fetch items")
        .endpoint("POST", "/items", Some("Item"), "Item", "Create item");

    let ts = gen.to_typescript();
    assert!(ts.contains("export interface Item"));
    assert!(ts.contains("export class AcmeLogisticsClient"));
    assert!(ts.contains("async getItems(): Promise<Item[]>"));

    let openapi = gen.to_openapi_json();
    assert_eq!(openapi["openapi"], "3.0.3");
    assert_eq!(openapi["info"]["title"], "Acme Logistics");
}

#[tokio::test]
async fn test_abort_error_into_response_and_constructors() {
    use terminus::prelude::*;
    use axum::response::IntoResponse;

    // 1. Test convenience constructors
    let err_forbidden = AbortError::forbidden("Access Denied");
    assert_eq!(err_forbidden.status, 403);
    assert_eq!(err_forbidden.message, "Access Denied");

    let err_404 = AbortError::not_found("Resource missing");
    assert_eq!(err_404.status, 404);

    let err_bad = AbortError::bad_request("Invalid input");
    assert_eq!(err_bad.status, 400);

    // 2. Test IntoResponse with JSON client
    let req_json = RequestInfo::new("GET", "/api/secure")
        .with_header("Accept", "application/json");
    let resp_json = with_request(req_json, || {
        err_forbidden.clone().into_response()
    });
    assert_eq!(resp_json.status(), axum::http::StatusCode::FORBIDDEN);

    // 3. Test IntoResponse with HTML browser client
    let req_html = RequestInfo::new("GET", "/admin/secure")
        .with_header("Accept", "text/html");
    let resp_html = with_request(req_html, || {
        err_forbidden.into_response()
    });
    assert_eq!(resp_html.status(), axum::http::StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_app_fluent_group_static_and_openapi() {
    use terminus::prelude::*;

    let sdk_gen = SdkGenerator::new("Acme API", "1.0.0")
        .endpoint("GET", "/api/v1/ping", None, "string", "Ping service");

    let app = App::new()
        .port(4000)
        .group("/api/v1", |group| {
            group
                .get("/ping", || async { "pong" })
                .post("/echo", || async { "echo" })
        })
        .static_dir("/assets", "./")
        .openapi("/api/openapi.json", &sdk_gen);

    let router = app.build();
    let _ = router;
}

#[test]
fn test_typescript_sdk_and_openapi_generation() {
    use terminus::prelude::*;

    let gen = SdkGenerator::new("Terminus Store", "2.0.0")
        .type_schema("Product", vec![
            ("id", "string"),
            ("title", "string"),
            ("price", "number"),
            ("inStock", "boolean"),
        ])
        .endpoint("GET", "/api/products", None, "Product[]", "Fetch product catalog")
        .endpoint("POST", "/api/checkout", Some("Product"), "Product", "Place order");

    // 1. TypeScript SDK Verification
    let ts = gen.to_typescript();
    assert!(ts.contains("export interface Product {"));
    assert!(ts.contains("title: string;"));
    assert!(ts.contains("price: number;"));
    assert!(ts.contains("inStock: boolean;"));
    assert!(ts.contains("export class TerminusStoreClient"));

    // 2. OpenAPI Specification Verification
    let openapi = gen.to_openapi_json();
    assert_eq!(openapi["openapi"], "3.0.3");
    assert_eq!(openapi["info"]["title"], "Terminus Store");
    assert!(openapi["paths"]["/api/products"]["get"].is_object());
    assert!(openapi["paths"]["/api/checkout"]["post"]["requestBody"].is_object());
    assert!(openapi["components"]["schemas"]["Product"].is_object());
}

#[test]
fn test_blueprint_types_and_database_manager() {
    use terminus::prelude::*;

    let db = Database::memory().expect("in-memory db initialization failed");

    // 1. Create table with comprehensive Blueprint column types
    db.with_conn(|conn| {
        Schema::create(conn, "test_records", |table| {
            table.id();
            table.string("title");
            table.text("body");
            table.nullable_text("notes");
            table.integer("views");
            table.boolean("published");
            table.float("rating");
            table.decimal("price");
            table.datetime("published_at");
            table.json("metadata");
            table.timestamps();
        }).unwrap();
    });

    // 2. Insert and query using Database helper
    db.execute("INSERT INTO test_records (id, title, body, views, published, rating, price, published_at, metadata, created_at, updated_at) VALUES ('1', 'Intro', 'Content', 10, 1, 4.8, 19.99, '2026-10-12', '{}', '2026-10-12', '2026-10-12')").unwrap();

    let count = db.with_conn(|conn| {
        conn.query_row("SELECT COUNT(1) FROM test_records WHERE title = ?1", rusqlite::params!["Intro"], |r| r.get::<_, usize>(0)).unwrap()
    });
    assert_eq!(count, 1);
}

#[test]
fn test_enhanced_testing_assertions() {
    use terminus::prelude::*;

    expect(42).to_equal(42);
    expect("hello").to_equal("hello");
    expect(10).to_not_equal(20);

    let items = vec!["a", "b", "c"];
    expect(items).to_have_length(3);
}

#[test]
fn test_project_scaffolding_presets() {
    use terminus::prelude::*;

    let temp_base = std::env::temp_dir().join(format!("terminus_scaffold_test_{}", std::process::id()));

    // 1. Fullstack preset
    let app1_dir = temp_base.join("app1_fullstack");
    let config1 = ProjectConfig::new("app1_fullstack")
        .preset(ProjectPreset::Fullstack)
        .database(DatabaseChoice::Sqlite)
        .port(3000);

    let files1 = scaffold_project(&app1_dir, &config1).unwrap();
    assert!(files1.iter().any(|f| f.ends_with("Cargo.toml")));
    assert!(files1.iter().any(|f| f.ends_with("src/main.rs")));
    assert!(files1.iter().any(|f| f.ends_with(".env.example")));
    assert!(files1.iter().any(|f| f.ends_with("README.md")));
    assert!(files1.iter().any(|f| f.ends_with("migrations/001_create_users_table.sql")));
    assert!(files1.iter().any(|f| f.ends_with("public/index.html")));

    let cargo1 = std::fs::read_to_string(app1_dir.join("Cargo.toml")).unwrap();
    assert!(cargo1.contains("name = \"app1_fullstack\""));
    assert!(cargo1.contains("terminus-framework"));

    let main1 = std::fs::read_to_string(app1_dir.join("src/main.rs")).unwrap();
    assert!(main1.contains("Database::open(\"app1_fullstack.db\")"));
    assert!(main1.contains("Terminus Fullstack Application"));

    let env1 = std::fs::read_to_string(app1_dir.join(".env.example")).unwrap();
    assert!(env1.contains("DATABASE_URL=sqlite://app1_fullstack.db"));

    let mig1 = std::fs::read_to_string(app1_dir.join("migrations/001_create_users_table.sql")).unwrap();
    assert!(mig1.contains("CREATE TABLE IF NOT EXISTS users"));

    // 2. Headless Backend API preset
    let app2_dir = temp_base.join("app2_backend");
    let config2 = ProjectConfig::new("app2_backend")
        .preset(ProjectPreset::Backend)
        .database(DatabaseChoice::Sqlite)
        .port(8000);

    let files2 = scaffold_project(&app2_dir, &config2).unwrap();
    assert!(files2.iter().any(|f| f.ends_with("Cargo.toml")));
    assert!(files2.iter().any(|f| f.ends_with("src/main.rs")));
    assert!(files2.iter().any(|f| f.ends_with(".env.example")));
    assert!(files2.iter().any(|f| f.ends_with("migrations/001_create_users_table.sql")));

    let main2 = std::fs::read_to_string(app2_dir.join("src/main.rs")).unwrap();
    assert!(main2.contains("Terminus Headless API"));
    assert!(main2.contains("SdkGenerator::new(\"app2_backend\""));
    assert!(main2.contains("openapi(\"/api/openapi.json\""));

    // 3. Targets and Presets
    assert_eq!(parse_targets("web"), vec![TargetPlatform::Web]);
    assert_eq!(parse_targets("any"), vec![TargetPlatform::Web]);
    assert_eq!(ProjectPreset::from_str_loose("fullstack"), Some(ProjectPreset::Fullstack));
    assert_eq!(ProjectPreset::from_str_loose("backend"), Some(ProjectPreset::Backend));

    // Clean up temporary directory
    let _ = std::fs::remove_dir_all(&temp_base);
}

#[test]
fn test_type_safe_validator_and_validation_errors() {
    use terminus::prelude::*;

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    struct OrderRequest {
        customer_email: String,
        amount: f64,
        shipping_country: Option<String>,
        is_rush: bool,
    }

    // 1. Successful validation
    let valid_req = OrderRequest {
        customer_email: "orders@company.com".to_string(),
        amount: 250.0,
        shipping_country: Some("US".to_string()),
        is_rush: false,
    };

    let result = Validator::new(&valid_req)
        .rule("customer_email", |r| !r.customer_email.is_empty(), "Email is required")
        .rule("customer_email", |r| r.customer_email.contains('@'), "Valid email required")
        .rule("amount", |r| r.amount > 0.0, "Amount must be greater than zero")
        .rule_when(|r| r.is_rush, "amount", |r| r.amount >= 50.0, "Rush orders require min $50")
        .validate();

    assert!(result.is_ok());

    // 2. Failed validation with multiple rules
    let invalid_req = OrderRequest {
        customer_email: "invalid-email".to_string(),
        amount: -10.0,
        shipping_country: None,
        is_rush: true,
    };

    let err_result = Validator::new(&invalid_req)
        .rule("customer_email", |r| !r.customer_email.is_empty(), "Email is required")
        .rule("customer_email", |r| r.customer_email.contains('@'), "Valid email required")
        .rule("amount", |r| r.amount > 0.0, "Amount must be greater than zero")
        .rule_when(|r| r.is_rush, "amount", |r| r.amount >= 50.0, "Rush orders require min $50")
        .validate();

    assert!(err_result.is_err());
    let errors = err_result.err().unwrap();
    assert_eq!(errors.message, "Validation failed");
    assert!(errors.has_errors_for("customer_email"));
    assert_eq!(errors.first("customer_email"), Some("Valid email required"));
    assert!(errors.has_errors_for("amount"));
    assert_eq!(errors.errors.get("amount").unwrap().len(), 2); // negative and rush condition

    // 3. JsonValidator on untyped payloads
    let raw_json = serde_json::json!({
        "name": "Widget A",
        "email": "test@domain.com",
        "price": 19.99
    });

    let mut jv = JsonValidator::new(&raw_json);
    let name = jv.require_str("name");
    let email = jv.require_email("email");
    let price = jv.require_number("price");
    assert_eq!(name, Some("Widget A"));
    assert_eq!(email, Some("test@domain.com"));
    assert_eq!(price, Some(19.99));
    assert!(jv.validate().is_ok());

    let invalid_json = serde_json::json!({
        "name": "",
        "email": "not-an-email",
        "price": "not-a-number"
    });

    let mut bad_jv = JsonValidator::new(&invalid_json);
    bad_jv.require_str("name");
    bad_jv.require_email("email");
    bad_jv.require_number("price");
    let bad_res = bad_jv.validate();
    assert!(bad_res.is_err());
    let j_errors = bad_res.err().unwrap();
    assert!(j_errors.has_errors_for("name"));
    assert!(j_errors.has_errors_for("email"));
    assert!(j_errors.has_errors_for("price"));
}

#[test]
fn test_agent_schema_introspection_and_app_spec() {
    use terminus::prelude::*;

    // 1. Direct AppSpec construction
    let mut spec = AppSpec::new("Warehouse Master");
    spec.set_targets(vec!["web".to_string(), "desktop".to_string(), "iot".to_string()]);

    spec.add_route(RouteSpec::new("GET", "/api/items", "list_items").with_auth(true));
    spec.add_route(RouteSpec::new("POST", "/api/items", "create_item"));

    let item_model = ModelSpec::new("Item", "items")
        .add_field(FieldSpec::new("id", "String", "TEXT").primary())
        .add_field(FieldSpec::new("sku", "String", "TEXT"))
        .add_field(FieldSpec::new("quantity", "i64", "INTEGER"))
        .add_field(FieldSpec::new("notes", "String", "TEXT").nullable());
    spec.add_model(item_model);

    let res = ResourceSpec::new("Item", "items", "items")
        .with_searchable(&["sku", "name"])
        .with_sortable(&["quantity", "created_at"]);
    spec.add_resource(res);

    assert_eq!(spec.app_name, "Warehouse Master");
    assert_eq!(spec.targets.len(), 3);
    assert_eq!(spec.routes.len(), 2);
    assert_eq!(spec.models.len(), 1);
    assert_eq!(spec.models[0].fields.len(), 4);
    assert_eq!(spec.resources.len(), 1);

    // Serialization for AI agents
    let json = spec.to_json_pretty();
    assert!(json.contains("\"framework\": \"terminus-framework\""));
    assert!(json.contains("\"app_name\": \"Warehouse Master\""));
    assert!(json.contains("\"sku\""));

    let ascii_table = spec.to_ascii_summary();
    assert!(ascii_table.contains("TERMINUS APPLICATION TOPOLOGY"));
    assert!(ascii_table.contains("[ROUTES]"));
    assert!(ascii_table.contains("/api/items"));
    assert!(ascii_table.contains("[MODELS & TABLES]"));
    assert!(ascii_table.contains("[ADMIN RESOURCES]"));

    // 2. Integration with App engine
    let app = App::new()
        .name("Logistics API")
        .port(4000)
        .get("/api/v1/health", || async { "OK" })
        .post("/api/v1/shipments", || async { "Created" })
        .model(ModelSpec::new("Shipment", "shipments"))
        .spec_endpoint("/spec");

    assert_eq!(app.spec().app_name, "Logistics API");
    assert_eq!(app.spec().port, 4000);
    assert!(app.spec().routes.iter().any(|r| r.path == "/api/v1/health" && r.method == "GET"));
    assert!(app.spec().routes.iter().any(|r| r.path == "/api/v1/shipments" && r.method == "POST"));
    assert_eq!(app.spec().models.len(), 1);
}

#[tokio::test]
async fn test_headless_test_client_and_http_assertions() {
    let app = App::new()
        .name("Test Commerce")
        .get("/api/health", || async {
            json_response(serde_json::json!({ "status": "healthy", "service": "commerce" }))
        })
        .post("/api/items", |axum::Json(payload): axum::Json<serde_json::Value>| async move {
            let mut jv = JsonValidator::new(&payload);
            jv.require_str("name");
            jv.require_number("price");
            match jv.validate() {
                Ok(_) => (
                    axum::http::StatusCode::CREATED,
                    [("Content-Type", "application/json")],
                    serde_json::json!({ "id": "item_123", "status": "created" }).to_string(),
                )
                    .into_response(),
                Err(errors) => errors.into_response(),
            }
        })
        .spec_endpoint("/api/_spec");

    let client = TestClient::new(app);

    // 1. GET /api/health
    let res = client.get("/api/health").await;
    res.assert_ok()
        .assert_contains("healthy");
    assert_eq!(res.json()["status"], "healthy");
    assert_eq!(res.json()["service"], "commerce");

    // 2. GET /api/_spec (machine introspection)
    let spec_res = client.get("/api/_spec").await;
    spec_res.assert_ok();
    assert_eq!(spec_res.json()["framework"], "terminus-framework");
    assert_eq!(spec_res.json()["app_name"], "Test Commerce");

    // 3. POST /api/items (validation failure -> 422)
    let invalid_res = client
        .post("/api/items", serde_json::json!({ "name": "" }))
        .await;
    invalid_res.assert_unprocessable();
    assert_eq!(invalid_res.json()["message"], "Validation failed");
    assert!(invalid_res.json()["errors"]["price"].is_array());

    // 4. POST /api/items (validation success -> 201)
    let valid_res = client
        .post(
            "/api/items",
            serde_json::json!({ "name": "Laptop", "price": 999.99 }),
        )
        .await;
    valid_res.assert_created();
    assert_eq!(valid_res.json()["status"], "created");
    assert_eq!(valid_res.json()["id"], "item_123");

    // 5. GET 404 Not Found
    let missing_res = client.get("/api/nonexistent").await;
    missing_res.assert_not_found();
}

#[test]
fn test_pure_rust_pbkdf2_hash_and_cryptographic_primitives() {
    let password = "super_secret_master_password_2026";
    let hash = Hash::make(password);

    assert!(hash.starts_with("$pbkdf2-sha256$i=10000$"));
    assert!(Hash::check(password, &hash));
    assert!(!Hash::check("wrong_password", &hash));
    assert!(!Hash::check("", &hash));

    // Test with specific salt and iterations
    let custom_hash = Hash::make_with_salt("admin123", "fixed_salt_123", 5_000);
    assert_eq!(custom_hash, Hash::make_with_salt("admin123", "fixed_salt_123", 5_000));
    assert!(Hash::check("admin123", &custom_hash));
    assert!(!Hash::check("admin124", &custom_hash));

    // Test constant-time equality
    assert!(terminus::token::constant_time_eq_str("hello", "hello"));
    assert!(!terminus::token::constant_time_eq_str("hello", "world"));
    assert!(!terminus::token::constant_time_eq_str("hello", "hell"));
}

#[test]
fn test_cryptographic_webhook_hmac_sha256_verification() {
    let payload = r#"{"event":"payment.succeeded","amount":9900}"#;
    let secret = "whsec_terminus_production_secret_key_8892";

    // 1. Raw HMAC-SHA256 hex
    let signature = WebhookVerifier::compute_signature(payload, secret);
    assert!(WebhookVerifier::verify_signature(payload, &signature, secret));

    // 2. GitHub-style prefix `sha256=<hex>`
    let gh_sig = format!("sha256={}", signature);
    assert!(WebhookVerifier::verify_signature(payload, &gh_sig, secret));

    // 3. Stripe-style timestamped format `t=...,v1=...`
    let ts = "1728900000";
    let signed_payload = format!("{}.{}", ts, payload);
    let v1_sig = WebhookVerifier::compute_signature(&signed_payload, secret);
    let stripe_sig = format!("t={},v1={}", ts, v1_sig);
    assert!(WebhookVerifier::verify_signature(payload, &stripe_sig, secret));

    // 4. Tampered payload or wrong secret must fail
    assert!(!WebhookVerifier::verify_signature(r#"{"tampered":true}"#, &signature, secret));
    assert!(!WebhookVerifier::verify_signature(payload, &signature, "wrong_secret"));
    assert!(!WebhookVerifier::verify_signature(payload, "invalid_sig", secret));
    assert!(!WebhookVerifier::verify_signature("", &signature, secret));
}

#[test]
fn test_transactional_email_html_and_text_rendering() {
    let msg = MailMessage::to("founder@company.com")
        .subject("Your Terminus Cluster is Ready")
        .line("Welcome to Terminus! Your high-performance cluster has been provisioned.")
        .line("You can now connect to your admin dashboard and manage your application topology.")
        .action("Access Dashboard", "https://app.terminus.dev/admin");

    let html = msg.render_html();
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("Your Terminus Cluster is Ready"));
    assert!(html.contains("founder@company.com"));
    assert!(html.contains("Access Dashboard"));
    assert!(html.contains("https://app.terminus.dev/admin"));

    let text = msg.render_text();
    assert!(text.contains("SUBJECT: Your Terminus Cluster is Ready"));
    assert!(text.contains("TO: founder@company.com"));
    assert!(text.contains("Access Dashboard: https://app.terminus.dev/admin"));
    assert!(text.contains("Sent via Terminus Framework"));
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct NewsletterSubscriber {
    email: String,
    channel: Option<String>,
}

impl Validate for NewsletterSubscriber {
    fn validate(&self) -> std::result::Result<(), ValidationErrors> {
        let mut errors = ValidationErrors::new();
        if !self.email.contains('@') || !self.email.contains('.') {
            errors.add("email", "A valid email address with domain is required");
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[tokio::test]
async fn test_valid_extractor_and_redirect_controller() {
    // 1. Setup in-memory SQLite schema
    let db = Database::memory().unwrap();
    db.with_conn(|conn| {
        conn.execute(
            "CREATE TABLE subscribers (id INTEGER PRIMARY KEY AUTOINCREMENT, email TEXT NOT NULL UNIQUE)",
            [],
        ).unwrap();
    });
    Database::set_global(db);

    // 2. Define controller handler function with fluent validation ergonomics
    async fn subscribe_handler(Valid(subscriber): Valid<NewsletterSubscriber>) -> Result<Redirect> {
        db::query!("INSERT INTO subscribers (email) VALUES (?)", subscriber.email)
            .insert()
            .await?;

        // Idiomatic controller response returning Result<Redirect>
        Ok(redirect("/subscribe/thank-you"))
    }

    let app = App::new()
        .name("Newsletter Portal")
        .post("/subscribe", subscribe_handler);

    let client = TestClient::new(app);

    // 3. Test valid subscriber submission -> HTTP 303 See Other Redirect to /subscribe/thank-you
    let res = client
        .post("/subscribe", serde_json::json!({
            "email": "subscriber@example.com",
            "channel": "rust-preview"
        }))
        .await;

    assert_eq!(res.status(), axum::http::StatusCode::SEE_OTHER);
    assert_eq!(
        res.header("location"),
        Some("/subscribe/thank-you")
    );

    // Verify row was inserted into database via db::query!
    let row = db::query!("SELECT email FROM subscribers WHERE email = ?", "subscriber@example.com")
        .fetch_one()
        .await
        .unwrap();
    assert!(row.is_some());
    assert_eq!(row.unwrap().get("email").unwrap(), "subscriber@example.com");

    // 4. Test invalid subscriber submission -> HTTP 422 Unprocessable Entity
    let bad_res = client
        .post("/subscribe", serde_json::json!({
            "email": "invalid-email-address",
            "channel": "rust-preview"
        }))
        .await;

    assert_eq!(bad_res.status(), axum::http::StatusCode::UNPROCESSABLE_ENTITY);
    let error_body = bad_res.text();
    assert!(error_body.contains("Validation failed"));
    assert!(error_body.contains("A valid email address with domain is required"));
}

#[tokio::test]
async fn test_db_query_macro_and_dialect_compilation() {
    // 1. Parameterized SQLite query compilation
    let query = SqlQuery::new("SELECT * FROM subscribers WHERE email = ? AND status = ?");
    assert_eq!(
        query.compiled_sql(),
        "SELECT * FROM subscribers WHERE email = ? AND status = ?"
    );

    // 2. Execution on SQLite database via macro
    let db = Database::memory().unwrap();
    db.with_conn(|conn| {
        conn.execute(
            "CREATE TABLE products (id TEXT PRIMARY KEY, name TEXT, price REAL)",
            [],
        ).unwrap();
    });
    Database::set_global(db);

    let inserted_id = db::query!(
        "INSERT INTO products (id, name, price) VALUES (?, ?, ?)",
        "p_101",
        "Ergonomic Keyboard",
        199.99
    )
    .insert()
    .await
    .unwrap();
    assert!(inserted_id > 0);

    let item = db::query!("SELECT id, name, price FROM products WHERE id = ?", "p_101")
        .fetch_one()
        .await
        .unwrap();
    assert!(item.is_some());
    let map = item.unwrap();
    assert_eq!(map.get("name").unwrap(), "Ergonomic Keyboard");

    let count = db::query!("UPDATE products SET price = ? WHERE id = ?", 179.99, "p_101")
        .execute()
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn test_sqlite_embedded_database_engine_and_persistence() {
    use terminus::prelude::*;

    // 1. In-memory SQLite connection
    let db = Database::memory().unwrap();
    db.execute(
        "CREATE TABLE test_users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT NOT NULL UNIQUE,
            email TEXT NOT NULL
        )",
    ).unwrap();

    // 2. Parameterized insertion
    let affected = db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO test_users (username, email) VALUES (?1, ?2)",
            rusqlite::params!["pratik", "pratik@example.com"],
        )
    }).unwrap();
    assert_eq!(affected, 1);

    // 3. Query execution
    db.with_conn(|conn| {
        let mut stmt = conn.prepare("SELECT username, email FROM test_users WHERE username = ?1").unwrap();
        let mut rows = stmt.query(rusqlite::params!["pratik"]).unwrap();
        let row = rows.next().unwrap().unwrap();
        let user: String = row.get(0).unwrap();
        let mail: String = row.get(1).unwrap();
        assert_eq!(user, "pratik");
        assert_eq!(mail, "pratik@example.com");
    });

    // 4. URL parsing for SQLite
    let cfg1 = DatabaseConfig::from_url("sqlite://custom.db").unwrap();
    assert_eq!(cfg1.database, "custom.db");

    let cfg2 = DatabaseConfig::from_url(":memory:").unwrap();
    assert_eq!(cfg2.database, ":memory:");
}

#[test]
fn test_rfc2104_hmac_sha256_signed_session_cookies() {
    use terminus::prelude::*;

    let secret = b"super_secure_production_secret_key_32_bytes";
    let session = Session::new();
    session.set("account_id", "acc_8892");
    session.set("role", "admin");

    // 1. Generate signed cookie header
    let cookie_header = session.signed_cookie_header(secret);
    assert!(cookie_header.starts_with("terminus_session="));
    assert!(cookie_header.contains("HttpOnly"));
    assert!(cookie_header.contains("SameSite=Lax"));

    // Extract cookie value
    let val = cookie_header
        .split(';')
        .next()
        .unwrap()
        .trim_start_matches("terminus_session=");

    // 2. Successful verification with correct secret
    let verified_id = Session::verify_signed_cookie(val, secret);
    assert_eq!(verified_id, Some(session.id().to_string()));

    // 3. Rejection with wrong secret
    let wrong_secret = b"wrong_secret_key_that_should_fail_verification";
    assert_eq!(Session::verify_signed_cookie(val, wrong_secret), None);

    // 4. Rejection with tampered session payload
    let tampered_cookie = format!("tampered_{}", val);
    assert_eq!(Session::verify_signed_cookie(&tampered_cookie, secret), None);

    // 5. Rejection with tampered signature
    if let Some((sid, _sig)) = val.split_once('.') {
        let tampered_sig = format!("{}.0000000000000000000000000000000000000000000000000000000000000000", sid);
        assert_eq!(Session::verify_signed_cookie(&tampered_sig, secret), None);
    }
}

#[test]
fn test_configurable_port_presets_and_app_spec() {
    let app_3000 = App::port_3000().name("Frontend API");
    assert_eq!(app_3000.spec().port, 3000);

    let app_8000 = App::port_8000().name("Classic Backend");
    assert_eq!(app_8000.spec().port, 8000);

    let app_8080 = App::port_8080().name("Microservice Gateway");
    assert_eq!(app_8080.spec().port, 8080);

    let custom_app = App::new().port(9090);
    assert_eq!(custom_app.spec().port, 9090);
}

#[test]
fn test_session_flash_bag_and_lifecycle() {
    let session = Session::new();
    assert!(!session.id().is_empty());

    // 1. Basic session store & get
    session.set("user_id", "usr_123");
    assert_eq!(session.get("user_id").as_deref(), Some("usr_123"));

    // 2. CSRF Token verification
    let token = session.csrf_token();
    assert_eq!(token.len(), 32);
    assert!(session.verify_csrf_token(&token));
    assert!(!session.verify_csrf_token("invalid_token"));

    // 3. Flash messages: present on first read, swept on subsequent cycle
    session.flash("success", "Profile updated successfully!");
    assert_eq!(session.get_flash("success").as_deref(), Some("Profile updated successfully!"));
    assert!(session.has_flash("success"));

    // Sweep flashes (as happens at end of HTTP response lifecycle)
    session.sweep_flashes();
    assert!(!session.has_flash("success"));
    assert_eq!(session.get_flash("success"), None);

    // 4. Cookie header format
    let cookie = session.cookie_header();
    assert!(cookie.starts_with("terminus_session="));
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Lax"));
}

#[test]
fn test_redirect_with_flash_chaining() {
    let r1 = redirect("/dashboard").with_success("Order placed!");
    assert_eq!(r1.url, "/dashboard");
    assert_eq!(r1.flash, Some(("success".to_string(), "Order placed!".to_string())));

    let r2 = redirect("/login").with_error("Invalid credentials.");
    assert_eq!(r2.url, "/login");
    assert_eq!(r2.flash, Some(("error".to_string(), "Invalid credentials.".to_string())));

    let r3 = redirect("/profile").with_warning("Email unverified.");
    assert_eq!(r3.url, "/profile");
    assert_eq!(r3.flash, Some(("warning".to_string(), "Email unverified.".to_string())));
}

#[test]
fn test_blade_style_view_engine_rendering() {
    // Session with flash message
    let session = Session::new();
    session.flash("success", "Operation completed!");
    set_current_session(Some(session));

    let v = view("Dashboard Portal", "<p class=\"text-zinc-600\">Welcome to Terminus Web</p>");

    let html = v.render();
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("<title>Dashboard Portal - Terminus</title>"));
    assert!(html.contains("Operation completed!"));
    assert!(html.contains("bg-emerald-950/60 border-emerald-800/80 text-emerald-200"));
    assert!(html.contains("SUCCESS"));
    assert!(html.contains("Welcome to Terminus Web"));
    assert!(html.contains("name=\"csrf-token\""));
}

#[test]
fn test_active_record_pagination_and_links_html() {
    let items = vec!["Item 1", "Item 2", "Item 3", "Item 4", "Item 5"];
    let paginated = Paginated::new(items, 23, 1, 5);

    assert_eq!(paginated.total, 23);
    assert_eq!(paginated.page, 1);
    assert_eq!(paginated.per_page, 5);
    assert_eq!(paginated.total_pages, 5);
    assert_eq!(paginated.from_index(), 1);
    assert_eq!(paginated.to_index(), 5);
    assert!(paginated.has_next());
    assert!(!paginated.has_prev());

    let links = paginated.links_html("/products");
    assert!(links.contains("Showing <span class=\"font-semibold text-zinc-200\">1</span> to <span class=\"font-semibold text-zinc-200\">5</span> of <span class=\"font-semibold text-zinc-200\">23</span> results"));
    assert!(links.contains("href=\"/products?page=2\""));
}

#[test]
fn test_database_seeder_registry() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static SEED_RUN_COUNT: AtomicUsize = AtomicUsize::new(0);

    struct TestUserSeeder;

    impl Seeder for TestUserSeeder {
        fn name(&self) -> &'static str {
            "TestUserSeeder"
        }

        fn run(&self, _db: &terminus::orm::Database) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            SEED_RUN_COUNT.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    let mut registry = SeederRegistry::new();
    registry.register(TestUserSeeder);
    assert_eq!(registry.len(), 1);

    let db = terminus::orm::Database::memory().unwrap();
    let res = registry.run_all(&db);
    assert!(res.is_ok());
    assert_eq!(SEED_RUN_COUNT.load(Ordering::SeqCst), 1);
}

#[test]
fn test_task_scheduler_and_frequency() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let flag = Arc::new(AtomicBool::new(false));
    let flag_clone = Arc::clone(&flag);

    let schedule = Schedule::new();
    schedule.call("cleanup:temp", move || {
        flag_clone.store(true, Ordering::SeqCst);
    }).every_minute();

    assert_eq!(schedule.tasks().len(), 1);
    assert_eq!(schedule.tasks()[0].name, "cleanup:temp");

    let executed = schedule.run_due();
    assert_eq!(executed.len(), 1);
    assert!(flag.load(Ordering::SeqCst));
}

#[tokio::test]
async fn test_session_axum_extractor_and_cookie_resolution() {
    use terminus::prelude::*;
    use axum::extract::FromRequestParts;
    use axum::http::Request;

    // 1. Session extractor with no cookies generates a fresh active session
    let req = Request::builder()
        .uri("/dashboard")
        .body(())
        .unwrap();
    let (mut parts, _) = req.into_parts();
    let session = Session::from_request_parts(&mut parts, &()).await.unwrap();
    assert!(!session.id().is_empty());
    session.set("user_email", "operator@terminus.dev");

    // 2. Subsequent request with the session cookie recovers identical session state
    let cookie_header = session.cookie_header();
    let req2 = Request::builder()
        .uri("/settings")
        .header(axum::http::header::COOKIE, cookie_header)
        .body(())
        .unwrap();
    let (mut parts2, _) = req2.into_parts();
    let recovered_session = Session::from_request_parts(&mut parts2, &()).await.unwrap();
    assert_eq!(recovered_session.id(), session.id());
    assert_eq!(recovered_session.get("user_email").as_deref(), Some("operator@terminus.dev"));
}

#[test]
fn test_csrf_field_and_token_helpers() {
    use terminus::prelude::*;

    let token = csrf_token();
    assert_eq!(token.len(), 32);

    let field = csrf_field();
    assert!(field.starts_with("<input type=\"hidden\" name=\"_csrf_token\" value=\""));
    assert!(field.contains(&token));
    assert!(field.ends_with("\">"));

    let sess = session();
    assert!(sess.verify_csrf_token(&token));
    assert!(!sess.verify_csrf_token("forged_csrf_token"));
}

#[test]
fn test_cryptographic_secure_random_entropy() {
    use terminus::prelude::*;
    use std::collections::HashSet;

    let mut generated = HashSet::new();
    for _ in 0..100 {
        let hex = secure_random_hex(16);
        assert_eq!(hex.len(), 32);
        assert!(generated.insert(hex), "Detected duplicate entropy generation");
    }

    let mut buf = [0u8; 32];
    secure_random_bytes(&mut buf);
    assert!(buf.iter().any(|&b| b != 0), "Entropy buffer must not be all zeroes");
}
