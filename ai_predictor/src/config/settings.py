"""
AI 예측 시스템 설정 관리
"""

import yaml
import os
from dataclasses import dataclass, field
from typing import Dict, Any, List
from pathlib import Path

@dataclass
class DataConfig:
    """데이터 수집 설정"""
    collection_interval: int = 5  # 초
    symbols: List[str] = field(default_factory=lambda: ["ETH/USDT", "BTC/USDT"])
    exchanges: List[str] = field(default_factory=lambda: ["binance", "uniswap"])
    mempool_sources: List[str] = field(default_factory=lambda: ["flashbots", "blocknative"])
    historical_days: int = 30

@dataclass
class AnalysisConfig:
    """시장 분석 설정"""
    technical_indicators: List[str] = field(default_factory=lambda: ["rsi", "macd", "bollinger"])
    lookback_periods: List[int] = field(default_factory=lambda: [14, 30, 60])
    volatility_window: int = 20
    correlation_threshold: float = 0.7

@dataclass
class MEVConfig:
    """MEV 탐지 설정"""
    min_confidence: float = 0.8
    min_profit_threshold: float = 0.01  # 1%
    gas_price_multiplier: float = 1.2
    block_time_estimate: int = 12  # 초
    opportunity_types: List[str] = field(default_factory=lambda: ["sandwich", "arbitrage", "liquidation"])

@dataclass
class ModelConfig:
    """모델 설정"""
    lstm: Dict[str, Any] = field(default_factory=lambda: {
        "hidden_size": 128,
        "num_layers": 2,
        "dropout": 0.2,
        "sequence_length": 60
    })
    transformer: Dict[str, Any] = field(default_factory=lambda: {
        "d_model": 128,
        "nhead": 8,
        "num_layers": 6,
        "dropout": 0.1
    })
    random_forest: Dict[str, Any] = field(default_factory=lambda: {
        "n_estimators": 100,
        "max_depth": 10,
        "min_samples_split": 5
    })
    xgboost: Dict[str, Any] = field(default_factory=lambda: {
        "n_estimators": 100,
        "max_depth": 6,
        "learning_rate": 0.1
    })
    model_save_dir: str = "saved_models"
    retrain_interval_hours: int = 24

@dataclass
class PredictionConfig:
    """예측 설정"""
    interval_seconds: int = 10
    confidence_threshold: float = 0.7
    max_predictions_per_symbol: int = 10
    prediction_horizons: List[int] = field(default_factory=lambda: [5, 15, 30, 60])  # 분

@dataclass
class CommunicationConfig:
    """통신 설정"""
    host: str = "localhost"
    port: int = 8080
    protocol: str = "websocket"  # websocket, redis, tcp
    reconnect_interval: int = 5
    heartbeat_interval: int = 30
    redis_url: str = "redis://localhost:6379"
    tcp_keepalive: bool = True

@dataclass
class Settings:
    """전체 설정"""
    data: DataConfig = field(default_factory=DataConfig)
    analysis: AnalysisConfig = field(default_factory=AnalysisConfig)
    mev: MEVConfig = field(default_factory=MEVConfig)
    models: ModelConfig = field(default_factory=ModelConfig)
    prediction: PredictionConfig = field(default_factory=PredictionConfig)
    communication: CommunicationConfig = field(default_factory=CommunicationConfig)
    
    @classmethod
    def load(cls, config_path: str) -> 'Settings':
        """YAML 설정 파일 로드"""
        try:
            config_file = Path(config_path)
            if not config_file.exists():
                # 기본 설정 파일 생성
                settings = cls()
                settings.save(config_path)
                return settings
            
            with open(config_file, 'r', encoding='utf-8') as f:
                config_dict = yaml.safe_load(f)
            
            if not config_dict:
                return cls()
            
            # 설정 객체 생성
            settings = cls()
            
            # 각 섹션별 설정 적용
            if 'data' in config_dict:
                settings.data = DataConfig(**config_dict['data'])
            
            if 'analysis' in config_dict:
                settings.analysis = AnalysisConfig(**config_dict['analysis'])
                
            if 'mev' in config_dict:
                settings.mev = MEVConfig(**config_dict['mev'])
                
            if 'models' in config_dict:
                settings.models = ModelConfig(**config_dict['models'])
                
            if 'prediction' in config_dict:
                settings.prediction = PredictionConfig(**config_dict['prediction'])
                
            if 'communication' in config_dict:
                settings.communication = CommunicationConfig(**config_dict['communication'])
            
            return settings
            
        except Exception as e:
            print(f"설정 로드 실패: {e}, 기본 설정 사용")
            return cls()
    
    def save(self, config_path: str):
        """설정을 YAML 파일로 저장"""
        try:
            config_dict = {
                'data': self.data.__dict__,
                'analysis': self.analysis.__dict__,
                'mev': self.mev.__dict__,
                'models': self.models.__dict__,
                'prediction': self.prediction.__dict__,
                'communication': self.communication.__dict__
            }
            
            config_file = Path(config_path)
            config_file.parent.mkdir(parents=True, exist_ok=True)
            
            with open(config_file, 'w', encoding='utf-8') as f:
                yaml.dump(config_dict, f, default_flow_style=False, allow_unicode=True)
                
        except Exception as e:
            print(f"설정 저장 실패: {e}")
    
    def validate(self) -> bool:
        """설정 유효성 검사"""
        try:
            # 필수 값 확인
            assert self.communication.host, "호스트가 설정되지 않음"
            assert self.communication.port > 0, "포트가 유효하지 않음"
            assert 0 <= self.prediction.confidence_threshold <= 1, "신뢰도 임계값이 유효하지 않음"
            assert 0 <= self.mev.min_confidence <= 1, "MEV 신뢰도 임계값이 유효하지 않음"
            
            # 모델 설정 확인
            assert self.models.lstm["hidden_size"] > 0, "LSTM 은닉 크기가 유효하지 않음"
            assert self.models.transformer["d_model"] > 0, "Transformer 모델 크기가 유효하지 않음"
            
            return True
            
        except AssertionError as e:
            print(f"설정 검증 실패: {e}")
            return False