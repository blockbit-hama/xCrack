pub mod traits;
pub mod stargate;
pub mod hop;
pub mod rubic;
pub mod synapse;
pub mod manager;

// Re-exports
pub use traits::{Bridge, BridgeQuote, BridgeError, BridgeResult};
pub use stargate::StargateBridge;
pub use hop::HopBridge;
pub use rubic::RubicBridge;
pub use synapse::SynapseBridge;
pub use manager::{BridgeManager, RouteStrategy};