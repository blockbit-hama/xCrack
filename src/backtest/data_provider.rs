/// Mock data provider for backtesting
use crate::types::{PriceData, OrderBookSnapshot, Transaction};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Historical data point for backtesting
#[derive(Debug, Clone)]
pub struct HistoricalDataPoint {
    pub timestamp: DateTime<Utc>,
    pub price: f64,
    pub volume: f64,
}

/// Data provider trait for different data sources
pub trait DataProvider: Send + Sync + std::fmt::Debug {
    /// Get historical price data
    async fn get_historical_data(&self, symbol: &str, start: DateTime<Utc>, end: DateTime<Utc>) -> Vec<HistoricalDataPoint>;
    
    /// Get order book snapshots
    async fn get_order_book_history(&self, symbol: &str, start: DateTime<Utc>, end: DateTime<Utc>) -> Vec<OrderBookSnapshot>;
}

/// Historical data source enumeration
#[derive(Debug, Clone)]
pub enum HistoricalDataSource {
    Mock,
    Database,
    External,
}

#[derive(Debug, Clone)]
pub struct MockDataProvider {
    pub prices: HashMap<String, Vec<PriceData>>,
    pub order_books: HashMap<String, Vec<OrderBookSnapshot>>,
    pub transactions: Vec<Transaction>,
    pub current_timestamp: DateTime<Utc>,
}

impl MockDataProvider {
    pub fn new() -> Self {
        Self {
            prices: HashMap::new(),
            order_books: HashMap::new(),
            transactions: Vec::new(),
            current_timestamp: Utc::now(),
        }
    }
    
    /// Get data at specific timestamp for backtesting
    pub async fn get_data_at_time(&self, symbol: &str, timestamp: u64) -> anyhow::Result<HistoricalDataPoint> {
        use anyhow::anyhow;
        
        // Convert timestamp to DateTime
        let target_time = chrono::DateTime::from_timestamp(timestamp as i64, 0)
            .unwrap_or(Utc::now());
        
        // For mock data, generate synthetic price data based on symbol and timestamp
        let base_price = match symbol {
            "WETH/USDC" => 2000.0,
            "WETH/USDT" => 2001.0,
            "WETH/DAI" => 1999.0,
            "WBTC/USDC" => 45000.0,
            "WBTC/USDT" => 45050.0,
            _ => 100.0,
        };
        
        // Add some time-based variation to make it more realistic
        let time_variation = (timestamp % 86400) as f64 / 86400.0; // Daily cycle
        let price_variation = (time_variation.sin() * 0.02) + (fastrand::f64() - 0.5) * 0.01; // +/- 1% random + 2% sine wave
        let price = base_price * (1.0 + price_variation);
        
        // Volume simulation
        let volume = 1000.0 + (fastrand::f64() * 5000.0);
        
        Ok(HistoricalDataPoint {
            timestamp: target_time,
            price,
            volume,
        })
    }
    
    /// Get historical price data for a symbol
    pub fn get_price_data(&self, symbol: &str) -> Option<&Vec<PriceData>> {
        self.prices.get(symbol)
    }
    
    /// Get order book snapshots for a symbol
    pub fn get_order_book_data(&self, symbol: &str) -> Option<&Vec<OrderBookSnapshot>> {
        self.order_books.get(symbol)
    }
    
    /// Add mock price data
    pub fn add_price_data(&mut self, symbol: String, price_data: Vec<PriceData>) {
        self.prices.insert(symbol, price_data);
    }
    
    /// Add mock order book data
    pub fn add_order_book_data(&mut self, symbol: String, order_book_data: Vec<OrderBookSnapshot>) {
        self.order_books.insert(symbol, order_book_data);
    }
    
    /// Add mock transaction
    pub fn add_transaction(&mut self, transaction: Transaction) {
        self.transactions.push(transaction);
    }
    
    /// Set current timestamp for simulation
    pub fn set_timestamp(&mut self, timestamp: DateTime<Utc>) {
        self.current_timestamp = timestamp;
    }
}

impl DataProvider for MockDataProvider {
    async fn get_historical_data(&self, symbol: &str, _start: DateTime<Utc>, _end: DateTime<Utc>) -> Vec<HistoricalDataPoint> {
        // Convert price data to historical data points
        if let Some(price_data) = self.get_price_data(symbol) {
            price_data.iter().map(|pd| HistoricalDataPoint {
                timestamp: pd.timestamp,
                price: pd.last_price.to_string().parse().unwrap_or(0.0),
                volume: pd.volume_24h.to::<u128>() as f64,
            }).collect()
        } else {
            vec![]
        }
    }
    
    async fn get_order_book_history(&self, symbol: &str, _start: DateTime<Utc>, _end: DateTime<Utc>) -> Vec<OrderBookSnapshot> {
        self.get_order_book_data(symbol).cloned().unwrap_or_default()
    }
}

impl Default for MockDataProvider {
    fn default() -> Self {
        Self::new()
    }
}