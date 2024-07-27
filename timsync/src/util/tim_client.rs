use anyhow::{Context, Result};
use rand::Rng;
use reqwest::{Client, ClientBuilder, RequestBuilder};
use serde::Deserialize;
use serde_json::json;
use thiserror::Error;

/// TIM API client
pub struct TimClient {
    client: Client,
    tim_host: String,
    xsrf_token: String,
}

#[derive(Error, Debug)]
pub enum TimClientErrors {
    #[error("No XSRF token found. Call refresh_xsrf_token() first.")]
    NoXsrfToken,
    #[error("No TIM host given")]
    NoHost,
    #[error("Invalid username or password for basic login. Server responded with: {0}")]
    InvalidLogin(String),
    #[error("Item not found from {0}: {1}")]
    ItemNotFound(String, String),
    #[error("Could not create item {0}: {1}")]
    CouldNotCreateItem(String, String),
    #[error("Item {0} is not a {1}, but a {2}")]
    InvalidItemType(String, String, String),
    #[error("Failed to process {0}: {1}")]
    ItemError(String, String),
}

/// Information about a TIM item (e.g., document or folder)
#[derive(Deserialize)]
#[allow(dead_code)]
pub struct ItemInfo {
    /// Item ID
    pub id: u64,

    #[serde(rename = "type")]
    /// Item type
    pub item_type: ItemType,

    /// Item's human-readable title
    pub title: String,

    /// URL path to the directory which contains the item in TIM
    pub location: String,

    /// The name of the item used in URLs
    /// To get a full path to the item, combine `location` and `short_name`
    pub short_name: String,

    /// Language ID of the item if it is a document and has a language set
    pub lang_id: Option<String>,
}

#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
// TIM item type
pub enum ItemType {
    Folder,
    Document,
}

impl std::fmt::Display for ItemType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ItemType::Folder => write!(f, "folder"),
            ItemType::Document => write!(f, "document"),
        }
    }
}

impl TimClient {
    /// Create a new uninitialized TIM client.
    ///
    /// **Note about initialization:**  
    /// Creating the client with this method does not initialize the CSRF token.  
    /// You must call `refresh_xsrf_token()` before using the client
    /// in order to use most of TIM API.
    ///
    /// You may also prefer using `TimClientBuilder` to create a client that automatically
    /// resolves the CSRF token.
    ///
    /// Some TIM API also requires authentication. To log in, use the `login_basic()` method.
    ///
    /// # Arguments
    ///
    /// * `tim_host`: TIM host URL, e.g. `https://tim.jyu.fi`
    ///
    /// returns: TimClient
    pub fn new(tim_host: String) -> Self {
        Self {
            client: ClientBuilder::new().cookie_store(true).build().unwrap(),
            tim_host,
            xsrf_token: String::new(),
        }
    }

    /// Refresh the CSRF token.
    ///
    /// The token is needed in most TIM API calls as they are CSRF protected.
    /// Usually, calling this method once is enough before any other calls,
    /// as the same CSRF token can be reused for multiple calls.
    pub async fn refresh_xsrf_token(&mut self) -> Result<()> {
        let result = self.client.get(&self.tim_host).send().await?;

        self.xsrf_token = result
            .cookies()
            .find(|c| c.name() == "XSRF-TOKEN")
            .unwrap()
            .value()
            .to_string();

        Ok(())
    }

    /// Log in to TIM using basic username-password authentication.
    ///
    /// Basic authentication uses TIM password to log in the user.
    /// To create a TIM password for an account, use the
    /// `I forgot my password` option in the login page.
    ///
    /// Calling this method again will log in as a different user.
    ///
    /// # Arguments
    ///
    /// * `username`: TIM username or user's primary email address.
    /// * `password`: TIM password.
    ///
    /// returns: Result<(), Error>
    pub async fn login_basic(&self, username: &str, password: &str) -> Result<()> {
        if self.xsrf_token.is_empty() {
            return Err(TimClientErrors::NoXsrfToken.into());
        }

        let result = self
            .post("emailLogin")
            .form(&[
                ("email", &username),
                ("password", &password),
                ("add_user", &"false"),
            ])
            .send()
            .await?;

        if !result.status().is_success() {
            return Err(TimClientErrors::InvalidLogin(result.status().to_string()).into());
        }

        Ok(())
    }

    /// Create a POST request to a TIM API endpoint.
    ///
    /// # Arguments
    ///
    /// * `tim_url`: Endpoint to make the request to. The hostname is automatically prepended.
    ///
    /// returns: RequestBuilder
    pub fn post(&self, tim_url: &str) -> RequestBuilder {
        self.client
            .post(format!("{}/{}", &self.tim_host, tim_url))
            .header("X-XSRF-TOKEN", &self.xsrf_token)
            .header("Referer", &self.tim_host)
    }

