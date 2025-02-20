use super::channel::Channel;
#[cfg(feature = "tls")]
use super::{
    service::TlsConnector,
    tls::{Certificate, Identity, TlsProvider},
};
use bytes::Bytes;
use http::uri::{InvalidUriBytes, Uri};
use std::{
    convert::{TryFrom, TryInto},
    fmt,
    sync::Arc,
    time::Duration,
};

/// Channel builder.
///
/// This struct is used to build and configure HTTP/2 channels.
#[derive(Clone)]
pub struct Endpoint {
    pub(super) uri: Uri,
    pub(super) timeout: Option<Duration>,
    pub(super) concurrency_limit: Option<usize>,
    pub(super) rate_limit: Option<(u64, Duration)>,
    #[cfg(feature = "tls")]
    pub(super) tls: Option<TlsConnector>,
    pub(super) buffer_size: Option<usize>,
    pub(super) interceptor_headers:
        Option<Arc<dyn Fn(&mut http::HeaderMap) + Send + Sync + 'static>>,
    pub(super) init_stream_window_size: Option<u32>,
    pub(super) init_connection_window_size: Option<u32>,
}

impl Endpoint {
    // FIXME: determine if we want to expose this or not. This is really
    // just used in codegen for a shortcut.
    #[doc(hidden)]
    pub fn new<D>(dst: D) -> Result<Self, super::Error>
    where
        D: TryInto<Self>,
        D::Error: Into<crate::Error>,
    {
        let me = dst
            .try_into()
            .map_err(|e| super::Error::from_source(super::ErrorKind::Client, e.into()))?;
        Ok(me)
    }

    /// Convert an `Endpoint` from a static string.
    ///
    /// ```
    /// # use tonic::transport::Endpoint;
    /// Endpoint::from_static("https://example.com");
    /// ```
    pub fn from_static(s: &'static str) -> Self {
        let uri = Uri::from_static(s);
        Self::from(uri)
    }

    /// Convert an `Endpoint` from shared bytes.
    ///
    /// ```
    /// # use tonic::transport::Endpoint;
    /// Endpoint::from_shared("https://example.com".to_string());
    /// ```
    pub fn from_shared(s: impl Into<Bytes>) -> Result<Self, InvalidUriBytes> {
        let uri = Uri::from_shared(s.into())?;
        Ok(Self::from(uri))
    }

    /// Apply a timeout to each request.
    ///
    /// ```
    /// # use tonic::transport::Endpoint;
    /// # use std::time::Duration;
    /// # let mut builder = Endpoint::from_static("https://example.com");
    /// builder.timeout(Duration::from_secs(5));
    /// ```
    pub fn timeout(&mut self, dur: Duration) -> &mut Self {
        self.timeout = Some(dur);
        self
    }

    /// Apply a concurrency limit to each request.
    ///
    /// ```
    /// # use tonic::transport::Endpoint;
    /// # let mut builder = Endpoint::from_static("https://example.com");
    /// builder.concurrency_limit(256);
    /// ```
    pub fn concurrency_limit(&mut self, limit: usize) -> &mut Self {
        self.concurrency_limit = Some(limit);
        self
    }

    /// Apply a rate limit to each request.
    ///
    /// ```
    /// # use tonic::transport::Endpoint;
    /// # use std::time::Duration;
    /// # let mut builder = Endpoint::from_static("https://example.com");
    /// builder.rate_limit(32, Duration::from_secs(1));
    /// ```
    pub fn rate_limit(&mut self, limit: u64, duration: Duration) -> &mut Self {
        self.rate_limit = Some((limit, duration));
        self
    }

    /// Sets the [`SETTINGS_INITIAL_WINDOW_SIZE`][spec] option for HTTP2
    /// stream-level flow control.
    ///
    /// Default is 65,535
    ///
    /// [spec]: https://http2.github.io/http2-spec/#SETTINGS_INITIAL_WINDOW_SIZE
    pub fn initial_stream_window_size(&mut self, sz: impl Into<Option<u32>>) -> &mut Self {
        self.init_stream_window_size = sz.into();
        self
    }

    /// Sets the max connection-level flow control for HTTP2
    ///
    /// Default is 65,535
    pub fn initial_connection_window_size(&mut self, sz: impl Into<Option<u32>>) -> &mut Self {
        self.init_connection_window_size = sz.into();
        self
    }

    /// Intercept outbound HTTP Request headers;
    pub fn intercept_headers<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn(&mut http::HeaderMap) + Send + Sync + 'static,
    {
        self.interceptor_headers = Some(Arc::new(f));
        self
    }

    /// Configures TLS for the endpoint.
    #[cfg(feature = "tls")]
    pub fn tls_config(&mut self, tls_config: &ClientTlsConfig) -> &mut Self {
        self.tls = Some(tls_config.tls_connector(self.uri.clone()).unwrap());
        self
    }

    /// Create a channel from this config.
    pub async fn connect(&self) -> Result<Channel, super::Error> {
        Channel::connect(self.clone()).await
    }
}

