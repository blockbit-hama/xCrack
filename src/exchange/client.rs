use std::sync::Arc;
use std::collections::HashMap;
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use reqwest::{Client, header::HeaderMap};
use serde::Deserialize;
use tokio::sync::RwLock;
use tracing::info;
use alloy::primitives::U256;
use rust_decimal::Decimal;
use std::time::{Duration, Instant};

use crate::types::OrderStatus;

/// Exchange client trait for unified trading interface
#[async_trait]
pub trait ExchangeClient: Send + Sync + std::fmt::Debug {
    /// Get exchange name
    fn name(&self) -> &str;
    
    /// Check if client is connected
    fn is_connected(&self) -> bool;
    
    /// Get average latency in milliseconds
    fn get_average_latency(&self) -> u64;
    
    /// Place buy order
    async fn place_buy_order(&self, symbol: &str, amount: U256, price: Decimal) -> Result<String>;
    
    /// Place sell order
    async fn place_sell_order(&self, symbol: &str, amount: U256, price: Decimal) -> Result<String>;
    
    /// Cancel order
    async fn cancel_order(&self, order_id: &str) -> Result<()>;
    
    /// Get order status
    async fn get_order_status(&self, order_id: &str) -> Result<OrderStatus>;
    
    /// Get current price
    async fn get_current_price(&self, symbol: &str) -> Result<Decimal>;
    
    /// Get account balance
    async fn get_balance(&self, asset: &str) -> Result<Decimal>;
}

/// Binance API client
#[derive(Debug)]
pub struct BinanceClient {
    client: Client,
    api_key: String,
    secret_key: String,
    base_url: String,
    connected: Arc<RwLock<bool>>,
    latency_history: Arc<RwLock<Vec<u64>>>,
    last_request_time: Arc<RwLock<Option<Instant>>>,
}

impl BinanceClient {
    pub fn new(api_key: String, secret_key: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_key,
            secret_key,
            base_url: "https://api.binance.com".to_string(),
            connected: Arc::new(RwLock::new(false)),
            latency_history: Arc::new(RwLock::new(Vec::new())),
            last_request_time: Arc::new(RwLock::new(None)),
        }
    }

    /// Create HMAC signature for Binance API
    fn create_signature(&self, query_string: &str) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        
        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(self.secret_key.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(query_string.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    /// Make authenticated request to Binance API
    async fn make_request<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        endpoint: &str,
        params: Option<HashMap<String, String>>,
    ) -> Result<T> {
        let start_time = Instant::now();
        
        // Rate limiting
        if let Some(last_request) = *self.last_request_time.read().await {
            let elapsed = last_request.elapsed();
            if elapsed < Duration::from_millis(100) { // 10 requests per second limit
                tokio::time::sleep(Duration::from_millis(100) - elapsed).await;
            }
        }
        *self.last_request_time.write().await = Some(Instant::now());

        let mut query_params = params.unwrap_or_default();
        query_params.insert("timestamp".to_string(), chrono::Utc::now().timestamp_millis().to_string());
        
        // Create query string
        let query_string: String = query_params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");
        
        // Create signature
        let signature = self.create_signature(&query_string);
        let signed_query = format!("{}&signature={}", query_string, signature);
        
        // Create headers
        let mut headers = HeaderMap::new();
        headers.insert("X-MBX-APIKEY", self.api_key.parse().unwrap());
        headers.insert("Content-Type", "application/json".parse().unwrap());
        
        let url = format!("{}/api/v3/{}", self.base_url, endpoint);
        let request = match method {
            "GET" => self.client.get(&url).headers(headers).query(&query_params),
            "POST" => self.client.post(&url).headers(headers).body(signed_query),
            _ => return Err(anyhow!("Unsupported HTTP method: {}", method)),
        };

        let response = request.send().await?;
        let latency = start_time.elapsed().as_millis() as u64;
        
        // Update latency history
        let mut history = self.latency_history.write().await;
        history.push(latency);
        if history.len() > 100 {
            history.remove(0);
        }
        
        if response.status().is_success() {
            *self.connected.write().await = true;
            let json: T = response.json().await?;
            Ok(json)
        } else {
            *self.connected.write().await = false;
            let error_text = response.text().await?;
            Err(anyhow!("Binance API error: {}", error_text))
        }
    }
}

