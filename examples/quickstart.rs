use terminus::prelude::*;
use std::sync::Arc;

pub struct UserResource {
    repo: Arc<dyn UserRepository>,
}

impl UserResource {
    pub fn new(repo: Arc<dyn UserRepository>) -> Self {
        Self { repo }
    }
}

impl Resource for UserResource {
    fn name(&self) -> &str {
        "User"
    }

    fn plural_name(&self) -> &str {
        "Users"
    }

    fn slug(&self) -> &str {
        "users"
    }

    fn table(&self) -> Table {
        Table::new()
            .column(Column::text("id").label("ID").sortable())
            .column(Column::text("name").label("Name").searchable().sortable())
            .column(Column::text("email").label("Email").searchable())
            .column(Column::badge("role", vec![
                ("Founder", "amber"),
                ("Admin", "blue"),
                ("Member", "slate"),
            ]))
            .column(Column::badge("status", vec![
                ("Active", "emerald"),
                ("Suspended", "rose"),
            ]))
            .page_size(8)
    }

    fn form(&self) -> Form {
        Form::new()
            .field(FormField::text("name").required().placeholder("Full Name"))
            .field(FormField::email("email").required().placeholder("email@example.com"))
    }

    fn fetch_rows(&self, query: &QueryState) -> (Vec<RowData>, usize) {
        let (users, total) = self.repo.list(query);
        let rows = users
            .into_iter()
            .map(|u| {
                RowData::new(&u.id)
                    .insert("id", &u.id)
                    .insert("name", &u.name)
                    .insert("email", &u.email)
                    .insert("role", u.role.as_str())
                    .insert("status", u.status.as_str())
                    .insert("created_at", &u.created_at)
            })
            .collect();
        (rows, total)
    }

    fn delete_row(&self, id: &str) -> Result<(), String> {
        self.repo.delete(id)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::open("terminus.db")?;
    let user_repo = Arc::new(SqliteUserRepository::new(db.connection_arc())?);
    let identity_repo: Arc<dyn UserRepository> = user_repo.clone();
    let credentials = user_repo.clone();

    // Sign-in verifies passwords against the users table (Argon2id hashes).
    // Create the first accounts with `terminus migrate` and
    // `terminus db:seed`, which prints a one-time generated password, or
    // insert your own users with `Hash::make` password hashes.
    let admin = AdminPanel::new()
        .brand_name("Terminus Admin")
        .users(identity_repo.clone())
        .authenticate_with(move |email, password| {
            credentials.verify_credentials(email, password)
        })
        .resource(UserResource::new(identity_repo));

    // With a stable APP_KEY (at least 32 bytes), sessions persist across
    // restarts in SQLite. Without one, an ephemeral key keeps the example
    // runnable locally and sessions end with the process.
    let sessions = match std::env::var("APP_KEY") {
        // secure_cookie(false) is for this local HTTP example only. Keep the
        // default Secure cookies in production behind HTTPS.
        Ok(key) if key.len() >= 32 => {
            SessionConfig::sqlite("sessions.db", key.as_bytes())?.secure_cookie(false)
        }
        _ => {
            eprintln!(
                "APP_KEY is not set to 32+ bytes; using ephemeral in-memory sessions for this run."
            );
            let mut key = [0u8; 32];
            terminus::session::secure_random_bytes(&mut key);
            SessionConfig::new(Arc::new(SessionStore::new()), key)?
        }
    };

    App::new()
        .name("Terminus Quickstart")
        .port(3000)
        .database(db)
        .sessions(sessions)
        .panel(admin)
        .run()
        .await
}
