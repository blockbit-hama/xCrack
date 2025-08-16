pub mod traits;
pub mod stargate;
pub mod hop;
pub mod rubic;
pub mod synapse;
pub mod lifi;
pub mod across;
pub mod multichain;
pub mod manager;

// Re-exports
pub use traits::{Bridge, BridgeQuote, BridgeError, BridgeResult};
pub use stargate::StargateBridge;
pub use hop::HopBridge;
pub use rubic::RubicBridge;
pub use synapse::SynapseBridge;
pub use lifi::LiFiBridge;
pub use across::AcrossBridge;
pub use multichain::MultichainBridge;
pub use manager::{BridgeManager, RouteStrategy};