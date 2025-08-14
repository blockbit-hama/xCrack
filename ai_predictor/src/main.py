#!/usr/bin/env python3
"""
AI 예측 시스템 메인 엔트리포인트
xCrack과 연동하여 실시간 시장 예측 및 MEV 기회 탐지
"""

import asyncio
import os
import signal
import sys
from typing import Dict, Any
import json
from datetime import datetime
from pathlib import Path

# Add src to path
sys.path.append(str(Path(__file__).parent))

from config.settings import Settings
from core.prediction_engine import PredictionEngine
from core.market_analyzer import MarketAnalyzer
from core.mev_detector import MEVDetector
from communication.rust_bridge import RustBridge
from data.market_data_collector import MarketDataCollector
from models.ensemble_predictor import EnsemblePredictor
from utils.logger import setup_logger

logger = setup_logger(__name__)

class AIPredictorSystem:
    """AI 예측 시스템 메인 클래스"""
    
    def __init__(self, config_path: str = "config/settings.yaml"):
        self.settings = Settings.load(config_path)
        self.running = False
        
        # 핵심 컴포넌트 초기화
        self.market_collector = MarketDataCollector(self.settings.data)
        self.market_analyzer = MarketAnalyzer(self.settings.analysis)
        self.mev_detector = MEVDetector(self.settings.mev)
        self.ensemble_predictor = EnsemblePredictor(self.settings.models)
        self.prediction_engine = PredictionEngine(
            self.market_analyzer,
            self.mev_detector,
            self.ensemble_predictor,
            self.settings.prediction
        )
        
        # Rust 브리지 초기화
        self.rust_bridge = RustBridge(
            host=self.settings.communication.host,
            port=self.settings.communication.port,
            protocol=self.settings.communication.protocol
        )
        
        # 성능 메트릭
        self.metrics = {
            "predictions_made": 0,
            "mev_opportunities_detected": 0,
            "accuracy_score": 0.0,
            "uptime_seconds": 0,
            "last_prediction_time": None
        }
        
    async def initialize(self):
        """시스템 초기화"""
        logger.info("🤖 AI 예측 시스템 초기화 중...")
        
        try:
            # 컴포넌트 초기화
            await self.market_collector.initialize()
            await self.market_analyzer.initialize()
            await self.mev_detector.initialize()
            await self.ensemble_predictor.initialize()
            await self.prediction_engine.initialize()
            
            # Rust 연결 설정
            await self.rust_bridge.connect()
            
            logger.info("✅ AI 예측 시스템 초기화 완료")
            return True
            
        except Exception as e:
            logger.error(f"❌ 시스템 초기화 실패: {e}")
            return False
    
    async def start(self):
        """메인 실행 루프 시작"""
        if not await self.initialize():
            return False
        
        self.running = True
        logger.info("🚀 AI 예측 시스템 시작")
        
        # 백그라운드 태스크들 시작
        tasks = [
            asyncio.create_task(self.prediction_loop()),
            asyncio.create_task(self.market_data_loop()),
            asyncio.create_task(self.mev_detection_loop()),
            asyncio.create_task(self.model_update_loop()),
            asyncio.create_task(self.metrics_reporting_loop()),
            asyncio.create_task(self.rust_communication_loop())
        ]
        
        try:
            await asyncio.gather(*tasks)
        except KeyboardInterrupt:
            logger.info("종료 신호 수신...")
        finally:
            await self.stop()
    
    async def stop(self):
        """시스템 안전 종료"""
        logger.info("🛑 AI 예측 시스템 종료 중...")
        self.running = False
        
        # 연결 종료
        await self.rust_bridge.disconnect()
        await self.market_collector.close()
        
        logger.info("✅ AI 예측 시스템 종료 완료")
    
    async def prediction_loop(self):
        """메인 예측 루프"""
        while self.running:
            try:
                start_time = datetime.now()
                
                # 시장 데이터 수집
                market_data = await self.market_collector.get_latest_data()
                
                if not market_data:
                    await asyncio.sleep(1)
                    continue
                
                # 예측 실행
                predictions = await self.prediction_engine.generate_predictions(market_data)
                
                # Rust로 예측 결과 전송
                for prediction in predictions:
                    await self.rust_bridge.send_prediction(prediction)
                
                # 메트릭 업데이트
                self.metrics["predictions_made"] += len(predictions)
                self.metrics["last_prediction_time"] = datetime.now()
                
                # 예측 주기 대기
                prediction_time = (datetime.now() - start_time).total_seconds()
                sleep_time = max(0, self.settings.prediction.interval_seconds - prediction_time)
                await asyncio.sleep(sleep_time)
                
            except Exception as e:
                logger.error(f"예측 루프 오류: {e}")
                await asyncio.sleep(5)
    
    async def market_data_loop(self):
        """시장 데이터 수집 루프"""
        while self.running:
            try:
                # 실시간 시장 데이터 수집
                await self.market_collector.collect_realtime_data()
                await asyncio.sleep(self.settings.data.collection_interval)
                
            except Exception as e:
                logger.error(f"시장 데이터 수집 오류: {e}")
                await asyncio.sleep(10)
    
    async def mev_detection_loop(self):
        """MEV 기회 탐지 루프"""
        while self.running:
            try:
                # 멤풀 데이터 분석
                mempool_data = await self.market_collector.get_mempool_data()
                
                if mempool_data:
                    # MEV 기회 탐지
                    mev_opportunities = await self.mev_detector.detect_opportunities(mempool_data)
                    
                    for opportunity in mev_opportunities:
                        # 고신뢰도 MEV 기회만 전송
                        if opportunity.confidence > self.settings.mev.min_confidence:
                            await self.rust_bridge.send_mev_opportunity(opportunity)
                            self.metrics["mev_opportunities_detected"] += 1
                
                await asyncio.sleep(0.1)  # 100ms 간격으로 고속 탐지
                
            except Exception as e:
                logger.error(f"MEV 탐지 오류: {e}")
                await asyncio.sleep(1)
    
    async def model_update_loop(self):
        """모델 업데이트 및 재학습 루프"""
        while self.running:
            try:
                # 성과 피드백 수집
                feedback_data = await self.rust_bridge.get_performance_feedback()
                
                if feedback_data:
                    # 모델 성능 평가 및 업데이트
                    await self.ensemble_predictor.update_models(feedback_data)
                    
                    # 정확도 계산
                    accuracy = await self.ensemble_predictor.calculate_accuracy()
                    self.metrics["accuracy_score"] = accuracy
                
                # 1시간마다 모델 업데이트
                await asyncio.sleep(3600)
                
            except Exception as e:
                logger.error(f"모델 업데이트 오류: {e}")
                await asyncio.sleep(600)  # 오류시 10분 대기
    
    async def metrics_reporting_loop(self):
        """성능 메트릭 리포팅 루프"""
        while self.running:
            try:
                # 시스템 상태 로그
                logger.info(f"📊 시스템 메트릭:")
                logger.info(f"  예측 수행: {self.metrics['predictions_made']}")
                logger.info(f"  MEV 기회 탐지: {self.metrics['mev_opportunities_detected']}")
                logger.info(f"  예측 정확도: {self.metrics['accuracy_score']:.3f}")
                
                # 메트릭을 Rust로 전송
                await self.rust_bridge.send_metrics(self.metrics)
                
                # 5분마다 리포팅
                await asyncio.sleep(300)
                
            except Exception as e:
                logger.error(f"메트릭 리포팅 오류: {e}")
                await asyncio.sleep(300)
    
    async def rust_communication_loop(self):
        """Rust와의 통신 관리 루프"""
        while self.running:
            try:
                # 연결 상태 확인
                if not await self.rust_bridge.is_connected():
                    logger.warning("Rust 연결 끊어짐, 재연결 시도...")
                    await self.rust_bridge.reconnect()
                
                # 헬스체크
                await self.rust_bridge.send_heartbeat()
                
                await asyncio.sleep(30)  # 30초마다 체크
                
            except Exception as e:
                logger.error(f"Rust 통신 오류: {e}")
                await asyncio.sleep(10)

