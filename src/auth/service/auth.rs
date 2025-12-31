use chrono;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};

use crate::auth::model::NacosJwtPayload;

pub fn decode_jwt_token(
    token: &str,
    secret_key: &str,
) -> jsonwebtoken::errors::Result<jsonwebtoken::TokenData<NacosJwtPayload>> {
    let decoding_key = DecodingKey::from_base64_secret(secret_key)?;
    decode::<NacosJwtPayload>(token, &decoding_key, &Validation::default())
}

pub fn encode_jwt_token(
    sub: &str,
    secret_key: &str,
    expire_seconds: i64,
) -> jsonwebtoken::errors::Result<String> {
    let exp = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::seconds(expire_seconds))
        .unwrap_or_else(chrono::Utc::now)
        .timestamp();

    let payload = NacosJwtPayload {
        sub: sub.to_string(),
        exp,
    };

    let header = Header {
        typ: None,
        alg: Algorithm::HS256,
        cty: None,
        jku: None,
        jwk: None,
        kid: None,
        x5u: None,
        x5c: None,
        x5t: None,
        x5t_s256: None,
    };

    let encoding_key = EncodingKey::from_base64_secret(secret_key)?;
    encode(&header, &payload, &encoding_key)
}
