
use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    program::{invoke, invoke_signed},
    system_instruction,
};
use anchor_lang::system_program;
use anchor_spl::{
    token_2022::Token2022,
    token_2022_extensions::spl_token_metadata_interface::state::TokenMetadata,
    token_interface::{
        token_metadata_initialize,
        TokenMetadataInitialize,
    },
};

use spl_token_2022_interface::{
    extension::{
        ExtensionType,
        default_account_state::instruction as default_state_ix,
        metadata_pointer::instruction as metadata_pointer_ix,
        transfer_fee::instruction as transfer_fee_ix,
    },
    instruction as token_ix,
    state::{AccountState, Mint},
};

// --------------------------------------------------
// CONSTANTS
// --------------------------------------------------

pub const MINT_SEED: &[u8] = b"rusd-mint";

pub const RUSD_DECIMALS: u8 = 6;

// 50 basis points = 0.5%
pub const TRANSFER_FEE_BPS: u16 = 50;

// 10 RUSD in base units (6 decimals)
pub const MAX_TRANSFER_FEE: u64 = 10_000_000;

// --------------------------------------------------
// INSTRUCTION ARGUMENTS
// --------------------------------------------------

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CreateMintArgs {
    pub name: String,
    pub symbol: String,
    pub uri: String,
}

// --------------------------------------------------
// ACCOUNTS
// --------------------------------------------------

#[derive(Accounts)]
pub struct CreateV1Mint<'info> {

    // Issuer pays for mint creation and controls
    // mint, freeze, fee and metadata authorities.

    #[account(mut)]
    pub issuer: Signer<'info>,

    // Mint is a PDA controlled by this program.
    //
    // It will be created with Token-2022 as its owner.

    /// CHECK: Created and initialized in the instruction.
    #[account(
        mut,
        seeds = [MINT_SEED],
        bump
    )]
    pub mint: UncheckedAccount<'info>,

    // Token-2022 program.

    pub token_program: Program<'info, Token2022>,

    pub system_program: Program<'info, System>,
}

// --------------------------------------------------
// MAIN HANDLER
// --------------------------------------------------