#[async_trait]
impl ExchangeClient for BinanceClient {
    fn name(&self) -> &str {
        "Binance"
    }
    
    fn is_connected(&self) -> bool {
        futures::executor::block_on(async { *self.connected.read().await })
    }
    
    fn get_average_latency(&self) -> u64 {
        futures::executor::block_on(async {
            let history = self.latency_history.read().await;
            if history.is_empty() {
                100 // Default latency
            } else {
                history.iter().sum::<u64>() / history.len() as u64
            }
        })
    }
    
    async fn place_buy_order(&self, symbol: &str, amount: U256, price: Decimal) -> Result<String> {
        if crate::mocks::is_mock_mode() {
            // Mock implementation
            let order_id = format!("binance_buy_{}_{}", symbol, chrono::Utc::now().timestamp_millis());
            info!("🎭 Mock Binance buy order: {} {} @ ${}", amount, symbol, price);
            return Ok(order_id);
        }

        let mut params = HashMap::new();
        params.insert("symbol".to_string(), symbol.to_uppercase());
        params.insert("side".to_string(), "BUY".to_string());
        params.insert("type".to_string(), "LIMIT".to_string());
        params.insert("timeInForce".to_string(), "GTC".to_string());
        params.insert("quantity".to_string(), format!("{}", amount.to::<u128>() as f64 / 1e18));
        params.insert("price".to_string(), price.to_string());

        #[derive(Deserialize)]
        struct OrderResponse {
            #[serde(rename = "orderId")]
            order_id: u64,
        }

        let response: OrderResponse = self.make_request("POST", "order", Some(params)).await?;
        Ok(response.order_id.to_string())
    }
    
    async fn place_sell_order(&self, symbol: &str, amount: U256, price: Decimal) -> Result<String> {
        if crate::mocks::is_mock_mode() {
            // Mock implementation
            let order_id = format!("binance_sell_{}_{}", symbol, chrono::Utc::now().timestamp_millis());
            info!("🎭 Mock Binance sell order: {} {} @ ${}", amount, symbol, price);
            return Ok(order_id);
        }

        let mut params = HashMap::new();
        params.insert("symbol".to_string(), symbol.to_uppercase());
        params.insert("side".to_string(), "SELL".to_string());
        params.insert("type".to_string(), "LIMIT".to_string());
        params.insert("timeInForce".to_string(), "GTC".to_string());
        params.insert("quantity".to_string(), format!("{}", amount.to::<u128>() as f64 / 1e18));
        params.insert("price".to_string(), price.to_string());

        #[derive(Deserialize)]
        struct OrderResponse {
            #[serde(rename = "orderId")]
            order_id: u64,
        }

        let response: OrderResponse = self.make_request("POST", "order", Some(params)).await?;
        Ok(response.order_id.to_string())
    }
    
    async fn cancel_order(&self, order_id: &str) -> Result<()> {
        if crate::mocks::is_mock_mode() {
            info!("🎭 Mock Binance cancel order: {}", order_id);
            return Ok(());
        }

        let mut params = HashMap::new();
        params.insert("orderId".to_string(), order_id.to_string());

        let _: serde_json::Value = self.make_request("DELETE", "order", Some(params)).await?;
        Ok(())
    }
    
