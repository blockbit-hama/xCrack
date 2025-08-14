"""
Rust xCrack과의 통신 브리지
WebSocket/TCP/Redis를 통한 실시간 데이터 교환
"""

import asyncio
import json
import time
from typing import Dict, Any, List, Optional, Union
from dataclasses import dataclass, asdict
from enum import Enum
import websockets
import redis.asyncio as aioredis
from utils.logger import setup_logger

logger = setup_logger(__name__)

class CommunicationProtocol(Enum):
    """통신 프로토콜 타입"""
    WEBSOCKET = "websocket"
    REDIS = "redis"
    TCP = "tcp"

@dataclass
class PredictionMessage:
    """예측 메시지 구조"""
    symbol: str
    direction: float  # -1.0 ~ 1.0
    confidence: float  # 0.0 ~ 1.0
    time_horizon: int  # minutes
    expected_move: float  # percentage
    timestamp: int
    strategy_type: str
    strategy_params: Dict[str, Any]
    model_version: str
    features_used: List[str]

@dataclass
class MEVOpportunityMessage:
    """MEV 기회 메시지 구조"""
    symbol: str
    opportunity_type: str  # "sandwich", "arbitrage", "liquidation"
    profit_potential: float
    gas_cost_estimate: float
    confidence: float
    time_sensitive: bool
    priority: int  # 1-10
    mempool_position: int
    block_prediction: int
    execution_strategy: str
    timestamp: int

@dataclass
class PerformanceMetrics:
    """성능 메트릭 메시지"""
    predictions_made: int
    mev_opportunities_detected: int
    accuracy_score: float
    profitable_trades: int
    total_profit: float
    uptime_seconds: int
    last_update: int

