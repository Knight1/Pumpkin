use std::net::{IpAddr, SocketAddr};

use bytes::{BufMut, BytesMut};
use hmac::{Hmac, Mac};
use pumpkin_config::proxy::VelocityConfig;
use pumpkin_protocol::{
    bytebuf::ByteBuffer, client::login::CLoginPluginRequest, server::login::SLoginPluginResponse,
    Property,
};
use rand::Rng;
use sha2::Sha256;
use thiserror::Error;

use crate::client::{authentication::GameProfile, Client};

/// Proxy implementation for Velocity <https://papermc.io/software/velocity> by PaperMC
/// Sadly PaperMC does not care about 3th Parties providing support for Velocity, There is no documentation.
/// I had to understand the Code logic by looking at PaperMC's Velocity implementation: <https://github.com/PaperMC/Paper/blob/master/patches/server/0731-Add-Velocity-IP-Forwarding-Support.patch>

type HmacSha256 = Hmac<Sha256>;

const MAX_SUPPORTED_FORWARDING_VERSION: u8 = 4;
const PLAYER_INFO_CHANNEL: &str = "velocity:player_info";

#[derive(Error, Debug)]
pub enum VelocityError {
    #[error("No response data received")]
    NoData,
    #[error("Unable to verify player details")]
    FailedVerifyIntegrity,
    #[error("Failed to read forward version")]
    FailedReadForwardVersion,
    #[error("Unsupported forwarding version {0}. Maximum supported version is {1}")]
    UnsupportedForwardVersion(u8, u8),
    #[error("Failed to read address")]
    FailedReadAddress,
    #[error("Failed to parse address")]
    FailedParseAddres,
    #[error("Failed to read game profile name")]
    FailedReadProfileName,
    #[error("Failed to read game profile UUID")]
    FailedReadProfileUUID,
    #[error("Failed to read game profile properties")]
    FailedReadProfileProperties,
}

pub async fn velocity_login(client: &Client) {
    // TODO: validate packet transaction id from plugin response with this
    let velocity_message_id: i32 = rand::thread_rng().gen();

    let mut buf = BytesMut::new();
    buf.put_u8(MAX_SUPPORTED_FORWARDING_VERSION);
    client
        .send_packet(&CLoginPluginRequest::new(
            velocity_message_id.into(),
            PLAYER_INFO_CHANNEL,
            &buf,
        ))
        .await;
}

pub fn check_integrity(data: (&[u8], &[u8]), secret: &str) -> bool {
    let (signature, data_without_signature) = data;
    // Our fault, We can panic/expect ?
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(data_without_signature);
    mac.verify_slice(signature).is_ok()
}

fn read_game_profile(buf: &mut ByteBuffer) -> Result<GameProfile, VelocityError> {
    let id = buf
        .get_uuid()
        .map_err(|_| VelocityError::FailedReadProfileUUID)?;

    let name = buf
        .get_string()
        .map_err(|_| VelocityError::FailedReadProfileName)?;
    let properties = buf
        .get_list(|data| {
            let name = data.get_string()?;
            let value = data.get_string()?;
            let signature = data.get_option(|data| data.get_string())?;

            Ok(Property {
                name,
                value,
                signature,
            })
        })
        .map_err(|_| VelocityError::FailedReadProfileProperties)?;
    Ok(GameProfile {
        id,
        name,
        properties,
        profile_actions: None,
    })
}

pub fn receive_velocity_plugin_response(
    port: u16,
    config: &VelocityConfig,
    response: SLoginPluginResponse,
) -> Result<(GameProfile, SocketAddr), VelocityError> {
    dbg!("velocity response");
    if let Some(data) = response.data {
        let (signature, data_without_signature) = data.split_at(32);

        if !check_integrity((signature, data_without_signature), &config.secret) {
            return Err(VelocityError::FailedVerifyIntegrity);
        }
        let mut buf = ByteBuffer::new(BytesMut::new());
        buf.put_slice(data_without_signature);

        // check velocity version
        let version = buf
            .get_var_int()
            .map_err(|_| VelocityError::FailedReadForwardVersion)?;
        let version = version.0 as u8;
        if version > MAX_SUPPORTED_FORWARDING_VERSION {
            return Err(VelocityError::UnsupportedForwardVersion(
                version,
                MAX_SUPPORTED_FORWARDING_VERSION,
            ));
        }
        let addr = buf
            .get_string()
            .map_err(|_| VelocityError::FailedReadAddress)?;

        let socket_addr: SocketAddr = SocketAddr::new(
            addr.parse::<IpAddr>()
                .map_err(|_| VelocityError::FailedParseAddres)?,
            port,
        );
        let profile = read_game_profile(&mut buf)?;
        return Ok((profile, socket_addr));
    }
    Err(VelocityError::NoData)
}
