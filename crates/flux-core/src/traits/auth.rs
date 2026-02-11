use async_trait::async_trait;

#[async_trait]
pub trait Authenticator: Send + Sync {
    /// Authenticate a client.
    ///
    /// # Arguments
    /// * `client_id` - The client ID provided in the CONNECT packet.
    /// * `username` - The optional username provided in the CONNECT packet.
    /// * `password` - The optional password provided in the CONNECT packet.
    ///
    /// # Returns
    /// * `Ok(true)` if authentication is successful.
    /// * `Ok(false)` if authentication failed.
    /// * `Err(e)` if an internal error occurred.
    async fn authenticate(
        &self,
        client_id: &str,
        username: Option<&str>,
        password: Option<&[u8]>,
    ) -> Result<bool, anyhow::Error>;
}
