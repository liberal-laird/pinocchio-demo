use core::convert::TryFrom;
use core::mem::size_of;
use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    program_error::ProgramError,
    pubkey::{find_program_address, Pubkey},
    sysvars::{rent::Rent, Sysvar},
    ProgramResult,
};
use pinocchio_log::log;
use pinocchio_system::instructions::{CreateAccount, Transfer as SystemTransfer};
use shank::ShankInstruction;

// instructions.rs
/// Shank IDL facade enum describing all program instructions and their required accounts.
/// This is used only for IDL generation and does not affect runtime behavior.
// / Shank IDL facade 枚举，描述所有程序指令及其所需的帐户。
// / 这仅用于 IDL 生成，不会影响运行时行为。
#[derive(ShankInstruction)]
pub enum ProgramIx {
    /// Deposit lamports into the vault.
    // / 将 lamports 存入 vault。
    #[account(0, signer, writable, name = "owner", desc = "Vault owner and payer")]
    #[account(1, writable, name = "vault", desc = "Vault PDA for lamports")]
    #[account(2, name = "program", desc = "Program Address")]
    #[account(3, name = "system_program", desc = "System Program Address")]
    Deposit { amount: u64 },

    /// Withdraw all lamports from the vault back to the owner.
    // / 将所有 lamports 从 vault 中提取回所有者。
    #[account(
        0,
        signer,
        writable,
        name = "owner",
        desc = "Vault owner and authority"
    )]
    #[account(1, writable, name = "vault", desc = "Vault PDA for lamports")]
    #[account(2, name = "program", desc = "Program Address")]
    Withdraw {},
}


// instructions.rs
/// Parse a u64 from instruction data.
// / 从指令数据中解析 u64。
fn parse_amount(data: &[u8]) -> Result<u64, ProgramError> {
    if data.len() != core::mem::size_of::<u64>() {
        return Err(ProgramError::InvalidInstructionData);
    }
    let amt = u64::from_le_bytes(data.try_into().unwrap());
    if amt == 0 {
        return Err(ProgramError::InvalidInstructionData);
    }
    Ok(amt)
}

/// Derive the vault PDA for an owner and return (pda, bump).
// / 为所有者派生 vault PDA 并返回 (pda, bump)。
fn derive_vault(owner: &AccountInfo) -> (Pubkey, u8) {
    find_program_address(&[b"vault", owner.key().as_ref()], &crate::ID)
}

/// Ensure the vault exists; if not, create it with PDA seeds.
// / 确保 vault 存在；如果不存在，则使用 PDA 种子创建它。
fn ensure_vault_exists(owner: &AccountInfo, vault: &AccountInfo) -> ProgramResult {
    if !owner.is_signer() {
        return Err(ProgramError::InvalidAccountOwner);
    }

    // Create when empty and fund rent-exempt.
    // / 在为空时创建并免除租金。
    if vault.lamports() == 0 {

        const ACCOUNT_DISCRIMINATOR_SIZE: usize = 8;

        let (_pda, bump) = derive_vault(owner);
        let signer_seeds = [\
            Seed::from(b"vault".as_slice()),\
            Seed::from(owner.key().as_ref()),\
            Seed::from(core::slice::from_ref(&bump)),\
        ];
        let signer = Signer::from(&signer_seeds);

        // Make the account rent-exempt.
        // / 使帐户免除租金。
        const VAULT_SIZE: usize = ACCOUNT_DISCRIMINATOR_SIZE + size_of::<u64>();
        let needed_lamports = Rent::get()?.minimum_balance(VAULT_SIZE);

        CreateAccount {
            from: owner,
            to: vault,
            lamports: needed_lamports,
            space: VAULT_SIZE as u64,
            owner: &crate::ID,
        }
        .invoke_signed(&[signer])?;

        log!("Vault created");

    } else {
        // If vault already exists, validate owner matches the program.
        // / 如果 vault 已经存在，则验证所有者是否与程序匹配。
        if !vault.is_owned_by(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }

        log!("Vault already exists");
    }

    Ok(())
}
// instructions.rs
pub struct Deposit<'a> {
    pub owner: &'a AccountInfo,
    pub vault: &'a AccountInfo,
    pub amount: u64,
}

impl<'a> Deposit<'a> {
    pub const DISCRIMINATOR: &'a u8 = &0;

    pub fn process(self) -> ProgramResult {
        let Deposit {
            owner,
            vault,
            amount,
        } = self;

        ensure_vault_exists(owner, vault)?;

        SystemTransfer {
            from: owner,
            to: vault,
            lamports: amount,
        }
        .invoke()?;
        log!("{} Lamports deposited to vault", amount);
        Ok(())
    }
}

impl<'a> TryFrom<(&'a [u8], &'a [AccountInfo])> for Deposit<'a> {
    type Error = ProgramError;

    fn try_from(value: (&'a [u8], &'a [AccountInfo])>) -> Result<Self, Self::Error> {
        let (data, accounts) = value;
        if accounts.len() < 2 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let owner = &accounts[0];
        let vault = &accounts[1];
        let amount = parse_amount(data)?;
        Ok(Self {
            owner,
            vault,
            amount,
        })
    }
}


// instructions.rs
pub struct Withdraw<'a> {
    pub owner: &'a AccountInfo,
    pub vault: &'a AccountInfo,
}

impl<'a> Withdraw<'a> {
    pub const DISCRIMINATOR: &'a u8 = &1;

    /// Transfer lamports from the vault PDA to the owner, leaving the rent minimum in place.
    // / 将 lamports 从 vault PDA 转移到所有者，并将租金最低限额留在原位。
    pub fn process(self) -> ProgramResult {
        let Withdraw { owner, vault } = self;
        if !owner.is_signer() {
            return Err(ProgramError::InvalidAccountOwner);
        }

        // Validate that the vault is owned by the program
        // / 验证 vault 是否归程序所有
        if !vault.is_owned_by(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }

        // Validate that the provided vault account is the correct PDA for this owner
        // / 验证提供的 vault 帐户是否是此所有者的正确 PDA
        let (expected_vault_pda, _bump) = derive_vault(owner);
        if vault.key() != &expected_vault_pda {
            return Err(ProgramError::InvalidAccountData);
        }

        // Compute how much can be withdrawn while keeping the account rent-exempt
        // / 计算在保持帐户免除租金的同时可以提取多少
        let data_len = vault.data_len();
        let min_balance = Rent::get()?.minimum_balance(data_len);
        let current = vault.lamports();
        if current <= min_balance {
            // Nothing withdrawable; keep behavior strict to avoid rent violations
            // / 没有可提取的东西；保持行为严格，以避免违反租金
            return Err(ProgramError::InsufficientFunds);
        }
        let withdraw_amount = current - min_balance;

        // Transfer from vault to owner
        // / 从 vault 转移到所有者
        {
            let mut vault_lamports = vault.try_borrow_mut_lamports()?;
            *vault_lamports = vault_lamports
                .checked_sub(withdraw_amount)
                .ok_or(ProgramError::InsufficientFunds)?;
        }

        {
            let mut owner_lamports = owner.try_borrow_mut_lamports()?;
            *owner_lamports = owner_lamports
                .checked_add(withdraw_amount)
                .ok_or(ProgramError::InsufficientFunds)?;
        }

        log!("{} lamports withdrawn from vault", withdraw_amount);
        Ok(())
    }
}

impl<'a> TryFrom<&'a [AccountInfo]> for Withdraw<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        if accounts.len() < 2 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let owner = &accounts[0];
        let vault = &accounts[1];
        Ok(Self { owner, vault })
    }
}
