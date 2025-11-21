// instructions.rs
// Pinocchio 演示程序 - 指令处理模块
// 这个文件定义了程序支持的所有指令类型、数据结构和处理逻辑

// 导入 Rust 核心库组件
use core::convert::TryFrom; // 用于实现类型转换 trait
use core::mem::size_of; // 用于获取类型大小

// 导入 pinocchio 框架组件
use pinocchio::{
    account_info::AccountInfo,   // 账户信息结构体，用于访问和操作账户数据
    instruction::{Seed, Signer}, // 指令相关类型：种子和签名者
    program_error::ProgramError, // 程序错误类型
    pubkey::{find_program_address, Pubkey}, // 公钥相关功能：查找程序派生地址和公钥类型
    sysvars::{rent::Rent, Sysvar}, // 系统变量：租金系统和系统变量 trait
    ProgramResult,               // 程序结果类型
};
use pinocchio_log::log; // 日志记录功能
use pinocchio_system::instructions::{CreateAccount, Transfer as SystemTransfer}; // 系统指令：创建账户和转账
use shank::ShankInstruction; // Shank 指令宏，用于生成 IDL

/// Shank IDL facade 枚举，描述所有程序指令及其所需的账户
/// 这个枚举仅用于 IDL（接口定义语言）生成，不会影响运行时行为
/// 它为外部客户端（如前端应用）提供程序的接口定义
#[derive(ShankInstruction)]
pub enum ProgramIx {
    /// 将 lamports 存入 vault（保险库）
    /// 这个指令允许用户将指定数量的 lamports 存入他们专属的 vault
    #[account(0, signer, writable, name = "owner", desc = "Vault owner and payer")]
    #[account(1, writable, name = "vault", desc = "Vault PDA for lamports")]
    #[account(2, name = "program", desc = "Program Address")]
    #[account(3, name = "system_program", desc = "System Program Address")]
    Deposit { amount: u64 },

    /// 将所有 lamports 从 vault 中提取回所有者
    /// 这个指令允许用户从他们的 vault 中提取所有可用的 lamports
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

/// 从指令数据中解析 u64 金额
///
/// # 参数
/// - `data`: 包含金额数据的字节切片
///
/// # 返回值
/// - `Result<u64, ProgramError>`: 成功时返回解析的金额，失败时返回错误
///
/// # 错误情况
/// - 数据长度不等于 u64 类型大小（8字节）
/// - 金额为0（不允许零金额存款）
fn parse_amount(data: &[u8]) -> Result<u64, ProgramError> {
    // 验证数据长度是否正确
    if data.len() != core::mem::size_of::<u64>() {
        return Err(ProgramError::InvalidInstructionData);
    }

    // 将字节数组转换为 u64（小端字节序）
    let amt = u64::from_le_bytes(data.try_into().unwrap());

    // 验证金额不为零
    if amt == 0 {
        return Err(ProgramError::InvalidInstructionData);
    }

    Ok(amt)
}

/// 为所有者派生 vault PDA（程序派生地址）并返回 (pda, bump)
///
/// # 参数
/// - `owner`: 所有者的账户信息
///
/// # 返回值
/// - `(Pubkey, u8)`: 派生出的 vault PDA 和 bump seed
///
/// # 说明
/// - 使用 "vault" 和所有者公钥作为种子来派生 PDA
/// - 确保每个所有者有唯一的 vault 地址
fn derive_vault(owner: &AccountInfo) -> (Pubkey, u8) {
    find_program_address(&[b"vault", owner.key().as_ref()], &crate::ID)
}

/// 确保 vault 存在；如果不存在，则使用 PDA 种子创建它
///
/// # 参数
/// - `owner`: 所有者的账户信息（必须是签名者）
/// - `vault`: vault 的账户信息
///
/// # 返回值
/// - `ProgramResult`: 操作结果
///
/// # 功能
/// - 验证所有者是否为签名者
/// - 如果 vault 不存在（lamports为0），则创建新的 vault 账户
/// - 如果 vault 已存在，验证其所有权是否正确
fn ensure_vault_exists(owner: &AccountInfo, vault: &AccountInfo) -> ProgramResult {
    // 验证所有者是否为签名者
    if !owner.is_signer() {
        return Err(ProgramError::InvalidAccountOwner);
    }

    // 检查 vault 是否为空（不存在）
    if vault.lamports() == 0 {
        // 账户鉴别器大小，用于存储账户类型信息
        const ACCOUNT_DISCRIMINATOR_SIZE: usize = 8;

        // 派生 vault PDA 和 bump seed
        let (_pda, bump) = derive_vault(owner);

        // 创建签名者种子数组
        let signer_seeds = [
            Seed::from(b"vault".as_slice()),
            Seed::from(owner.key().as_ref()),
            Seed::from(core::slice::from_ref(&bump)),
        ];
        let signer = Signer::from(&signer_seeds);

        // 计算 vault 账户所需的大小
        const VAULT_SIZE: usize = ACCOUNT_DISCRIMINATOR_SIZE + size_of::<u64>();

        // 获取免除租金所需的最低 lamports 余额
        let needed_lamports = Rent::get()?.minimum_balance(VAULT_SIZE);

        // 创建 vault 账户
        CreateAccount {
            from: owner,               // 付款账户
            to: vault,                 // 目标账户（vault）
            lamports: needed_lamports, // 初始 lamports 金额
            space: VAULT_SIZE as u64,  // 账户空间大小
            owner: &crate::ID,         // 账户所有者（当前程序）
        }
        .invoke_signed(&[signer])?; // 使用 PDA 签名执行创建操作

        log!("Vault created"); // 记录创建日志
    } else {
        // 如果 vault 已经存在，验证其所有权是否正确
        if !vault.is_owned_by(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }

        log!("Vault already exists"); // 记录存在日志
    }

    Ok(())
}

/// 存款指令结构体
/// 表示一个存款操作，包含相关的账户信息和存款金额
pub struct Deposit<'a> {
    pub owner: &'a AccountInfo, // 存款所有者账户
    pub vault: &'a AccountInfo, // 目标 vault 账户
    pub amount: u64,            // 存款金额（lamports）
}

