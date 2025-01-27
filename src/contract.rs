#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ChunkResponse, CooldownResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Config, Dimensions, PixelInfo, CHUNKS, CONFIG, COOLDOWNS, DIMENSIONS};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:nopixels";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const CHUNK_SIZE: u64 = 32;

fn validate_color(color_code: u8) -> Result<(), ContractError> {
    if color_code > 15 {
        return Err(ContractError::InvalidColor {});
    }

    Ok(())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let admin_address = deps.api.addr_validate(&msg.admin_address)?;

    if let Some(start_height) = msg.start_height {
        if start_height > env.block.height {
            return Err(ContractError::InvalidStartHeight {});
        }
    } else if let Some(end_height) = msg.end_height {
        if end_height <= env.block.height {
            return Err(ContractError::InvalidEndHeight {});
        }
    }

    let config = Config {
        admin_address,
        cooldown: msg.cooldown,
        end_height: msg.end_height,
        start_height: msg.start_height
    };
    let dimensions = Dimensions {
        width: msg.width,
        height: msg.height,
    };

    CONFIG.save(deps.storage, &config)?;
    DIMENSIONS.save(deps.storage, &dimensions)?;

    Ok(Response::new().add_attribute("method", "instantiate"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Draw {
            chunk_x,
            chunk_y,
            x,
            y,
            color,
        } => execute_draw(deps, env, info, chunk_x, chunk_y, x, y, color),
        ExecuteMsg::UpdateAdmin { new_admin_address } => {
            execute_update_admin(deps, env, info, new_admin_address)
        }
        ExecuteMsg::UpdateDimensions { new_width, new_height } => {
            execute_update_dimensions(deps, env, info, new_width, new_height)
        }
        ExecuteMsg::UpdateCooldown { new_cooldown } => {
            execute_update_cooldown(deps, env, info, new_cooldown)
        }
        ExecuteMsg::UpdateEndHeight { new_end_height } => {
            execute_update_end_height(deps, env, info, new_end_height)
        }
        ExecuteMsg::UpdateStartHeight { new_start_height } => {
            execute_update_start_height(deps, env, info, new_start_height)
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn execute_draw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    chunk_x: u64,
    chunk_y: u64,
    x: u64,
    y: u64,
    color: u8,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let dimensions = DIMENSIONS.load(deps.storage)?;
    let user_cooldown = COOLDOWNS
        .may_load(deps.storage, &info.sender)?
        .unwrap_or_default();
    validate_color(color)?;
    if x > CHUNK_SIZE - 1
        || y > CHUNK_SIZE - 1
        || chunk_x > dimensions.width - 1
        || chunk_y > dimensions.height - 1
    {
        return Err(ContractError::InvalidCoordinates {});
    }

    if env.block.height < user_cooldown {
        return Err(ContractError::StillOnCooldown {});
    }
    if let Some(start_height) = config.start_height {
        if env.block.height < start_height {
            return Err(ContractError::StartHeightNotReached {});
        } else if let Some(end_height) = config.end_height {
            if env.block.height > end_height && end_height > start_height {
                return Err(ContractError::EndHeightReached {});
            }
        }
    } else if let Some(end_height) = config.end_height {
        if env.block.height > end_height {
            return Err(ContractError::EndHeightReached {});
        }
    }

    let default = vec![
        vec![
            PixelInfo {
                color: 0
            };
            CHUNK_SIZE as usize
        ];
        CHUNK_SIZE as usize
    ];
    let mut chunk = CHUNKS
        .may_load(deps.storage, (chunk_x, chunk_y))?
        .unwrap_or(default);
    chunk[y as usize][x as usize] = PixelInfo {
        color
    };

    CHUNKS.save(deps.storage, (chunk_x, chunk_y), &chunk)?;
    COOLDOWNS.save(
        deps.storage,
        &info.sender,
        &(env.block.height + config.cooldown),
    )?;

    Ok(Response::new().add_attribute("action", "draw"))
}

pub fn execute_update_admin(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_admin_address: String,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin_address {
        return Err(ContractError::Unauthorized {});
    }

    let validated_admin_address = deps.api.addr_validate(&new_admin_address)?;
    config.admin_address = validated_admin_address;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_admin"))
}

pub fn execute_update_dimensions(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_width: u64,
    new_height: u64,
) -> Result<Response, ContractError> {
    let mut dimensions = DIMENSIONS.load(deps.storage)?;
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin_address {
        return Err(ContractError::Unauthorized {});
    }
    if info.sender != config.admin_address {
        return Err(ContractError::Unauthorized {});
    }

    dimensions.width = new_width;
    dimensions.height = new_height;

    DIMENSIONS.save(deps.storage, &dimensions)?;

    Ok(Response::new().add_attribute("action", "update_dimensions"))
}

pub fn execute_update_cooldown(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_cooldown: u64,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin_address {
        return Err(ContractError::Unauthorized {});
    }

    config.cooldown = new_cooldown;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_cooldown"))
}

pub fn execute_update_end_height(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    new_end_height: Option<u64>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin_address {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(end_height) = new_end_height {
        if end_height <= env.block.height {
            return Err(ContractError::InvalidEndHeight {});
        }
    }

    config.end_height = new_end_height;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_end_height"))
}

pub fn execute_update_start_height(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    new_start_height: Option<u64>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin_address {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(start_height) = new_start_height {
        if start_height <= env.block.height {
            return Err(ContractError::InvalidStartHeight {});
        }
    }

    config.start_height = new_start_height;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_start_height"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => to_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::GetDimensions {} => to_binary(&DIMENSIONS.load(deps.storage)?),
        QueryMsg::GetCooldown { address } => query_cooldown(deps, address),
        QueryMsg::GetChunk { x, y } => to_binary(&ChunkResponse {
            grid: CHUNKS.may_load(deps.storage, (x, y))?.unwrap_or_else(|| {
                vec![
                    vec![
                        PixelInfo {
                            color: 0
                        };
                        CHUNK_SIZE as usize
                    ];
                    CHUNK_SIZE as usize
                ]
            }),
        }),
    }
}

pub fn query_cooldown(deps: Deps, address: String) -> StdResult<Binary> {
    let address = deps.api.addr_validate(&address).unwrap();
    let current_cooldown = COOLDOWNS
        .may_load(deps.storage, &address)?
        .unwrap_or_default();
    to_binary(&CooldownResponse { current_cooldown })
}