    async fn get_order_status(&self, order_id: &str) -> Result<OrderStatus> {
        if crate::mocks::is_mock_mode() {
            // Mock - 90% success rate
            if fastrand::f64() > 0.1 {
                return Ok(OrderStatus::Filled);
            } else {
                return Ok(OrderStatus::Cancelled);
            }
        }

        let mut params = HashMap::new();
        params.insert("orderId".to_string(), order_id.to_string());

        #[derive(Deserialize)]
        struct OrderStatusResponse {
            status: String,
        }

        let response: OrderStatusResponse = self.make_request("GET", "order", Some(params)).await?;
        
        match response.status.as_str() {
            "NEW" => Ok(OrderStatus::Pending),
            "PARTIALLY_FILLED" => Ok(OrderStatus::PartiallyFilled),
            "FILLED" => Ok(OrderStatus::Filled),
            "CANCELED" | "REJECTED" | "EXPIRED" => Ok(OrderStatus::Cancelled),
            _ => Ok(OrderStatus::Pending),
        }
    }
    
    async fn get_current_price(&self, symbol: &str) -> Result<Decimal> {
        if crate::mocks::is_mock_mode() {
            // Mock price with some volatility
            let base_price = match symbol.to_uppercase().as_str() {
                "ETHUSDT" => 2000.0,
                "BTCUSDT" => 50000.0,
                "ADAUSDT" => 0.5,
                _ => 100.0,
            };
            let volatility = (fastrand::f64() - 0.5) * 0.02; // ±1% volatility
            let price = base_price * (1.0 + volatility);
            return Ok(Decimal::from_f64_retain(price).unwrap_or_default());
        }

        let mut params = HashMap::new();
        params.insert("symbol".to_string(), symbol.to_uppercase());

        #[derive(Deserialize)]
        struct PriceResponse {
            price: String,
        }

        let response: PriceResponse = self.make_request("GET", "ticker/price", Some(params)).await?;
        Ok(response.price.parse().unwrap_or_default())
    }
    
    async fn get_balance(&self, asset: &str) -> Result<Decimal> {
        if crate::mocks::is_mock_mode() {
            // Mock balance
            return Ok(Decimal::from(10000)); // 10k units
        }

        #[derive(Deserialize)]
        struct AccountResponse {
            balances: Vec<Balance>,
        }

        #[derive(Deserialize)]
        struct Balance {
            asset: String,
            free: String,
        }

        let response: AccountResponse = self.make_request("GET", "account", None).await?;
        
        for balance in response.balances {
            if balance.asset.eq_ignore_ascii_case(asset) {
                return Ok(balance.free.parse().unwrap_or_default());
            }
        }
        
        Ok(Decimal::ZERO)
    }
}

/// Coinbase Pro API client
#[derive(Debug)]
pub struct CoinbaseClient {
    client: Client,
    api_key: String,
    secret_key: String,
    passphrase: String,
    base_url: String,
    connected: Arc<RwLock<bool>>,
    latency_history: Arc<RwLock<Vec<u64>>>,
    last_request_time: Arc<RwLock<Option<Instant>>>,
}

