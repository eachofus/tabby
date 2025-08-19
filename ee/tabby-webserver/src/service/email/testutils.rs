// === IMPORTS ===
use std::time::Duration;

use serde::Deserialize;
use tokio::process::{Child, Command};

use tabby_db::DbConn;
use tabby_schema::email::{AuthMethod, EmailService, EmailSettingInput, Encryption};

use super::new_email_service;

// === STRUCTS ===
/// Email message representation for testing
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct Message {
    pub from: MailAddress,
    pub subject: String,
}

/// Mail address with optional name field
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct MailAddress {
    #[allow(dead_code)] // Field used by serde for deserialization but not accessed directly
    pub name: Option<String>,
    pub address: String,
}

/// Internal structure for API response parsing
#[derive(Deserialize, Debug)]
struct MessageList {
    messages: Vec<Message>,
}

/// Test email server wrapper for integration testing
pub struct TestEmailServer {
    #[allow(dead_code)] // Child process handle kept alive for server lifecycle
    child: Child,
}

// === IMPLEMENTATIONS ===
impl TestEmailServer {
    /// Start a new test email server instance
    pub async fn start() -> TestEmailServer {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let mut cmd = Command::new("mailpit");
        cmd.kill_on_drop(true)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());

        let child = cmd
            .spawn()
            .expect("You need to install `mailpit` before running this test");
        
        // Wait for server to be ready
        loop {
            if reqwest::get("http://localhost:8025").await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
        
        TestEmailServer { child }
    }

    /// Retrieve list of messages from test email server
    pub async fn list_mail(&self) -> Vec<Message> {
        let mails = reqwest::get("http://localhost:8025/api/v1/messages")
            .await
            .unwrap();

        let data = mails.json::<MessageList>().await.unwrap();
        data.messages
    }

    /// Create and configure test email service instance
    pub async fn create_test_email_service(&self, db_conn: DbConn) -> impl EmailService {
        let service = new_email_service(db_conn).await.unwrap();
        service
            .update_setting(default_email_settings())
            .await
            .unwrap();
        service
    }
}

impl Drop for TestEmailServer {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

// === FREE FUNCTIONS ===
/// Create default email settings for testing
fn default_email_settings() -> EmailSettingInput {
    EmailSettingInput {
        smtp_username: "tabby".into(),
        smtp_server: "127.0.0.1".into(),
        smtp_port: 1025,
        from_address: "tabby@localhost".into(),
        encryption: Encryption::None,
        auth_method: AuthMethod::None,
        smtp_password: Some("fake".into()),
    }
}