impl<'a> Deposit<'a> {
    /// 存款指令的标识符（discriminator）
    /// 在指令数据中第一个字节使用 0 表示存款指令
    pub const DISCRIMINATOR: &'a u8 = &0;

    /// 处理存款指令
    ///
    /// # 功能
    /// - 确保 vault 账户存在
    /// - 从所有者账户向 vault 账户转账指定金额
    /// - 记录存款操作日志
    ///
    /// # 返回值
    /// - `ProgramResult`: 操作结果
    pub fn process(self) -> ProgramResult {
        let Deposit {
            owner,
            vault,
            amount,
        } = self;

        // 确保 vault 账户存在（如果不存在则创建）
        ensure_vault_exists(owner, vault)?;

        // 执行系统转账操作
        SystemTransfer {
            from: owner,      // 来源账户（所有者）
            to: vault,        // 目标账户（vault）
            lamports: amount, // 转账金额
        }
        .invoke()?;

        // 记录存款成功日志
        log!("{} Lamports deposited to vault", amount);
        Ok(())
    }
}

/// 为 Deposit 结构体实现 TryFrom trait
/// 允许从原始指令数据和账户信息创建 Deposit 实例
impl<'a> TryFrom<(&'a [u8], &'a [AccountInfo])> for Deposit<'a> {
    type Error = ProgramError;

    /// 从指令数据和账户信息创建 Deposit 实例
    ///
    /// # 参数
    /// - `value`: 包含指令数据字节切片和账户信息切片的元组
    ///
    /// # 返回值
    /// - `Result<Self, Self::Error>`: 成功时返回 Deposit 实例，失败时返回错误
    fn try_from(value: (&'a [u8], &'a [AccountInfo])) -> Result<Self, Self::Error> {
        let (data, accounts) = value;

        // 验证账户数量是否足够
        if accounts.len() < 2 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }

