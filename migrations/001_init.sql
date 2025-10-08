-- xCrack Liquidation Bot Database Schema
-- Version: 1.0.0
-- Description: Initial database schema for liquidation bot

-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Users table: Track all users with lending positions
CREATE TABLE IF NOT EXISTS users (
    address VARCHAR(42) PRIMARY KEY,
    protocol VARCHAR(20) NOT NULL,
    health_factor DECIMAL(10, 4),
    total_collateral_usd DECIMAL(20, 2),
    total_debt_usd DECIMAL(20, 2),
    available_borrows_usd DECIMAL(20, 2),
    liquidation_threshold DECIMAL(5, 4),
    ltv DECIMAL(5, 4),
    is_liquidatable BOOLEAN DEFAULT FALSE,
    priority_score DECIMAL(20, 2),
    first_seen TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_updated TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    scan_count INTEGER DEFAULT 1
);

-- Indexes for users table
CREATE INDEX idx_users_health_factor ON users(health_factor);
CREATE INDEX idx_users_is_liquidatable ON users(is_liquidatable);
CREATE INDEX idx_users_protocol ON users(protocol);
CREATE INDEX idx_users_priority_score ON users(priority_score DESC);
CREATE INDEX idx_users_last_updated ON users(last_updated);

-- Collateral positions table
CREATE TABLE IF NOT EXISTS collateral_positions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_address VARCHAR(42) NOT NULL REFERENCES users(address) ON DELETE CASCADE,
    asset_address VARCHAR(42) NOT NULL,
    asset_symbol VARCHAR(20),
    amount DECIMAL(40, 18),
    usd_value DECIMAL(20, 2),
    liquidation_threshold DECIMAL(5, 4),
    price_usd DECIMAL(20, 8),
    last_updated TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_collateral_user ON collateral_positions(user_address);
CREATE INDEX idx_collateral_asset ON collateral_positions(asset_address);

-- Debt positions table
CREATE TABLE IF NOT EXISTS debt_positions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_address VARCHAR(42) NOT NULL REFERENCES users(address) ON DELETE CASCADE,
    asset_address VARCHAR(42) NOT NULL,
    asset_symbol VARCHAR(20),
    amount DECIMAL(40, 18),
    usd_value DECIMAL(20, 2),
    borrow_rate DECIMAL(10, 6),
    price_usd DECIMAL(20, 8),
    last_updated TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_debt_user ON debt_positions(user_address);
CREATE INDEX idx_debt_asset ON debt_positions(asset_address);

-- Liquidation opportunities table
CREATE TABLE IF NOT EXISTS liquidation_opportunities (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_address VARCHAR(42) NOT NULL,
    protocol VARCHAR(20) NOT NULL,
    health_factor DECIMAL(10, 4),
    estimated_profit_usd DECIMAL(20, 2),
    max_liquidatable_debt_usd DECIMAL(20, 2),
    liquidation_bonus DECIMAL(5, 4),
    status VARCHAR(20) DEFAULT 'pending', -- pending, executing, completed, failed
    priority_score DECIMAL(20, 2),
    detected_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    executed_at TIMESTAMP,
    completed_at TIMESTAMP,
    tx_hash VARCHAR(66),
    error_message TEXT
);

CREATE INDEX idx_opportunities_status ON liquidation_opportunities(status);
CREATE INDEX idx_opportunities_priority ON liquidation_opportunities(priority_score DESC);
CREATE INDEX idx_opportunities_detected ON liquidation_opportunities(detected_at DESC);
CREATE INDEX idx_opportunities_user ON liquidation_opportunities(user_address);

-- Liquidation history table
CREATE TABLE IF NOT EXISTS liquidation_history (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    opportunity_id UUID REFERENCES liquidation_opportunities(id),
    user_address VARCHAR(42) NOT NULL,
    protocol VARCHAR(20) NOT NULL,
    collateral_asset VARCHAR(42),
    debt_asset VARCHAR(42),
    liquidated_debt DECIMAL(40, 18),
    received_collateral DECIMAL(40, 18),
    profit_usd DECIMAL(20, 2),
    gas_used BIGINT,
    gas_price_gwei DECIMAL(10, 2),
    tx_hash VARCHAR(66) UNIQUE,
    block_number BIGINT,
    executed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    success BOOLEAN DEFAULT TRUE
);

CREATE INDEX idx_history_user ON liquidation_history(user_address);
CREATE INDEX idx_history_protocol ON liquidation_history(protocol);
CREATE INDEX idx_history_executed ON liquidation_history(executed_at DESC);
CREATE INDEX idx_history_success ON liquidation_history(success);
CREATE INDEX idx_history_tx_hash ON liquidation_history(tx_hash);

