use anyhow::Result;
use async_trait::async_trait;
use flux_core::entity::prelude::*;
use flux_core::traits::auth::Authenticator;
use sea_orm::{DatabaseConnection, EntityTrait};

pub struct DbAuthenticator {
    db: DatabaseConnection,
    handle: tokio::runtime::Handle,
}

impl DbAuthenticator {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            handle: tokio::runtime::Handle::current(),
        }
    }
}

#[async_trait]
impl Authenticator for DbAuthenticator {
    async fn authenticate(
        &self,
        client_id: &str,
        _username: Option<&str>,
        password: Option<&[u8]>,
    ) -> Result<bool> {
        let client_id = client_id.to_string();
        let password = password.map(|p| p.to_vec());
        let db = self.db.clone();

        // ntex worker threads don't have a Tokio runtime context.
        // We must offload the DB query to the Tokio runtime via the captured handle.
        let is_valid = self
            .handle
            .spawn(async move {
                let device_opt = Devices::find_by_id(&client_id)
                    .one(&db)
                    .await
                    .ok()
                    .flatten();

                if let Some(device) = device_opt {
                    if let Some(token) = &device.token {
                        if let Some(pwd) = password {
                            let pwd_str = std::str::from_utf8(&pwd).unwrap_or("");
                            if token == pwd_str {
                                return true;
                            } else {
                                tracing::warn!(
                                    "Auth failed for device {}: invalid password",
                                    client_id
                                );
                                return false;
                            }
                        } else {
                            tracing::warn!(
                                "Auth failed for device {}: password required",
                                client_id
                            );
                            return false;
                        }
                    } else {
                        if password.is_some() {
                            tracing::warn!(
                                "Auth warning for device {}: password provided but none set in DB",
                                client_id
                            );
                        }
                        return true;
                    }
                }

                tracing::warn!("Auth failed: device {} not found", client_id);
                false
            })
            .await?;

        Ok(is_valid)
    }
}