class RustBridge:
    """Rust xCrack과의 통신 브리지"""
    
    def __init__(self, host: str = "localhost", port: int = 8080, 
                 protocol: CommunicationProtocol = CommunicationProtocol.WEBSOCKET):
        self.host = host
        self.port = port
        self.protocol = protocol
        self.connected = False
        
        # 연결 객체들
        self.websocket = None
        self.redis_client = None
        self.tcp_reader = None
        self.tcp_writer = None
        
        # 메시지 큐
        self.outbound_queue = asyncio.Queue()
        self.response_futures = {}
        
        # 통계
        self.messages_sent = 0
        self.messages_received = 0
        self.connection_errors = 0
        
    async def connect(self) -> bool:
        """연결 설정"""
        try:
            if self.protocol == CommunicationProtocol.WEBSOCKET:
                await self._connect_websocket()
            elif self.protocol == CommunicationProtocol.REDIS:
                await self._connect_redis()
            elif self.protocol == CommunicationProtocol.TCP:
                await self._connect_tcp()
            
            self.connected = True
            logger.info(f"✅ Rust 브리지 연결 성공 ({self.protocol.value}://{self.host}:{self.port})")
            
            # 메시지 처리 태스크 시작
            asyncio.create_task(self._message_sender())
            asyncio.create_task(self._message_receiver())
            
            return True
            
        except Exception as e:
            logger.error(f"❌ Rust 브리지 연결 실패: {e}")
            self.connection_errors += 1
            return False
    
    async def disconnect(self):
        """연결 해제"""
        self.connected = False
        
        if self.websocket:
            await self.websocket.close()
        if self.redis_client:
            await self.redis_client.close()
        if self.tcp_writer:
            self.tcp_writer.close()
            await self.tcp_writer.wait_closed()
        
        logger.info("Rust 브리지 연결 해제 완료")
    
    async def is_connected(self) -> bool:
        """연결 상태 확인"""
        if not self.connected:
            return False
        
        try:
            # 각 프로토콜별 연결 상태 확인
            if self.protocol == CommunicationProtocol.WEBSOCKET:
                return self.websocket and not self.websocket.closed
            elif self.protocol == CommunicationProtocol.REDIS:
                if self.redis_client:
                    await self.redis_client.ping()
                    return True
            elif self.protocol == CommunicationProtocol.TCP:
                return self.tcp_writer and not self.tcp_writer.is_closing()
        except:
            return False
        
        return False
    
    async def send_prediction(self, prediction: PredictionMessage) -> bool:
        """예측 결과 전송"""
        message = {
            "type": "prediction",
            "data": asdict(prediction),
            "timestamp": int(time.time() * 1000)
        }
        
        success = await self._send_message(message)
        if success:
            logger.debug(f"예측 전송: {prediction.symbol} ({prediction.confidence:.3f})")
        
        return success
    
    async def send_mev_opportunity(self, opportunity: MEVOpportunityMessage) -> bool:
        """MEV 기회 전송"""
        message = {
            "type": "mev_opportunity",
            "data": asdict(opportunity),
            "timestamp": int(time.time() * 1000)
        }
        
        success = await self._send_message(message)
        if success:
            logger.debug(f"MEV 기회 전송: {opportunity.symbol} ({opportunity.opportunity_type})")
        
        return success
    
    async def send_metrics(self, metrics: Dict[str, Any]) -> bool:
        """성능 메트릭 전송"""
        message = {
            "type": "metrics",
            "data": metrics,
            "timestamp": int(time.time() * 1000)
        }
        
        return await self._send_message(message)
    
    async def send_heartbeat(self) -> bool:
        """헬스체크 전송"""
        message = {
            "type": "heartbeat",
            "data": {
                "status": "alive",
                "timestamp": int(time.time() * 1000),
                "messages_sent": self.messages_sent,
                "messages_received": self.messages_received
            }
        }
        
        return await self._send_message(message)
    
    async def get_performance_feedback(self) -> Optional[Dict[str, Any]]:
        """성과 피드백 요청"""
        message = {
            "type": "request_feedback",
            "request_id": f"feedback_{int(time.time())}",
            "timestamp": int(time.time() * 1000)
        }
        
        # 응답 대기를 위한 Future 생성
        request_id = message["request_id"]
        future = asyncio.Future()
        self.response_futures[request_id] = future
        
        if await self._send_message(message):
            try:
                # 5초 타임아웃으로 응답 대기
                response = await asyncio.wait_for(future, timeout=5.0)
                return response
            except asyncio.TimeoutError:
                logger.warning("성과 피드백 응답 타임아웃")
                del self.response_futures[request_id]
                return None
        
        return None
    
    async def reconnect(self) -> bool:
        """재연결 시도"""
        logger.info("Rust 브리지 재연결 시도...")
        await self.disconnect()
        await asyncio.sleep(1)
        return await self.connect()
    
    # Private methods
    
    async def _connect_websocket(self):
        """WebSocket 연결"""
        uri = f"ws://{self.host}:{self.port}/ai_bridge"
        self.websocket = await websockets.connect(uri)
    
    async def _connect_redis(self):
        """Redis 연결"""
        self.redis_client = await aioredis.from_url(
            f"redis://{self.host}:{self.port}",
            decode_responses=True
        )
    
    async def _connect_tcp(self):
        """TCP 연결"""
        self.tcp_reader, self.tcp_writer = await asyncio.open_connection(
            self.host, self.port
        )
    
    async def _send_message(self, message: Dict[str, Any]) -> bool:
        """메시지 전송"""
        try:
            await self.outbound_queue.put(message)
            return True
        except Exception as e:
            logger.error(f"메시지 큐 추가 실패: {e}")
            return False
    
    async def _message_sender(self):
        """메시지 전송 루프"""
        while self.connected:
            try:
                # 큐에서 메시지 가져오기
                message = await asyncio.wait_for(
                    self.outbound_queue.get(), timeout=1.0
                )
                
                # 프로토콜별 전송
                if self.protocol == CommunicationProtocol.WEBSOCKET:
                    await self._send_websocket(message)
                elif self.protocol == CommunicationProtocol.REDIS:
                    await self._send_redis(message)
                elif self.protocol == CommunicationProtocol.TCP:
                    await self._send_tcp(message)
                
                self.messages_sent += 1
                
            except asyncio.TimeoutError:
                continue
            except Exception as e:
                logger.error(f"메시지 전송 오류: {e}")
                self.connection_errors += 1
                await asyncio.sleep(1)
    
    async def _message_receiver(self):
        """메시지 수신 루프"""
        while self.connected:
            try:
                # 프로토콜별 수신
                if self.protocol == CommunicationProtocol.WEBSOCKET:
                    message = await self._receive_websocket()
                elif self.protocol == CommunicationProtocol.REDIS:
                    message = await self._receive_redis()
                elif self.protocol == CommunicationProtocol.TCP:
                    message = await self._receive_tcp()
                else:
                    await asyncio.sleep(1)
                    continue
                
                if message:
                    await self._handle_received_message(message)
                    self.messages_received += 1
                
            except Exception as e:
                logger.error(f"메시지 수신 오류: {e}")
                await asyncio.sleep(1)
    
    async def _send_websocket(self, message: Dict[str, Any]):
        """WebSocket 메시지 전송"""
        if self.websocket:
            await self.websocket.send(json.dumps(message))
    
    async def _send_redis(self, message: Dict[str, Any]):
        """Redis 메시지 전송"""
        if self.redis_client:
            channel = f"ai_to_rust_{message['type']}"
            await self.redis_client.publish(channel, json.dumps(message))
    
    async def _send_tcp(self, message: Dict[str, Any]):
        """TCP 메시지 전송"""
        if self.tcp_writer:
            data = json.dumps(message).encode() + b'\n'
            self.tcp_writer.write(data)
            await self.tcp_writer.drain()
    
    async def _receive_websocket(self) -> Optional[Dict[str, Any]]:
        """WebSocket 메시지 수신"""
        if self.websocket:
            try:
                data = await asyncio.wait_for(self.websocket.recv(), timeout=1.0)
                return json.loads(data)
            except asyncio.TimeoutError:
                return None
        return None
    
    async def _receive_redis(self) -> Optional[Dict[str, Any]]:
        """Redis 메시지 수신"""
        # Redis pubsub은 별도 구현 필요
        return None
    
    async def _receive_tcp(self) -> Optional[Dict[str, Any]]:
        """TCP 메시지 수신"""
        if self.tcp_reader:
            try:
                data = await asyncio.wait_for(
                    self.tcp_reader.readline(), timeout=1.0
                )
                if data:
                    return json.loads(data.decode().strip())
            except asyncio.TimeoutError:
                return None
        return None
    
    async def _handle_received_message(self, message: Dict[str, Any]):
        """수신된 메시지 처리"""
        message_type = message.get("type")
        
        if message_type == "feedback_response":
            # 피드백 응답 처리
            request_id = message.get("request_id")
            if request_id in self.response_futures:
                future = self.response_futures.pop(request_id)
                if not future.done():
                    future.set_result(message.get("data"))
        
        elif message_type == "config_update":
            # 설정 업데이트 요청
            logger.info("Rust로부터 설정 업데이트 요청 수신")
            # 실제 구현에서는 설정 업데이트 로직 추가
        
        elif message_type == "status_request":
            # 상태 요청에 대한 응답
            status_response = {
                "type": "status_response",
                "data": {
                    "status": "running",
                    "connected": True,
                    "messages_sent": self.messages_sent,
                    "messages_received": self.messages_received,
                    "connection_errors": self.connection_errors
                },
                "timestamp": int(time.time() * 1000)
            }
            await self._send_message(status_response)
        
        else:
            logger.debug(f"알 수 없는 메시지 타입: {message_type}")

