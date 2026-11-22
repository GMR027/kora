use std::net::SocketAddr;
use std::sync::Arc;

use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use quinn::{ClientConfig, Connection, Endpoint, ServerConfig};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{ring, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{DigitallySignedStruct, DistinguishedName, SignatureScheme};

use super::identity::DeviceIdentity;
use super::protocol::ALPN;
use crate::error::{KoraError, Result};

/// Verificador que acepta el certificado autofirmado durante el handshake y
/// deja la confianza real a la huella verificada tras la conexión. El canal
/// sigue estando cifrado y autenticado por TLS 1.3.
#[derive(Debug)]
struct AcceptAnyServerCert {
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl ServerCertVerifier for AcceptAnyServerCert {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[derive(Debug)]
struct AcceptAnyClientCert {
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl ClientCertVerifier for AcceptAnyClientCert {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> std::result::Result<ClientCertVerified, rustls::Error> {
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

pub fn server_endpoint(identity: &DeviceIdentity, bind: SocketAddr) -> Result<Endpoint> {
    let provider = Arc::new(ring::default_provider());
    let mut crypto = rustls::ServerConfig::builder()
        .with_client_cert_verifier(Arc::new(AcceptAnyClientCert {
            provider: provider.clone(),
        }))
        .with_single_cert(identity.cert_chain(), identity.private_key())
        .map_err(|error| KoraError::Internal(format!("TLS servidor: {error}")))?;
    crypto.alpn_protocols = vec![ALPN.to_vec()];

    let quic = QuicServerConfig::try_from(crypto)
        .map_err(|error| KoraError::Internal(format!("QUIC servidor: {error}")))?;
    let server_config = ServerConfig::with_crypto(Arc::new(quic));

    let mut endpoint = Endpoint::server(server_config, bind)
        .map_err(|error| KoraError::Internal(format!("no se pudo abrir UDP: {error}")))?;
    endpoint.set_default_client_config(client_config(identity)?);
    Ok(endpoint)
}

pub fn client_config(identity: &DeviceIdentity) -> Result<ClientConfig> {
    let provider = Arc::new(ring::default_provider());
    let mut crypto = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAnyServerCert {
            provider: provider.clone(),
        }))
        .with_client_auth_cert(identity.cert_chain(), identity.private_key())
        .map_err(|error| KoraError::Internal(format!("TLS cliente: {error}")))?;
    crypto.alpn_protocols = vec![ALPN.to_vec()];

    let quic = QuicClientConfig::try_from(crypto)
        .map_err(|error| KoraError::Internal(format!("QUIC cliente: {error}")))?;
    Ok(ClientConfig::new(Arc::new(quic)))
}

pub async fn connect(
    endpoint: &Endpoint,
    address: SocketAddr,
    expected_fingerprint: Option<&str>,
) -> Result<(Connection, String)> {
    let connection = endpoint
        .connect(address, "kora.local")
        .map_err(|error| KoraError::Internal(format!("destino QUIC inválido: {error}")))?
        .await
        .map_err(|error| KoraError::Internal(format!("conexión QUIC falló: {error}")))?;

    let fingerprint = peer_fingerprint(&connection).ok_or_else(|| {
        KoraError::Unsupported("el peer no presentó certificado de dispositivo".into())
    })?;

    if let Some(expected) = expected_fingerprint
        && expected != fingerprint
    {
        connection.close(1u32.into(), b"fingerprint mismatch");
        return Err(KoraError::Conflict(
            "la huella del dispositivo no coincide con la emparejada".into(),
        ));
    }

    Ok((connection, fingerprint))
}

pub fn peer_fingerprint(connection: &Connection) -> Option<String> {
    let identity = connection.peer_identity()?;
    let certs = identity.downcast::<Vec<CertificateDer<'static>>>().ok()?;
    let first = certs.first()?;
    Some(crate::storage::hash::sha256_bytes(first.as_ref()))
}
