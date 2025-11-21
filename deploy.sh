# 首先编译出release 程序
cargo build-sbf
# 然后发布到devnet 上
solana program deploy --program-id target/deploy/pinocchio_demo-keypair.json  target/deploy/pinocchio_demo.so --url devnet
