use std::{env, time::Duration};

use chrono::{DateTime, TimeDelta, Utc};
use log::debug;
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Certificate, Client,
};

use crate::core::authenticator::SessionTokenProvider;

const FBX_APP_AUTH_HEADER: &str = "X-Fbx-App-Auth";

const FBX_ECC_ROOT: &str = "
-----BEGIN CERTIFICATE-----
MIICWTCCAd+gAwIBAgIJAMaRcLnIgyukMAoGCCqGSM49BAMCMGExCzAJBgNVBAYT
AkZSMQ8wDQYDVQQIDAZGcmFuY2UxDjAMBgNVBAcMBVBhcmlzMRMwEQYDVQQKDApG
cmVlYm94IFNBMRwwGgYDVQQDDBNGcmVlYm94IEVDQyBSb290IENBMB4XDTE1MDkw
MTE4MDIwN1oXDTM1MDgyNzE4MDIwN1owYTELMAkGA1UEBhMCRlIxDzANBgNVBAgM
BkZyYW5jZTEOMAwGA1UEBwwFUGFyaXMxEzARBgNVBAoMCkZyZWVib3ggU0ExHDAa
BgNVBAMME0ZyZWVib3ggRUNDIFJvb3QgQ0EwdjAQBgcqhkjOPQIBBgUrgQQAIgNi
AASCjD6ZKn5ko6cU5Vxh8GA1KqRi6p2GQzndxHtuUmwY8RvBbhZ0GIL7bQ4f08ae
JOv0ycWjEW0fyOnAw6AYdsN6y1eNvH2DVfoXQyGoCSvXQNAUxla+sJuLGICRYiZz
mnijYzBhMB0GA1UdDgQWBBTIB3c2GlbV6EIh2ErEMJvFxMz/QTAfBgNVHSMEGDAW
gBTIB3c2GlbV6EIh2ErEMJvFxMz/QTAPBgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB
/wQEAwIBhjAKBggqhkjOPQQDAgNoADBlAjA8tzEMRVX8vrFuOGDhvZr7OSJjbBr8
gl2I70LeVNGEXZsAThUkqj5Rg9bV8xw3aSMCMQCDjB5CgsLH8EdZmiksdBRRKM2r
vxo6c0dSSNrr7dDN+m2/dRvgoIpGL2GauOGqDFY=
-----END CERTIFICATE-----";

const FBX_ROOT_CA: &str = "
-----BEGIN CERTIFICATE-----
MIIFmjCCA4KgAwIBAgIJAKLyz15lYOrYMA0GCSqGSIb3DQEBCwUAMFoxCzAJBgNV
BAYTAkZSMQ8wDQYDVQQIDAZGcmFuY2UxDjAMBgNVBAcMBVBhcmlzMRAwDgYDVQQK
DAdGcmVlYm94MRgwFgYDVQQDDA9GcmVlYm94IFJvb3QgQ0EwHhcNMTUwNzMwMTUw
OTIwWhcNMzUwNzI1MTUwOTIwWjBaMQswCQYDVQQGEwJGUjEPMA0GA1UECAwGRnJh
bmNlMQ4wDAYDVQQHDAVQYXJpczEQMA4GA1UECgwHRnJlZWJveDEYMBYGA1UEAwwP
RnJlZWJveCBSb290IENBMIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEA
xqYIvq8538SH6BJ99jDlOPoyDBrlwKEp879oYplicTC2/p0X66R/ft0en1uSQadC
sL/JTyfgyJAgI1Dq2Y5EYVT/7G6GBtVH6Bxa713mM+I/v0JlTGFalgMqamMuIRDQ
tdyvqEIs8DcfGB/1l2A8UhKOFbHQsMcigxOe9ZodMhtVNn0mUyG+9Zgu1e/YMhsS
iG4Kqap6TGtk80yruS1mMWVSgLOq9F5BGD4rlNlWLo0C3R10mFCpqvsFU+g4kYoA
dTxaIpi1pgng3CGLE0FXgwstJz8RBaZObYEslEYKDzmer5zrU1pVHiwkjsgwbnuy
WtM1Xry3Jxc7N/i1rxFmN/4l/Tcb1F7x4yVZmrzbQVptKSmyTEvPvpzqzdxVWuYi
qIFSe/njl8dX9v5hjbMo4CeLuXIRE4nSq2A7GBm4j9Zb6/l2WIBpnCKtwUVlroKw
NBgB6zHg5WI9nWGuy3ozpP4zyxqXhaTgrQcDDIG/SQS1GOXKGdkCcSa+VkJ0jTf5
od7PxBn9/TuN0yYdgQK3YDjD9F9+CLp8QZK1bnPdVGywPfL1iztngF9J6JohTyL/
VMvpWfS/X6R4Y3p8/eSio4BNuPvm9r0xp6IMpW92V8SYL0N6TQQxzZYgkLV7TbQI
Hw6v64yMbbF0YS9VjS0sFpZcFERVQiodRu7nYNC1jy8CAwEAAaNjMGEwHQYDVR0O
BBYEFD2erMkECujilR0BuER09FdsYIebMB8GA1UdIwQYMBaAFD2erMkECujilR0B
uER09FdsYIebMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgGGMA0GCSqG
SIb3DQEBCwUAA4ICAQAZ2Nx8mWIWckNY8X2t/ymmCbcKxGw8Hn3BfTDcUWQ7GLRf
MGzTqxGSLBQ5tENaclbtTpNrqPv2k6LY0VjfrKoTSS8JfXkm6+FUtyXpsGK8MrLL
hZ/YdADTfbbWOjjD0VaPUoglvo2N4n7rOuRxVYIij11fL/wl3OUZ7GHLgL3qXSz0
+RGW+1oZo8HQ7pb6RwLfv42Gf+2gyNBckM7VVh9R19UkLCsHFqhFBbUmqwJgNA2/
3twgV6Y26qlyHXXODUfV3arLCwFoNB+IIrde1E/JoOry9oKvF8DZTo/Qm6o2KsdZ
dxs/YcIUsCvKX8WCKtH6la/kFCUcXIb8f1u+Y4pjj3PBmKI/1+Rs9GqB0kt1otyx
Q6bqxqBSgsrkuhCfRxwjbfBgmXjIZ/a4muY5uMI0gbl9zbMFEJHDojhH6TUB5qd0
JJlI61gldaT5Ci1aLbvVcJtdeGhElf7pOE9JrXINpP3NOJJaUSueAvxyj/WWoo0v
4KO7njox8F6jCHALNDLdTsX0FTGmUZ/s/QfJry3VNwyjCyWDy1ra4KWoqt6U7SzM
d5jENIZChM8TnDXJzqc+mu00cI3icn9bV9flYCXLTIsprB21wVSMh0XeBGylKxeB
S27oDfFq04XSox7JM9HdTt2hLK96x1T7FpFrBTnALzb7vHv9MhXqAT90fPR/8A==
-----END CERTIFICATE-----";

