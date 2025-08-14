"""
앙상블 예측 모델
LSTM, Transformer, RandomForest, XGBoost를 결합한 다중 모델 예측 시스템
"""

import numpy as np
import pandas as pd
from typing import Dict, List, Tuple, Any, Optional
import joblib
import torch
import torch.nn as nn
from sklearn.ensemble import RandomForestRegressor
from sklearn.metrics import mean_squared_error, mean_absolute_error
import xgboost as xgb
from dataclasses import dataclass
import asyncio
from datetime import datetime, timedelta

from utils.logger import setup_logger
from .lstm_model import LSTMPredictor
from .transformer_model import TransformerPredictor
from .features import FeatureEngineer

logger = setup_logger(__name__)

@dataclass
class ModelPrediction:
    """개별 모델 예측 결과"""
    model_name: str
    direction: float  # -1.0 ~ 1.0
    confidence: float  # 0.0 ~ 1.0
    price_change: float  # 예상 가격 변화율
    time_horizon: int  # 예측 시간 (분)
    features_importance: Dict[str, float]

@dataclass
class EnsemblePrediction:
    """앙상블 최종 예측 결과"""
    symbol: str
    final_direction: float
    final_confidence: float
    final_price_change: float
    model_predictions: List[ModelPrediction]
    ensemble_weights: Dict[str, float]
    prediction_timestamp: datetime