        // 提取账户信息
        let owner = &accounts[0]; // 第一个账户是所有者
        let vault = &accounts[1]; // 第二个账户是 vault

        // 解析存款金额
        let amount = parse_amount(data)?;

        // 创建并返回 Deposit 实例
        Ok(Self {
            owner,
            vault,
            amount,
        })
    }
}

/// 取款指令结构体
/// 表示一个取款操作，包含相关的账户信息
pub struct Withdraw<'a> {
    pub owner: &'a AccountInfo, // 取款所有者账户
    pub vault: &'a AccountInfo, // 来源 vault 账户
}

impl<'a> Withdraw<'a> {
    /// 取款指令的标识符（discriminator）
    /// 在指令数据中第一个字节使用 1 表示取款指令
    pub const DISCRIMINATOR: &'a u8 = &1;

    /// 处理取款指令
    ///
    /// 将 lamports 从 vault PDA 转移到所有者，同时保持账户的租金免除状态
    ///
    /// # 功能
    /// - 验证所有者签名
    /// - 验证 vault 所有权
    /// - 验证 vault PDA 的正确性
    /// - 计算可提取金额（保留租金最低限额）
    /// - 执行 lamports 转移
    /// - 记录取款操作日志
    ///
    /// # 返回值
    /// - `ProgramResult`: 操作结果
    pub fn process(self) -> ProgramResult {
        let Withdraw { owner, vault } = self;

        // 验证所有者是否为签名者
        if !owner.is_signer() {
            return Err(ProgramError::InvalidAccountOwner);
        }

        // 验证 vault 是否归程序所有
        if !vault.is_owned_by(&crate::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }

        // 验证提供的 vault 账户是否是此所有者的正确 PDA
        let (expected_vault_pda, _bump) = derive_vault(owner);
        if vault.key() != &expected_vault_pda {
            return Err(ProgramError::InvalidAccountData);
        }

        // 计算在保持账户免除租金的同时可以提取的金额
        let data_len = vault.data_len(); // vault 账户的数据长度
        let min_balance = Rent::get()?.minimum_balance(data_len); // 租金最低余额
        let current = vault.lamports(); // 当前 vault 余额

        // 检查是否有足够的余额可以提取
        if current <= min_balance {
            // 没有可提取的金额；保持行为严格以避免违反租金规定
            return Err(ProgramError::InsufficientFunds);
        }

        // 计算实际可提取金额（当前余额减去租金最低限额）
        let withdraw_amount = current - min_balance;

        // 从 vault 向所有者转移 lamports
        // 使用作用域来管理 lamports 的可变借用

        // 第一步：从 vault 中扣除金额
        {
            let mut vault_lamports = vault.try_borrow_mut_lamports()?;
            *vault_lamports = vault_lamports
                .checked_sub(withdraw_amount)
                .ok_or(ProgramError::InsufficientFunds)?;
        }

        // 第二步：向所有者账户添加金额
        {
            let mut owner_lamports = owner.try_borrow_mut_lamports()?;
            *owner_lamports = owner_lamports
                .checked_add(withdraw_amount)
                .ok_or(ProgramError::InsufficientFunds)?;
        }

        // 记录取款成功日志
        log!("{} lamports withdrawn from vault", withdraw_amount);
        Ok(())
    }
}

/// 为 Withdraw 结构体实现 TryFrom trait
/// 允许从账户信息创建 Withdraw 实例
impl<'a> TryFrom<&'a [AccountInfo]> for Withdraw<'a> {
    type Error = ProgramError;

    /// 从账户信息创建 Withdraw 实例
    ///
    /// # 参数
    /// - `accounts`: 账户信息切片
    ///
    /// # 返回值
    /// - `Result<Self, Self::Error>`: 成功时返回 Withdraw 实例，失败时返回错误
    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        // 验证账户数量是否足够
        if accounts.len() < 2 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }

        // 提取账户信息
        let owner = &accounts[0]; // 第一个账户是所有者
        let vault = &accounts[1]; // 第二个账户是 vault

        // 创建并返回 Withdraw 实例
        Ok(Self { owner, vault })
    }
}