    /// Create a PUT request to a TIM API endpoint.
    ///
    /// # Arguments
    ///
    /// * `tim_url`: Endpoint to make the request to. The hostname is automatically prepended.
    ///
    /// returns: RequestBuilder
    pub fn put(&self, tim_url: &str) -> RequestBuilder {
        self.client
            .put(format!("{}/{}", &self.tim_host, tim_url))
            .header("X-XSRF-TOKEN", &self.xsrf_token)
            .header("Referer", &self.tim_host)
    }

    /// Create a GET request to a TIM API endpoint.
    ///
    /// # Arguments
    ///
    /// * `tim_url`: Endpoint to make the request to. The hostname is automatically prepended.
    ///
    /// returns: RequestBuilder
    pub fn get(&self, tim_url: &str) -> RequestBuilder {
        self.client
            .get(format!("{}/{}", &self.tim_host, tim_url))
            .header("X-XSRF-TOKEN", &self.xsrf_token)
            .header("Referer", &self.tim_host)
    }

    /// Get information about an item (document or folder) in TIM.
    ///
    /// # Arguments
    ///
    /// * `item_path`: Path to the item in TIM, e.g. `kurssit/tie/kurssi`.
    ///
    /// returns: Result<ItemInfo, Error>
    pub async fn get_item_info(&self, item_path: &str) -> Result<ItemInfo> {
        let result = self
            .get(&format!("itemInfo/{}", item_path))
            .send()
            .await
            .context("Could not get item info");

        match result {
            Ok(result) => {
                if result.status().is_success() {
                    let json = result
                        .json::<ItemInfo>()
                        .await
                        .context("Could not parse item info JSON")?;
                    Ok(json)
                } else {
                    Err(TimClientErrors::ItemNotFound(
                        item_path.to_string(),
                        result.status().to_string(),
                    )
                    .into())
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Create a new item (document or folder) in TIM.
    ///
    /// # Arguments
    ///
    /// * `item_type`: Item type to create.
    /// * `item_path`: Full path to the new item, e.g. `kurssit/tie/kurssi`.
    /// * `title`: Human-readable title for the item.
    ///
    /// returns: Result<(), Error>
    pub async fn create_item(
        &self,
        item_type: ItemType,
        item_path: &str,
        title: &str,
    ) -> Result<()> {
        let result = self
            .post("createItem")
            .form(&[
                ("item_path", item_path),
                ("item_title", title),
                ("item_type", &item_type.to_string()),
            ])
            .send()
            .await
            .with_context(|| format!("Could not create item {}", item_path));

        match result {
            Ok(result) => {
                if result.status().is_success() {
                    Ok(())
                } else {
                    Err(TimClientErrors::CouldNotCreateItem(
                        item_path.to_string(),
                        result.status().to_string(),
                    )
                    .into())
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Create a new item (document or folder) in TIM, or update the title if it already exists.
    /// Returns information about the item.
    ///
    /// # Arguments
    ///
    /// * `item_type`: Item type to create.
    /// * `path`: Full path to the new item, e.g. `kurssit/tie/kurssi`.
    /// * `title`: Human-readable title for the item.
    ///
    /// returns: Result<ItemInfo, Error>
    pub async fn create_or_update_item(
        &self,
        item_type: ItemType,
        path: &str,
        title: &str,
    ) -> Result<ItemInfo> {
        let item_info = self.get_item_info(&path).await;
        match item_info {
            Ok(info) => {
                if info.item_type == item_type {
                    self.set_item_title(&path, title).await?;
                    Ok(info)
                } else {
                    Err(TimClientErrors::InvalidItemType(
                        path.to_string(),
                        item_type.to_string(),
                        info.item_type.to_string(),
                    )
                    .into())
                }
            }
            Err(e) => {
                match e.downcast_ref::<TimClientErrors>() {
                    Some(TimClientErrors::ItemNotFound(_, _)) => {
                        // Item does not exist, create it
                        self.create_item(item_type, &path, title).await?;
                        let item_info = self.get_item_info(&path).await?;
                        Ok(item_info)
                    }
                    _ => Err(e),
                }
            }
        }
    }

    /// Set the title of an item (document or folder) in TIM.
    ///
    /// # Arguments
    ///
    /// * `item_path`: Full path to the new item, e.g. `kurssit/tie/kurssi`.
    /// * `title`: New title for the item.
    ///
    /// returns: Result<(), Error>
    pub async fn set_item_title(&self, item_path: &str, title: &str) -> Result<()> {
        let item = self.get_item_info(item_path).await?;

        let result = self
            .put(&format!("changeTitle/{}", item.id))
            .json(&json!({
                "new_title": title,
            }))
            .send()
            .await
            .with_context(|| format!("Could not set title for item {}", item_path))?;

        if result.status().is_success() {
            Ok(())
        } else {
            Err(
                TimClientErrors::ItemError(item_path.to_string(), result.status().to_string())
                    .into(),
            )
        }
    }

    /// Download the markdown contents of a document in TIM.
    ///
    /// # Arguments
    ///
    /// * `item_path`: Path to the document in TIM, e.g. `kurssit/tie/kurssi`.
    ///
    /// returns: Result<String, Error>
    pub async fn download_markdown(&self, item_path: &str) -> Result<String> {
        let item = self.get_item_info(item_path).await?;

        let result = self
            .get(&format!("download/{}", item.id))
            .send()
            .await
            .with_context(|| format!("Could not download item {}", item_path))?;

        if result.status().is_success() {
            let markdown = result
                .text()
                .await
                .context("Could not load markdown response")?;
            Ok(markdown)
        } else {
            Err(
                TimClientErrors::ItemError(item_path.to_string(), result.status().to_string())
                    .into(),
            )
        }
    }

    /// Upload markdown contents to a document in TIM.
    ///
    /// # Arguments
    ///
    /// * `item_path`: Path to the document in TIM, e.g. `kurssit/tie/kurssi`.
    /// * `markdown`: New markdown contents of the document.
    ///
    /// returns: Result<(), Error>
    pub async fn upload_markdown(&self, item_path: &str, markdown: &str) -> Result<()> {
        let item = self.get_item_info(item_path).await?;

        match item.item_type {
            ItemType::Document => (),
            _ => {
                return Err(TimClientErrors::InvalidItemType(
                    item_path.to_string(),
                    ItemType::Document.to_string(),
                    item.item_type.to_string(),
                )
                .into());
            }
        }

        let current_markdown = self.download_markdown(item_path).await?;

        let result = self
            .post(&format!("update/{}", item.id))
            .json(&json!({
                "fulltext": markdown,
                "original": current_markdown.as_str(),
            }))
            .send()
            .await
            .with_context(|| format!("Could not upload markdown to {}", item_path))?;

        if result.status().is_success() {
            Ok(())
        } else {
            Err(
                TimClientErrors::ItemError(item_path.to_string(), result.status().to_string())
                    .into(),
            )
        }
    }
}

/// Builder for TimClient
pub struct TimClientBuilder {
    tim_host: Option<String>,
}

impl TimClientBuilder {
    /// Create a new TimClientBuilder.
    pub fn new() -> Self {
        Self { tim_host: None }
    }

    /// Set the TIM host URL.
    ///
    /// The host must be a valid URL, e.g. `https://tim.jyu.fi`.
    ///
    /// # Arguments
    ///
    /// * `tim_host`: TIM host URL
    ///
    /// returns: TimClientBuilder
    pub fn tim_host(mut self, tim_host: &str) -> Self {
        self.tim_host = Some(tim_host.to_string());
        self
    }

    /// Build a new TimClient.
    ///
    /// This will validate the host and refresh the CSRF token, making the client ready to use.
    ///
    /// returns: Result<TimClient, Error>
    pub async fn build(self) -> Result<TimClient> {
        let host = self.tim_host.clone().ok_or(TimClientErrors::NoHost)?;
        let mut tim_client = TimClient::new(host);
        tim_client.refresh_xsrf_token().await?;
        Ok(tim_client)
    }
}

const GEN_ASCII_STR_CHARSET: &[u8] =
    b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
const GEN_ASCII_STR_CHARSET_LEN: usize = GEN_ASCII_STR_CHARSET.len();

pub fn random_par_id() -> String {
    fn luhn_checksum(id: &str) -> usize {
        let mut acc = 0;
        let ascii_str = id.as_bytes();
        for i in (0..id.len()).rev() {
            let c = ascii_str[i];
            let value = GEN_ASCII_STR_CHARSET.iter().position(|&x| x == c).unwrap();
            acc += if i % 2 == 0 { value * 2 } else { value };
        }
        acc % GEN_ASCII_STR_CHARSET_LEN
    }

    fn id_checksum(id: &str) -> char {
        let check_digit = luhn_checksum(&format!("{}{}", id, GEN_ASCII_STR_CHARSET[0] as char));
        if check_digit == 0 {
            GEN_ASCII_STR_CHARSET[0] as char
        } else {
            GEN_ASCII_STR_CHARSET[GEN_ASCII_STR_CHARSET_LEN - check_digit] as char
        }
    }

    let mut rand = rand::thread_rng();
    let random_id = (0..11)
        .map(|_| {
            let idx = rand.gen_range(0..GEN_ASCII_STR_CHARSET_LEN);
            GEN_ASCII_STR_CHARSET[idx] as char
        })
        .collect::<String>();

    format!("{}{}", random_id, id_checksum(&random_id))
}