impl From<Uri> for Endpoint {
    fn from(uri: Uri) -> Self {
        Self {
            uri,
            concurrency_limit: None,
            rate_limit: None,
            timeout: None,
            #[cfg(feature = "tls")]
            tls: None,
            buffer_size: None,
            interceptor_headers: None,
            init_stream_window_size: None,
            init_connection_window_size: None,
        }
    }
}

impl TryFrom<Bytes> for Endpoint {
    type Error = InvalidUriBytes;

    fn try_from(t: Bytes) -> Result<Self, Self::Error> {
        Self::from_shared(t)
    }
}

impl TryFrom<String> for Endpoint {
    type Error = InvalidUriBytes;

    fn try_from(t: String) -> Result<Self, Self::Error> {
        Self::from_shared(t.into_bytes())
    }
}

impl TryFrom<&'static str> for Endpoint {
    type Error = Never;

    fn try_from(t: &'static str) -> Result<Self, Self::Error> {
        Ok(Self::from_static(t))
    }
}

#[derive(Debug)]
pub enum Never {}

impl std::fmt::Display for Never {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {}
    }
}

impl std::error::Error for Never {}

impl fmt::Debug for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Endpoint").finish()
    }
}

/// Configures TLS settings for endpoints.
#[cfg(feature = "tls")]
#[derive(Clone)]
pub struct ClientTlsConfig {
    provider: TlsProvider,
    domain: Option<String>,
    cert: Option<Certificate>,
    identity: Option<Identity>,
    #[cfg(feature = "openssl")]
    openssl_raw: Option<openssl1::ssl::SslConnector>,
    #[cfg(feature = "rustls")]
    rustls_raw: Option<tokio_rustls::rustls::ClientConfig>,
}

#[cfg(feature = "tls")]
impl fmt::Debug for ClientTlsConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientTlsConfig")
            .field("provider", &self.provider)
            .field("domain", &self.domain)
            .field("cert", &self.cert)
            .field("identity", &self.identity)
            .finish()
    }
}

#[cfg(feature = "tls")]
impl ClientTlsConfig {
    /// Creates a new `ClientTlsConfig` using OpenSSL.
    #[cfg(feature = "openssl")]
    pub fn with_openssl() -> Self {
        Self::new(TlsProvider::OpenSsl)
    }

    /// Creates a new `ClientTlsConfig` using Rustls.
    #[cfg(feature = "rustls")]
    pub fn with_rustls() -> Self {
        Self::new(TlsProvider::Rustls)
    }

    fn new(provider: TlsProvider) -> Self {
        ClientTlsConfig {
            provider,
            domain: None,
            cert: None,
            identity: None,
            #[cfg(feature = "openssl")]
            openssl_raw: None,
            #[cfg(feature = "rustls")]
            rustls_raw: None,
        }
    }

    /// Sets the domain name against which to verify the server's TLS certificate.
    pub fn domain_name(&mut self, domain_name: impl Into<String>) -> &mut Self {
        self.domain = Some(domain_name.into());
        self
    }

    /// Sets the CA Certificate against which to verify the server's TLS certificate.
    pub fn ca_certificate(&mut self, ca_certificate: Certificate) -> &mut Self {
        self.cert = Some(ca_certificate);
        self
    }

    /// Sets the client identity to present to the server.
    pub fn identity(&mut self, identity: Identity) -> &mut Self {
        self.identity = Some(identity);
        self
    }

    /// Use options specified by the given `SslConnector` to configure TLS.
    ///
    /// This overrides all other TLS options set via other means.
    #[cfg(feature = "openssl")]
    pub fn openssl_connector(&mut self, connector: openssl1::ssl::SslConnector) -> &mut Self {
        self.openssl_raw = Some(connector);
        self
    }

    /// Use options specified by the given `ClientConfig` to configure TLS.
    ///
    /// This overrides all other TLS options set via other means.
    #[cfg(feature = "rustls")]
    pub fn rustls_client_config(
        &mut self,
        config: tokio_rustls::rustls::ClientConfig,
    ) -> &mut Self {
        self.rustls_raw = Some(config);
        self
    }

    fn tls_connector(&self, uri: Uri) -> Result<TlsConnector, crate::Error> {
        let domain = match &self.domain {
            None => uri.to_string(),
            Some(domain) => domain.clone(),
        };
        match self.provider {
            #[cfg(feature = "openssl")]
            TlsProvider::OpenSsl => match &self.openssl_raw {
                None => TlsConnector::new_with_openssl_cert(
                    self.cert.clone(),
                    self.identity.clone(),
                    domain,
                ),
                Some(r) => TlsConnector::new_with_openssl_raw(r.clone(), domain),
            },
            #[cfg(feature = "rustls")]
            TlsProvider::Rustls => match &self.rustls_raw {
                None => TlsConnector::new_with_rustls_cert(
                    self.cert.clone(),
                    self.identity.clone(),
                    domain,
                ),
                Some(c) => TlsConnector::new_with_rustls_raw(c.clone(), domain),
            },
        }
    }
}