def signal_handler(signum, frame):
    """시그널 핸들러"""
    logger.info(f"시그널 {signum} 수신, 종료 중...")
    sys.exit(0)

async def main():
    """메인 함수"""
    # 시그널 핸들러 등록
    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)
    
    # 환경 변수 로드
    from dotenv import load_dotenv
    load_dotenv()
    
    # 설정 파일 경로
    config_path = os.getenv("CONFIG_PATH", "config/settings.yaml")
    
    # AI 예측 시스템 시작
    predictor_system = AIPredictorSystem(config_path)
    
    try:
        await predictor_system.start()
    except Exception as e:
        logger.error(f"시스템 실행 오류: {e}")
        sys.exit(1)

if __name__ == "__main__":
    print("""
    ╔═══════════════════════════════════════════════════════════════╗
    ║                                                               ║
    ║  🤖 xCrack AI 예측 시스템 v1.0.0                            ║
    ║                                                               ║
    ║  실시간 시장 분석 및 MEV 기회 탐지                           ║
    ║                                                               ║
    ║  🧠 핵심 기능:                                               ║
    ║     • 다중 모델 앙상블 예측                                 ║
    ║     • 실시간 멤풀 분석                                       ║
    ║     • MEV 기회 탐지                                          ║
    ║     • Rust xCrack 연동                                      ║
    ║                                                               ║
    ║  ⚡ AI 모델:                                                ║
    ║     • LSTM 시계열 예측                                      ║
    ║     • Transformer 어텐션 모델                              ║
    ║     • Random Forest 특성 기반                               ║
    ║     • XGBoost 그래디언트 부스팅                            ║
    ║                                                               ║
    ╚═══════════════════════════════════════════════════════════════╝
    """)
    
    # asyncio 실행
    asyncio.run(main())