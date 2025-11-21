// lib.rs
// Pinocchio 演示程序 - 主程序入口文件
// 这个文件定义了 Solana 程序的入口点和主要指令处理逻辑

// 禁用标准库，因为 Solana 程序运行在受限环境中
#![no_std]
// 允许意外的配置值，用于抑制 pinocchio crate 的警告
#![allow(unexpected_cfgs)]

// 导入必要的 pinocchio 库组件
use pinocchio::{
    account_info::AccountInfo,   // 账户信息结构体，用于访问和操作账户数据
    entrypoint,                  // 程序入口点宏，定义 Solana 程序的入口函数
    program_error::ProgramError, // 程序错误类型，用于处理程序执行中的错误
    pubkey::Pubkey,              // 公钥类型，用于表示 Solana 账户地址
    ProgramResult,               // 程序结果类型，表示程序执行的结果（成功或错误）
};
use pinocchio_pubkey::declare_id; // 声明程序 ID 的宏，用于定义程序的唯一标识符

// 设置程序入口点
// 这个宏将 process_instruction 函数标记为 Solana 程序的入口点
// 当程序被调用时，Solana 运行时会执行这个函数
entrypoint!(process_instruction);

// 导入指令模块
// instructions 模块包含程序支持的所有指令和相关的数据结构
pub mod instructions;
// 重新导出指令模块中的所有公共项，方便外部使用
pub use instructions::*;

// 声明程序的唯一标识符
// 这个 ID 用于在 Solana 区块链上唯一标识这个程序
// 在部署程序之前需要生成一个新的 ID
declare_id!("GMYuTSUDK5psTjN45KTCWrMNfSdDbRHdnY1zzpgVDYgG");

/// 程序的主要指令处理函数
///
/// 这个函数是 Solana 程序的入口点，负责：
/// 1. 解析传入的指令数据
/// 2. 根据指令类型分发到相应的处理逻辑
/// 3. 执行相应的操作并返回结果
///
/// # 参数
/// - `program_id`: 当前程序的公钥，用于验证程序所有权
/// - `accounts`: 账户信息切片，包含指令操作所需的所有账户
/// - `instruction_data`: 指令数据字节切片，包含具体的指令类型和参数
///
/// # 返回值
/// - `ProgramResult`: 程序执行结果，成功时返回 `Ok(())`，失败时返回相应的错误
///
/// # 指令分发逻辑
/// - 指令数据的第一个字节作为指令标识符（discriminator）
/// - 0: 存款指令 (Deposit)
/// - 1: 取款指令 (Withdraw)
/// - 其他: 无效指令数据错误
fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // 使用 split_first 方法分离指令标识符和剩余数据
    match instruction_data.split_first() {
        // 处理存款指令：指令标识符为 0
        Some((Deposit::DISCRIMINATOR, data)) => {
            // 从数据和账户信息创建 Deposit 结构体并执行处理
            Deposit::try_from((data, accounts))?.process()
        }
        // 处理取款指令：指令标识符为 1
        Some((Withdraw::DISCRIMINATOR, _)) => {
            // 从账户信息创建 Withdraw 结构体并执行处理
            Withdraw::try_from(accounts)?.process()
        }
        // 处理无效指令：指令标识符不在支持范围内
        _ => Err(ProgramError::InvalidInstructionData),
    }
}