#[derive(Clone)]
pub struct AuthenticatedHttpClientFactory<'a> {
    pub api_url: String,
    token_provider: SessionTokenProvider<'a>,
    pub expiration: TimeDelta,
}
static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

impl<'a> AuthenticatedHttpClientFactory<'a> {
    /// Create a new factory with the API URL and the session token provider.
    pub fn new(api_url: String, token_provider: SessionTokenProvider<'a>) -> Self {
        Self {
            api_url,
            token_provider,
            expiration: TimeDelta::minutes(30),
        }
    }

    /// Creates a new managed HTTP client with the necessary headers and configurations.
    ///
    /// # Returns
    ///
    /// A `Result` containing a `ManagedHttpClient` on success, or a boxed error on failure.
    ///
    /// # Errors
    ///
    /// This function will return an error if the session token cannot be retrieved from the token provider.
    ///
    /// # Example
    ///
    /// ```rust
    /// let factory = AuthenticatedHttpClientFactory::new(api_url, token_provider);
    /// let client = factory.create_managed_client().await?;
    /// ```
    pub async fn create_managed_client(
        &self,
    ) -> Result<ManagedHttpClient, Box<dyn std::error::Error + Sync + Send>> {
        debug!("creating managed http client");
        let mut headers = HeaderMap::new();

        let session_token = match self.token_provider.get().await {
            Err(e) => return Err(e),
            Ok(t) => t,
        };

        headers.append(
            FBX_APP_AUTH_HEADER,
            HeaderValue::from_str(session_token.as_str()).unwrap(),
        );

        // Load the freebox API X509 certificate chain
        let root_ca_cert_value = FBX_ROOT_CA.to_string();
        let root_ca = Certificate::from_pem(root_ca_cert_value.as_bytes())?;
        let ecc_cert_value = FBX_ECC_ROOT.to_string();
        let ecc = Certificate::from_pem(ecc_cert_value.as_bytes())?;

        let client = reqwest::ClientBuilder::new()
            .add_root_certificate(root_ca)
            .add_root_certificate(ecc)
            .default_headers(headers)
            .tcp_keepalive(Duration::from_secs(self.expiration.num_seconds() as u64))
            .user_agent(APP_USER_AGENT)
            .build()
            .expect("cannot create HTTP Client");

        Ok(ManagedHttpClient::new(client, self.expiration))
    }
}

/*
auth_required 	Invalid session token, or not session token sent
invalid_token 	The app token you are trying to use is invalid or has been revoked
pending_token 	The app token you are trying to use has not been validated by user yet
insufficient_rights 	Your app permissions does not allow accessing this API
denied_from_external_ip 	You are trying to get an app_token from a remote IP
invalid_request 	Your request is invalid
ratelimited 	Too many auth error have been made from your IP
new_apps_denied 	New application token request has been disabled
apps_denied 	API access from apps has been disabled
internal_error 	Internal error
 */

pub fn http_client_factory() -> Result<Client, ()> {
    debug!("creating HTTP client");

    let client = reqwest::ClientBuilder::new()
        .danger_accept_invalid_certs(true)
        .build()
        .expect("cannot create HTTP Client");
    Ok(client)
}
#[derive(Clone)]
pub struct ManagedHttpClient {
    client: Client,
    expiry: DateTime<Utc>, // 30 minutes
}

impl ManagedHttpClient {
    pub fn new(client: Client, timeout: TimeDelta) -> Self {
        let expiry = Utc::now().checked_add_signed(timeout).unwrap();
        Self { client, expiry }
    }

    pub fn get(&self) -> Result<Client, Box<dyn std::error::Error + Sync + Send>> {
        if Utc::now() > self.expiry {
            return Err(Box::new(ManagedHttpClientError::new(
                "HTTP Client expired".to_string(),
            )));
        }
        Ok(self.client.clone())
    }
}

pub struct ManagedHttpClientError {
    error: String,
}

impl ManagedHttpClientError {
    pub fn new(error: String) -> Self {
        Self { error }
    }
}

impl std::fmt::Display for ManagedHttpClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "ManagedHttpClientError: {}", self.error)
    }
}

impl std::fmt::Debug for ManagedHttpClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "ManagedHttpClientError: {}", self.error)
    }
}

impl std::error::Error for ManagedHttpClientError {}