impl CoinbaseClient {
    pub fn new(api_key: String, secret_key: String, passphrase: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_key,
            secret_key,
            passphrase,
            base_url: "https://api.exchange.coinbase.com".to_string(),
            connected: Arc::new(RwLock::new(false)),
            latency_history: Arc::new(RwLock::new(Vec::new())),
            last_request_time: Arc::new(RwLock::new(None)),
        }
    }

    /// Create signature for Coinbase Pro API
    fn create_signature(&self, timestamp: &str, method: &str, path: &str, body: &str) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        use base64::Engine;
        
        type HmacSha256 = Hmac<Sha256>;
        
        let secret_decoded = base64::engine::general_purpose::STANDARD
            .decode(&self.secret_key)
            .expect("Failed to decode secret key");
        
        let mut mac = HmacSha256::new_from_slice(&secret_decoded)
            .expect("HMAC can take key of any size");
        
        let message = format!("{}{}{}{}", timestamp, method, path, body);
        mac.update(message.as_bytes());
        
        base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes())
    }

    async fn make_request<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        endpoint: &str,
        body: Option<&str>,
    ) -> Result<T> {
        let start_time = Instant::now();
        
        // Rate limiting
        if let Some(last_request) = *self.last_request_time.read().await {
            let elapsed = last_request.elapsed();
            if elapsed < Duration::from_millis(100) { // 10 requests per second limit
                tokio::time::sleep(Duration::from_millis(100) - elapsed).await;
            }
        }
        *self.last_request_time.write().await = Some(Instant::now());

        let timestamp = chrono::Utc::now().timestamp().to_string();
        let path = format!("/{}", endpoint);
        let body_str = body.unwrap_or("");
        
        let signature = self.create_signature(&timestamp, method, &path, body_str);
        
        let mut headers = HeaderMap::new();
        headers.insert("CB-ACCESS-KEY", self.api_key.parse().unwrap());
        headers.insert("CB-ACCESS-SIGN", signature.parse().unwrap());
        headers.insert("CB-ACCESS-TIMESTAMP", timestamp.parse().unwrap());
        headers.insert("CB-ACCESS-PASSPHRASE", self.passphrase.parse().unwrap());
        headers.insert("Content-Type", "application/json".parse().unwrap());

        let url = format!("{}{}", self.base_url, path);
        let request = match method {
            "GET" => self.client.get(&url).headers(headers),
            "POST" => self.client.post(&url).headers(headers).body(body_str.to_string()),
            "DELETE" => self.client.delete(&url).headers(headers),
            _ => return Err(anyhow!("Unsupported HTTP method: {}", method)),
        };

        let response = request.send().await?;
        let latency = start_time.elapsed().as_millis() as u64;
        
        // Update latency history
        let mut history = self.latency_history.write().await;
        history.push(latency);
        if history.len() > 100 {
            history.remove(0);
        }

        if response.status().is_success() {
            *self.connected.write().await = true;
            let json: T = response.json().await?;
            Ok(json)
        } else {
            *self.connected.write().await = false;
            let error_text = response.text().await?;
            Err(anyhow!("Coinbase API error: {}", error_text))
        }
    }
}

#[async_trait]
impl ExchangeClient for CoinbaseClient {
    fn name(&self) -> &str {
        "Coinbase"
    }
    
    fn is_connected(&self) -> bool {
        futures::executor::block_on(async { *self.connected.read().await })
    }
    
    fn get_average_latency(&self) -> u64 {
        futures::executor::block_on(async {
            let history = self.latency_history.read().await;
            if history.is_empty() {
                150 // Default latency
            } else {
                history.iter().sum::<u64>() / history.len() as u64
            }
        })
    }
    
    async fn place_buy_order(&self, symbol: &str, amount: U256, price: Decimal) -> Result<String> {
        if crate::mocks::is_mock_mode() {
            let order_id = format!("coinbase_buy_{}_{}", symbol, chrono::Utc::now().timestamp_millis());
            info!("🎭 Mock Coinbase buy order: {} {} @ ${}", amount, symbol, price);
            return Ok(order_id);
        }

        let body = serde_json::json!({
            "size": format!("{}", amount.to::<u128>() as f64 / 1e18),
            "price": price.to_string(),
            "side": "buy",
            "product_id": symbol.to_uppercase().replace("USDT", "-USD"),
            "type": "limit"
        });

        #[derive(Deserialize)]
        struct OrderResponse {
            id: String,
        }

        let response: OrderResponse = self.make_request("POST", "orders", Some(&body.to_string())).await?;
        Ok(response.id)
    }
    