pub fn handler(
    ctx: Context<CreateV1Mint>,
    args: CreateMintArgs,
) -> Result<()> {

    // ----------------------------------------------
    // 1. Validate metadata
    // ----------------------------------------------

    require!(
        args.name.len() <= 32,
        MintError::NameTooLong
    );

    require!(
        args.symbol.len() <= 10,
        MintError::SymbolTooLong
    );

    require!(
        args.uri.len() <= 200,
        MintError::UriTooLong
    );

    let issuer = &ctx.accounts.issuer;
    let mint = &ctx.accounts.mint;
    let token_program = &ctx.accounts.token_program;

    let issuer_key = issuer.key();
    let mint_key = mint.key();
    let token_program_key = token_program.key();

    // Ensure the PDA hasn't already been initialized.

    require!(
        mint.lamports() == 0 && mint.data_is_empty(),
        MintError::MintAlreadyExists
    );

    // ----------------------------------------------
    // 2. Calculate metadata size
    // ----------------------------------------------

    let metadata = TokenMetadata {
        name: args.name.clone(),
        symbol: args.symbol.clone(),
        uri: args.uri.clone(),
        ..Default::default()
    };

    // TokenMetadata is variable-length.
    // Calculate its TLV size separately.

    let metadata_size = metadata.tlv_size_of()?;

    // ----------------------------------------------
    // 3. Calculate fixed extension account length
    // ----------------------------------------------

    let extensions = [
        ExtensionType::TransferFeeConfig,
        ExtensionType::MetadataPointer,
        ExtensionType::DefaultAccountState,
        ExtensionType::MintCloseAuthority,
    ];

    let fixed_size =
        ExtensionType::try_calculate_account_len::<Mint>(
            &extensions
        )?;

    // TokenMetadata cannot be included in
    // try_calculate_account_len because it is
    // variable-length.

    let total_size = fixed_size
        .checked_add(metadata_size)
        .ok_or(MintError::AccountSizeOverflow)?;

    msg!("Fixed mint size: {}", fixed_size);
    msg!("Metadata size: {}", metadata_size);
    msg!("Total mint size: {}", total_size);

    // ----------------------------------------------
    // 4. Calculate rent
    // ----------------------------------------------

    let rent = Rent::get()?;

    let lamports = rent.minimum_balance(total_size);

    // ----------------------------------------------
    // 5. Create mint PDA
    // ----------------------------------------------

    let create_ix = system_instruction::create_account(
        &issuer_key,
        &mint_key,
        lamports,
        total_size as u64,
        &token_program_key,
    );

    let bump = ctx.bumps.mint;

    let signer_seeds: &[&[u8]] = &[
        MINT_SEED,
        &[bump],
    ];

    invoke_signed(
        &create_ix,
        &[
            issuer.to_account_info(),
            mint.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
        &[signer_seeds],
    )?;

    msg!("Mint PDA created");

    // ----------------------------------------------
    // 6. Initialize TransferFeeConfig
    // ----------------------------------------------

    let transfer_fee_ix =
        transfer_fee_ix::initialize_transfer_fee_config(
            &token_program_key,
            &mint_key,
            Some(&issuer_key),
            Some(&issuer_key),
            TRANSFER_FEE_BPS,
            MAX_TRANSFER_FEE,
        )?;

    invoke(
        &transfer_fee_ix,
        &[
            mint.to_account_info(),
            token_program.to_account_info(),
        ],
    )?;

    msg!("TransferFeeConfig initialized");

    // ----------------------------------------------
    // 7. Initialize MetadataPointer
    // ----------------------------------------------

    let metadata_pointer_ix =
        metadata_pointer_ix::initialize(
            &token_program_key,
            &mint_key,
            Some(issuer_key),
            Some(mint_key),
        )?;

    invoke(
        &metadata_pointer_ix,
        &[
            mint.to_account_info(),
            token_program.to_account_info(),
        ],
    )?;

    msg!("MetadataPointer initialized");

    // ----------------------------------------------
    // 8. Initialize DefaultAccountState
    // ----------------------------------------------

    let default_state_ix =
        default_state_ix::initialize_default_account_state(
            &token_program_key,
            &mint_key,
            &AccountState::Frozen,
        )?;

    invoke(
        &default_state_ix,
        &[
            mint.to_account_info(),
            token_program.to_account_info(),
        ],
    )?;

    msg!("DefaultAccountState initialized as Frozen");

    // ----------------------------------------------
    // 9. Initialize MintCloseAuthority
    // ----------------------------------------------

    let close_authority_ix =
        token_ix::initialize_mint_close_authority(
            &token_program_key,
            &mint_key,
            Some(&issuer_key),
        )?;

    invoke(
        &close_authority_ix,
        &[
            mint.to_account_info(),
            token_program.to_account_info(),
        ],
    )?;

    msg!("MintCloseAuthority initialized");

    // ----------------------------------------------
    // 10. Initialize base mint LAST
    // ----------------------------------------------

    let initialize_mint_ix =
        token_ix::initialize_mint2(
            &token_program_key,
            &mint_key,
            &issuer_key,
            Some(&issuer_key),
            RUSD_DECIMALS,
        )?;

    invoke(
        &initialize_mint_ix,
        &[
            mint.to_account_info(),
            token_program.to_account_info(),
        ],
    )?;

    msg!("Base mint initialized");

    // ----------------------------------------------
    // 11. Initialize on-chain TokenMetadata
    // ----------------------------------------------

    token_metadata_initialize(
    CpiContext::new(
        token_program.to_account_info(),
        TokenMetadataInitialize {
            program_id: token_program.key(),
            mint: mint.to_account_info(),
            metadata: mint.to_account_info(),
            mint_authority: issuer.to_account_info(),
            update_authority: issuer.to_account_info(),
        },
    ),
    args.name,
    args.symbol,
    args.uri,
)?;

    msg!("TokenMetadata initialized");

    msg!("--------------------------------");
    msg!("RUSD V1 mint successfully created");
    msg!("Mint address: {}", mint_key);
    msg!("Decimals: {}", RUSD_DECIMALS);
    msg!("Transfer fee: {} bps", TRANSFER_FEE_BPS);
    msg!("Maximum fee: {}", MAX_TRANSFER_FEE);
    msg!("--------------------------------");

    Ok(())
}

// --------------------------------------------------
// ERRORS
// --------------------------------------------------

#[error_code]
pub enum MintError {

    #[msg("Token name is too long")]
    NameTooLong,

    #[msg("Token symbol is too long")]
    SymbolTooLong,

    #[msg("Metadata URI is too long")]
    UriTooLong,

    #[msg("Mint PDA has already been initialized")]
    MintAlreadyExists,

    #[msg("Mint account size overflow")]
    AccountSizeOverflow,
}