class EnsemblePredictor:
    """앙상블 예측 시스템"""
    
    def __init__(self, config: Dict[str, Any]):
        self.config = config
        self.models = {}
        self.ensemble_weights = {}
        self.feature_engineer = FeatureEngineer()
        self.performance_history = {}
        self.model_metadata = {}
        
        # 모델 설정
        self.model_configs = {
            'lstm': config.get('lstm', {}),
            'transformer': config.get('transformer', {}),
            'random_forest': config.get('random_forest', {}),
            'xgboost': config.get('xgboost', {})
        }
        
        # 앙상블 가중치 초기화 (균등)
        self.ensemble_weights = {
            'lstm': 0.3,
            'transformer': 0.3,
            'random_forest': 0.2,
            'xgboost': 0.2
        }
        
    async def initialize(self):
        """모델들 초기화 및 로드"""
        logger.info("🧠 앙상블 예측 모델 초기화 중...")
        
        try:
            # LSTM 모델
            self.models['lstm'] = LSTMPredictor(self.model_configs['lstm'])
            await self.models['lstm'].initialize()
            
            # Transformer 모델
            self.models['transformer'] = TransformerPredictor(self.model_configs['transformer'])
            await self.models['transformer'].initialize()
            
            # Random Forest 모델
            self.models['random_forest'] = RandomForestRegressor(
                n_estimators=self.model_configs['random_forest'].get('n_estimators', 100),
                max_depth=self.model_configs['random_forest'].get('max_depth', 10),
                random_state=42
            )
            
            # XGBoost 모델
            self.models['xgboost'] = xgb.XGBRegressor(
                n_estimators=self.model_configs['xgboost'].get('n_estimators', 100),
                max_depth=self.model_configs['xgboost'].get('max_depth', 6),
                learning_rate=self.model_configs['xgboost'].get('learning_rate', 0.1),
                random_state=42
            )
            
            # 기존 모델 로드 시도
            await self._load_existing_models()
            
            logger.info("✅ 앙상블 모델 초기화 완료")
            
        except Exception as e:
            logger.error(f"❌ 앙상블 모델 초기화 실패: {e}")
            raise
    
    async def predict(self, market_data: Dict[str, Any], symbol: str) -> EnsemblePrediction:
        """앙상블 예측 수행"""
        try:
            # 특성 엔지니어링
            features = await self.feature_engineer.create_features(market_data, symbol)
            
            if features is None or len(features) == 0:
                raise ValueError("특성 생성 실패")
            
            # 각 모델별 예측 수행
            model_predictions = []
            
            for model_name, model in self.models.items():
                try:
                    prediction = await self._predict_single_model(
                        model_name, model, features, symbol
                    )
                    if prediction:
                        model_predictions.append(prediction)
                except Exception as e:
                    logger.warning(f"{model_name} 모델 예측 실패: {e}")
            
            if not model_predictions:
                raise ValueError("모든 모델 예측 실패")
            
            # 앙상블 가중 평균 계산
            final_prediction = self._combine_predictions(model_predictions, symbol)
            
            return final_prediction
            
        except Exception as e:
            logger.error(f"앙상블 예측 오류: {e}")
            raise
    
    async def _predict_single_model(
        self, 
        model_name: str, 
        model: Any, 
        features: np.ndarray, 
        symbol: str
    ) -> Optional[ModelPrediction]:
        """개별 모델 예측"""
        try:
            if model_name in ['lstm', 'transformer']:
                # 딥러닝 모델
                direction, confidence, price_change = await model.predict(features)
                features_importance = {}  # 딥러닝 모델은 특성 중요도 계산 복잡
            else:
                # 전통적 ML 모델
                if not hasattr(model, 'predict'):
                    # 모델이 학습되지 않은 경우 기본값 반환
                    return ModelPrediction(
                        model_name=model_name,
                        direction=0.0,
                        confidence=0.1,
                        price_change=0.0,
                        time_horizon=60,
                        features_importance={}
                    )
                
                # 2D 특성으로 변환 (sklearn 요구사항)
                if len(features.shape) > 2:
                    features_2d = features.reshape(features.shape[0], -1)
                else:
                    features_2d = features
                
                # 예측 수행
                prediction = model.predict(features_2d[-1:])  # 마지막 데이터포인트 예측
                
                # 결과 해석
                direction = np.clip(prediction[0], -1.0, 1.0)
                confidence = min(0.8, abs(direction))  # 전통적 모델은 보수적 신뢰도
                price_change = direction * 0.02  # 2% 최대 변동 가정
                
                # 특성 중요도 (Random Forest만 지원)
                features_importance = {}
                if hasattr(model, 'feature_importances_'):
                    importance = model.feature_importances_
                    feature_names = self.feature_engineer.get_feature_names()
                    features_importance = dict(zip(feature_names[:len(importance)], importance))
            
            return ModelPrediction(
                model_name=model_name,
                direction=direction,
                confidence=confidence,
                price_change=price_change,
                time_horizon=60,  # 1시간 예측
                features_importance=features_importance
            )
            
        except Exception as e:
            logger.error(f"{model_name} 모델 예측 오류: {e}")
            return None
    
    def _combine_predictions(
        self, 
        model_predictions: List[ModelPrediction], 
        symbol: str
    ) -> EnsemblePrediction:
        """예측 결과 앙상블 결합"""
        # 가중 평균 계산
        total_weight = 0
        weighted_direction = 0
        weighted_confidence = 0
        weighted_price_change = 0
        
        for pred in model_predictions:
            weight = self.ensemble_weights.get(pred.model_name, 0.25)
            total_weight += weight
            
            weighted_direction += pred.direction * weight
            weighted_confidence += pred.confidence * weight
            weighted_price_change += pred.price_change * weight
        
        if total_weight > 0:
            final_direction = weighted_direction / total_weight
            final_confidence = weighted_confidence / total_weight
            final_price_change = weighted_price_change / total_weight
        else:
            final_direction = 0.0
            final_confidence = 0.1
            final_price_change = 0.0
        
        # 신뢰도 조정 (모델 간 일치도 고려)
        directions = [pred.direction for pred in model_predictions]
        direction_std = np.std(directions) if len(directions) > 1 else 0
        
        # 모델들이 일치할수록 신뢰도 증가
        consistency_bonus = max(0, 1.0 - direction_std) * 0.2
        final_confidence = min(1.0, final_confidence + consistency_bonus)
        
        return EnsemblePrediction(
            symbol=symbol,
            final_direction=final_direction,
            final_confidence=final_confidence,
            final_price_change=final_price_change,
            model_predictions=model_predictions,
            ensemble_weights=self.ensemble_weights.copy(),
            prediction_timestamp=datetime.now()
        )
    
    async def update_models(self, feedback_data: Dict[str, Any]):
        """성과 피드백을 통한 모델 업데이트"""
        try:
            logger.info("📈 모델 성과 피드백 처리 중...")
            
            # 성과 데이터 파싱
            predictions_performance = feedback_data.get('predictions', [])
            
            if not predictions_performance:
                logger.warning("성과 피드백 데이터 없음")
                return
            
            # 모델별 성과 계산
            model_scores = {}
            for perf in predictions_performance:
                model_name = perf.get('model_name')
                actual_outcome = perf.get('actual_outcome', 0)
                predicted_outcome = perf.get('predicted_outcome', 0)
                
                if model_name and model_name in self.models:
                    if model_name not in model_scores:
                        model_scores[model_name] = []
                    
                    # MAE 계산
                    error = abs(actual_outcome - predicted_outcome)
                    model_scores[model_name].append(1.0 / (1.0 + error))  # 오차가 적을수록 높은 점수
            
            # 앙상블 가중치 업데이트
            await self._update_ensemble_weights(model_scores)
            
            # 개별 모델 재학습 (비동기)
            asyncio.create_task(self._retrain_models(feedback_data))
            
            logger.info("✅ 모델 업데이트 완료")
            
        except Exception as e:
            logger.error(f"모델 업데이트 오류: {e}")
    
    async def _update_ensemble_weights(self, model_scores: Dict[str, List[float]]):
        """앙상블 가중치 동적 조정"""
        # 각 모델의 평균 성과 계산
        avg_scores = {}
        for model_name, scores in model_scores.items():
            if scores:
                avg_scores[model_name] = np.mean(scores)
        
        if not avg_scores:
            return
        
        # 소프트맥스로 가중치 정규화
        total_score = sum(avg_scores.values())
        if total_score > 0:
            for model_name in self.ensemble_weights:
                if model_name in avg_scores:
                    self.ensemble_weights[model_name] = avg_scores[model_name] / total_score
                else:
                    self.ensemble_weights[model_name] *= 0.9  # 성과 데이터가 없는 모델은 가중치 감소
        
        logger.info(f"앙상블 가중치 업데이트: {self.ensemble_weights}")
    
    async def _retrain_models(self, feedback_data: Dict[str, Any]):
        """모델 재학습 (백그라운드)"""
        try:
            # 재학습 데이터 준비
            training_data = feedback_data.get('training_data', {})
            
            if not training_data:
                logger.warning("재학습용 데이터 없음")
                return
            
            # 각 모델별 재학습
            for model_name, model in self.models.items():
                if model_name in ['lstm', 'transformer']:
                    # 딥러닝 모델 재학습
                    await model.incremental_train(training_data)
                else:
                    # 전통적 ML 모델 재학습
                    await self._retrain_ml_model(model_name, model, training_data)
            
            # 모델 저장
            await self._save_models()
            
            logger.info("모델 재학습 완료")
            
        except Exception as e:
            logger.error(f"모델 재학습 오류: {e}")
    
    async def _retrain_ml_model(self, model_name: str, model: Any, training_data: Dict[str, Any]):
        """전통적 ML 모델 재학습"""
        try:
            # 학습 데이터 준비
            X = np.array(training_data.get('features', []))
            y = np.array(training_data.get('targets', []))
            
            if len(X) < 10:  # 최소 데이터 요구사항
                logger.warning(f"{model_name}: 재학습용 데이터 부족 ({len(X)}개)")
                return
            
            # 2D 특성으로 변환
            if len(X.shape) > 2:
                X = X.reshape(X.shape[0], -1)
            
            # 점진적 학습 또는 전체 재학습
            if hasattr(model, 'partial_fit'):
                model.partial_fit(X, y)
            else:
                model.fit(X, y)
            
            logger.info(f"{model_name} 재학습 완료 ({len(X)}개 샘플)")
            
        except Exception as e:
            logger.error(f"{model_name} 재학습 오류: {e}")
    
    async def calculate_accuracy(self) -> float:
        """전체 모델 정확도 계산"""
        try:
            # 최근 성과 데이터를 기반으로 정확도 계산
            if not self.performance_history:
                return 0.5  # 기본값
            
            recent_performances = []
            cutoff_time = datetime.now() - timedelta(days=7)  # 최근 7일
            
            for timestamp, performance in self.performance_history.items():
                if timestamp > cutoff_time:
                    recent_performances.append(performance)
            
            if not recent_performances:
                return 0.5
            
            # 평균 정확도 계산
            avg_accuracy = np.mean([p.get('accuracy', 0.5) for p in recent_performances])
            return min(1.0, max(0.0, avg_accuracy))
            
        except Exception as e:
            logger.error(f"정확도 계산 오류: {e}")
            return 0.5
    
    async def _load_existing_models(self):
        """기존 저장된 모델 로드"""
        try:
            model_dir = self.config.get('model_save_dir', 'saved_models')
            
            # Random Forest 로드
            try:
                rf_path = f"{model_dir}/random_forest.joblib"
                self.models['random_forest'] = joblib.load(rf_path)
                logger.info("Random Forest 모델 로드 완료")
            except:
                logger.info("Random Forest 모델 없음, 새로 생성")
            
            # XGBoost 로드
            try:
                xgb_path = f"{model_dir}/xgboost.json"
                self.models['xgboost'].load_model(xgb_path)
                logger.info("XGBoost 모델 로드 완료")
            except:
                logger.info("XGBoost 모델 없음, 새로 생성")
            
        except Exception as e:
            logger.warning(f"기존 모델 로드 실패: {e}")
    
    async def _save_models(self):
        """모델 저장"""
        try:
            model_dir = self.config.get('model_save_dir', 'saved_models')
            import os
            os.makedirs(model_dir, exist_ok=True)
            
            # Random Forest 저장
            if hasattr(self.models['random_forest'], 'predict'):
                rf_path = f"{model_dir}/random_forest.joblib"
                joblib.dump(self.models['random_forest'], rf_path)
            
            # XGBoost 저장
            if hasattr(self.models['xgboost'], 'save_model'):
                xgb_path = f"{model_dir}/xgboost.json"
                self.models['xgboost'].save_model(xgb_path)
            
            # 앙상블 가중치 저장
            weights_path = f"{model_dir}/ensemble_weights.json"
            import json
            with open(weights_path, 'w') as f:
                json.dump(self.ensemble_weights, f)
            
            logger.info("모델 저장 완료")
            
        except Exception as e:
            logger.error(f"모델 저장 오류: {e}")

    def get_model_info(self) -> Dict[str, Any]:
        """모델 정보 반환"""
        return {
            "ensemble_weights": self.ensemble_weights,
            "available_models": list(self.models.keys()),
            "model_configs": self.model_configs,
            "performance_history_count": len(self.performance_history)
        }