    async fn place_sell_order(&self, symbol: &str, amount: U256, price: Decimal) -> Result<String> {
        if crate::mocks::is_mock_mode() {
            let order_id = format!("coinbase_sell_{}_{}", symbol, chrono::Utc::now().timestamp_millis());
            info!("🎭 Mock Coinbase sell order: {} {} @ ${}", amount, symbol, price);
            return Ok(order_id);
        }

        let body = serde_json::json!({
            "size": format!("{}", amount.to::<u128>() as f64 / 1e18),
            "price": price.to_string(),
            "side": "sell",
            "product_id": symbol.to_uppercase().replace("USDT", "-USD"),
            "type": "limit"
        });

        #[derive(Deserialize)]
        struct OrderResponse {
            id: String,
        }

        let response: OrderResponse = self.make_request("POST", "orders", Some(&body.to_string())).await?;
        Ok(response.id)
    }
    
    async fn cancel_order(&self, order_id: &str) -> Result<()> {
        if crate::mocks::is_mock_mode() {
            info!("🎭 Mock Coinbase cancel order: {}", order_id);
            return Ok(());
        }

        let endpoint = format!("orders/{}", order_id);
        let _: serde_json::Value = self.make_request("DELETE", &endpoint, None).await?;
        Ok(())
    }
    
    async fn get_order_status(&self, order_id: &str) -> Result<OrderStatus> {
        if crate::mocks::is_mock_mode() {
            if fastrand::f64() > 0.15 {
                return Ok(OrderStatus::Filled);
            } else {
                return Ok(OrderStatus::Cancelled);
            }
        }

        let endpoint = format!("orders/{}", order_id);

        #[derive(Deserialize)]
        struct OrderStatusResponse {
            status: String,
        }

        let response: OrderStatusResponse = self.make_request("GET", &endpoint, None).await?;
        
        match response.status.as_str() {
            "open" | "pending" => Ok(OrderStatus::Pending),
            "done" => Ok(OrderStatus::Filled),
            "cancelled" | "rejected" => Ok(OrderStatus::Cancelled),
            _ => Ok(OrderStatus::Pending),
        }
    }
    
    async fn get_current_price(&self, symbol: &str) -> Result<Decimal> {
        if crate::mocks::is_mock_mode() {
            let base_price = match symbol.to_uppercase().as_str() {
                "ETHUSDT" | "ETH-USD" => 2100.0,
                "BTCUSDT" | "BTC-USD" => 52000.0,
                "ADAUSDT" | "ADA-USD" => 0.48,
                _ => 105.0,
            };
            let volatility = (fastrand::f64() - 0.5) * 0.015;
            let price = base_price * (1.0 + volatility);
            return Ok(Decimal::from_f64_retain(price).unwrap_or_default());
        }

        let product_id = symbol.to_uppercase().replace("USDT", "-USD");
        let endpoint = format!("products/{}/ticker", product_id);

        #[derive(Deserialize)]
        struct TickerResponse {
            price: String,
        }

        let response: TickerResponse = self.make_request("GET", &endpoint, None).await?;
        Ok(response.price.parse().unwrap_or_default())
    }
    
    async fn get_balance(&self, asset: &str) -> Result<Decimal> {
        if crate::mocks::is_mock_mode() {
            return Ok(Decimal::from(8000));
        }

        #[derive(Deserialize)]
        struct Account {
            currency: String,
            balance: String,
        }

        let response: Vec<Account> = self.make_request("GET", "accounts", None).await?;
        
        for account in response {
            if account.currency.eq_ignore_ascii_case(asset) {
                return Ok(account.balance.parse().unwrap_or_default());
            }
        }
        
        Ok(Decimal::ZERO)
    }
}

/// Exchange client factory
pub struct ExchangeClientFactory;

impl ExchangeClientFactory {
    pub fn create_binance_client(api_key: String, secret_key: String) -> Arc<dyn ExchangeClient> {
        Arc::new(BinanceClient::new(api_key, secret_key))
    }
    
    pub fn create_coinbase_client(api_key: String, secret_key: String, passphrase: String) -> Arc<dyn ExchangeClient> {
        Arc::new(CoinbaseClient::new(api_key, secret_key, passphrase))
    }
}