-- Price history table
CREATE TABLE IF NOT EXISTS price_history (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    asset_address VARCHAR(42) NOT NULL,
    asset_symbol VARCHAR(20),
    price_usd DECIMAL(20, 8),
    price_source VARCHAR(20), -- chainlink, thegraph, dex
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_price_asset ON price_history(asset_address);
CREATE INDEX idx_price_timestamp ON price_history(timestamp DESC);

-- Mempool signals table
CREATE TABLE IF NOT EXISTS mempool_signals (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    signal_type VARCHAR(30), -- oracle_update, competitor_liquidation, gas_spike, etc.
    tx_hash VARCHAR(66),
    user_address VARCHAR(42),
    asset_address VARCHAR(42),
    urgency VARCHAR(20), -- low, medium, high, critical
    metadata JSONB,
    detected_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_signals_type ON mempool_signals(signal_type);
CREATE INDEX idx_signals_urgency ON mempool_signals(urgency);
CREATE INDEX idx_signals_detected ON mempool_signals(detected_at DESC);
CREATE INDEX idx_signals_user ON mempool_signals(user_address);

-- Statistics table
CREATE TABLE IF NOT EXISTS statistics (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    metric_name VARCHAR(50) NOT NULL,
    metric_value DECIMAL(20, 2),
    protocol VARCHAR(20),
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_stats_metric ON statistics(metric_name);
CREATE INDEX idx_stats_protocol ON statistics(protocol);
CREATE INDEX idx_stats_timestamp ON statistics(timestamp DESC);

-- Create view for active liquidation opportunities
CREATE OR REPLACE VIEW v_active_opportunities AS
SELECT
    lo.id,
    lo.user_address,
    u.protocol,
    u.health_factor,
    u.total_collateral_usd,
    u.total_debt_usd,
    lo.estimated_profit_usd,
    lo.priority_score,
    lo.status,
    lo.detected_at,
    EXTRACT(EPOCH FROM (CURRENT_TIMESTAMP - lo.detected_at)) AS age_seconds
FROM liquidation_opportunities lo
JOIN users u ON lo.user_address = u.address
WHERE lo.status IN ('pending', 'executing')
  AND u.is_liquidatable = TRUE
ORDER BY lo.priority_score DESC;

-- Create view for user summary
CREATE OR REPLACE VIEW v_user_summary AS
SELECT
    u.address,
    u.protocol,
    u.health_factor,
    u.total_collateral_usd,
    u.total_debt_usd,
    u.is_liquidatable,
    u.priority_score,
    u.last_updated,
    COUNT(DISTINCT cp.id) AS collateral_count,
    COUNT(DISTINCT dp.id) AS debt_count,
    COUNT(DISTINCT lo.id) FILTER (WHERE lo.status = 'completed') AS liquidations_completed,
    COALESCE(SUM(lh.profit_usd), 0) AS total_profit_earned
FROM users u
LEFT JOIN collateral_positions cp ON u.address = cp.user_address
LEFT JOIN debt_positions dp ON u.address = dp.user_address
LEFT JOIN liquidation_opportunities lo ON u.address = lo.user_address
LEFT JOIN liquidation_history lh ON u.address = lh.user_address AND lh.success = TRUE
GROUP BY u.address, u.protocol, u.health_factor, u.total_collateral_usd,
         u.total_debt_usd, u.is_liquidatable, u.priority_score, u.last_updated;

-- Create function to update last_updated timestamp
CREATE OR REPLACE FUNCTION update_last_updated()
RETURNS TRIGGER AS $$
BEGIN
    NEW.last_updated = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create triggers for auto-updating last_updated
CREATE TRIGGER users_update_timestamp
    BEFORE UPDATE ON users
    FOR EACH ROW
    EXECUTE FUNCTION update_last_updated();

CREATE TRIGGER collateral_update_timestamp
    BEFORE UPDATE ON collateral_positions
    FOR EACH ROW
    EXECUTE FUNCTION update_last_updated();

CREATE TRIGGER debt_update_timestamp
    BEFORE UPDATE ON debt_positions
    FOR EACH ROW
    EXECUTE FUNCTION update_last_updated();

-- Insert initial statistics
INSERT INTO statistics (metric_name, metric_value, protocol)
VALUES
    ('total_users_scanned', 0, 'all'),
    ('total_liquidations_executed', 0, 'all'),
    ('total_profit_usd', 0, 'all'),
    ('avg_health_factor', 1.0, 'all')
ON CONFLICT DO NOTHING;

-- Grant permissions (if needed)
-- GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO xcrack;
-- GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO xcrack;