# 유틸리티 함수들

def create_prediction_message(
    symbol: str,
    direction: float,
    confidence: float,
    time_horizon: int,
    expected_move: float,
    strategy_type: str = "vwap",
    strategy_params: Dict[str, Any] = None
) -> PredictionMessage:
    """예측 메시지 생성 헬퍼"""
    return PredictionMessage(
        symbol=symbol,
        direction=direction,
        confidence=confidence,
        time_horizon=time_horizon,
        expected_move=expected_move,
        timestamp=int(time.time() * 1000),
        strategy_type=strategy_type,
        strategy_params=strategy_params or {},
        model_version="1.0.0",
        features_used=["price", "volume", "volatility"]
    )

def create_mev_opportunity_message(
    symbol: str,
    opportunity_type: str,
    profit_potential: float,
    confidence: float,
    priority: int = 5
) -> MEVOpportunityMessage:
    """MEV 기회 메시지 생성 헬퍼"""
    return MEVOpportunityMessage(
        symbol=symbol,
        opportunity_type=opportunity_type,
        profit_potential=profit_potential,
        gas_cost_estimate=0.01,  # 기본값
        confidence=confidence,
        time_sensitive=True,
        priority=priority,
        mempool_position=0,
        block_prediction=0,
        execution_strategy="fast",
        timestamp=int(time.time() * 1000)
    )