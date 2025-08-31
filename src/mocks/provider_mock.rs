use std::sync::Arc;
use anyhow::Result;
use ethers::providers::Provider;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};

/// Mock WebSocket server that provides a minimal WebSocket interface
/// for testing purposes without requiring external connections
pub struct MockWebSocketServer {
    port: u16,
}

impl MockWebSocketServer {
    pub async fn new() -> Result<Self> {
        // Find an available port
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        
        // Spawn the mock WebSocket server
        tokio::spawn(async move {
            if let Err(e) = Self::run_server(listener).await {
                eprintln!("🎭 Mock WebSocket server error: {}", e);
            }
        });
        
        // Give the server a moment to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        Ok(Self { port })
    }
    
    pub fn ws_url(&self) -> String {
        format!("ws://127.0.0.1:{}", self.port)
    }
    
    async fn run_server(listener: TcpListener) -> Result<()> {
        println!("🎭 Mock WebSocket server starting on {}", listener.local_addr()?);
        
        while let Ok((stream, addr)) = listener.accept().await {
            println!("🎭 Mock WebSocket client connected: {}", addr);
            tokio::spawn(Self::handle_connection(stream));
        }
        
        Ok(())
    }
    
    async fn handle_connection(stream: TcpStream) -> Result<()> {
        let ws_stream = accept_async(stream).await?;
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        
        while let Some(message) = ws_receiver.next().await {
            match message? {
                Message::Text(text) => {
                    if let Ok(request) = serde_json::from_str::<Value>(&text) {
                        let response = Self::handle_rpc_request(&request);
                        let response_text = serde_json::to_string(&response)?;
                        ws_sender.send(Message::Text(response_text)).await?;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        
        println!("🎭 Mock WebSocket client disconnected");
        Ok(())
    }
    
    fn handle_rpc_request(request: &Value) -> Value {
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let id = request.get("id").cloned().unwrap_or(json!(1));
        
        match method {
            "eth_blockNumber" => {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": "0x1000000" // Mock block number
                })
            }
            "eth_getBalance" => {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": "0x1000000000000000000" // 1 ETH
                })
            }
            "eth_gasPrice" => {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": "0x4a817c800" // 20 gwei
                })
            }
            "net_version" => {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": "1337" // Mock chain ID
                })
            }
            "eth_chainId" => {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": "0x539" // 1337 in hex
                })
            }
            "eth_subscribe" => {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": "0x1234567890abcdef" // Mock subscription ID
                })
            }
            _ => {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32601,
                        "message": "Method not found"
                    }
                })
            }
        }
    }
}

/// Create a mock WebSocket provider for testing
pub async fn create_mock_ws_provider() -> Result<Arc<Provider<ethers::providers::Ws>>> {
    // Start mock WebSocket server
    let mock_server = MockWebSocketServer::new().await?;
    let ws_url = mock_server.ws_url();
    
    println!("🎭 Connecting to mock WebSocket server at {}", ws_url);
    
    // Connect to our mock server
    let ws = ethers::providers::Ws::connect(&ws_url).await?;
    let provider = Provider::new(ws);
    
    println!("✅ Mock WebSocket provider created successfully");
    
    Ok(Arc::new(provider))
}