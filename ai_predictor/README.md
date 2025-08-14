# 🤖 xCrack AI 예측 시스템

xCrack의 통합 AI 예측 모듈로, 실시간 시장 분석 및 MEV 기회 탐지를 수행합니다.

## 🎯 주요 기능

### 📊 **다중 모델 앙상블 예측**
- **LSTM**: 시계열 패턴 학습
- **Transformer**: 어텐션 기반 예측
- **Random Forest**: 특성 기반 분류
- **XGBoost**: 그래디언트 부스팅

### ⚡ **실시간 MEV 탐지**
- 멤풀 거래 실시간 분석
- 샌드위치, 아비트래지, 청산 기회 탐지
- 수익성 및 가스비 최적화

### 🔗 **Rust xCrack 연동**
- WebSocket/Redis/TCP 멀티 프로토콜
- 실시간 예측 신호 전송
- 성과 피드백 자동 학습

## 🚀 빠른 시작

### 1. 의존성 설치
```bash
# xCrack 루트 디렉토리에서
cd ai_predictor
pip install -r requirements.txt
```

### 2. 환경 설정
```bash
# 환경 변수 파일 복사
cp .env.example .env

# 설정 파일 편집
vim .env
vim config/settings.yaml
```

### 3. 실행
```bash
# 스크립트를 통한 실행 (권장)
../scripts/run_ai_predictor.sh

# 직접 실행
python src/main.py

# 개발 모드
../scripts/run_ai_predictor.sh --dev

# GPU 모드
../scripts/run_ai_predictor.sh --gpu
```

## 📁 프로젝트 구조

```
ai_predictor/
├── src/
│   ├── main.py                 # 메인 엔트리포인트
│   ├── config/
│   │   └── settings.py         # 설정 관리
│   ├── core/
│   │   ├── prediction_engine.py # 예측 엔진
│   │   ├── market_analyzer.py   # 시장 분석기
│   │   └── mev_detector.py      # MEV 탐지기
│   ├── models/
│   │   ├── ensemble_predictor.py # 앙상블 모델
│   │   ├── lstm_model.py        # LSTM 모델
│   │   └── transformer_model.py # Transformer 모델
│   ├── communication/
│   │   └── rust_bridge.py       # Rust 통신 브리지
│   ├── data/
│   │   └── market_data_collector.py # 데이터 수집기
│   └── utils/
│       └── logger.py            # 로깅 유틸
├── config/
│   └── settings.yaml           # 기본 설정
├── requirements.txt            # Python 의존성
├── .env.example               # 환경 변수 예시
└── README.md                  # 이 파일
```

## ⚙️ 설정 옵션

### 📈 **예측 설정**
```yaml
prediction:
  interval_seconds: 10           # 예측 주기
  confidence_threshold: 0.7      # 최소 신뢰도
  prediction_horizons: [5, 15, 30, 60]  # 예측 시간 (분)
```

### 🤖 **모델 설정**
```yaml
models:
  lstm:
    hidden_size: 128
    num_layers: 2
    sequence_length: 60
  
  transformer:
    d_model: 128
    nhead: 8
    num_layers: 6
```

### 🔗 **통신 설정**
```yaml
communication:
  host: "localhost"
  port: 8080
  protocol: "websocket"  # websocket, redis, tcp
```

## 🎯 사용 예시

### Python에서 직접 사용
```python
from src.core.prediction_engine import PredictionEngine
from src.config.settings import Settings

# 설정 로드
settings = Settings.load("config/settings.yaml")

# 예측 엔진 초기화
engine = PredictionEngine(settings)
await engine.initialize()

# 예측 수행
predictions = await engine.generate_predictions(market_data)
```

### Rust xCrack과 연동
```rust
use crate::strategies::predictive::{PredictionSignal, PredictiveStrategy};

// AI 예측 신호 수신
let signal = PredictionSignal {
    symbol: "ETH/USDT".to_string(),
    direction: 0.8,  // 강한 매수 신호
    confidence: 0.85,
    time_horizon: 30,  // 30분
    // ...
};

// 예측 기반 전략 실행
strategy.execute_prediction(signal).await?;
```

## 📊 성능 모니터링

### 실시간 메트릭
- 예측 정확도
- MEV 기회 탐지 건수
- 수익률 및 샤프 비율
- 모델별 성과 분석

### 로그 확인
```bash
# 실시간 로그
tail -f logs/ai_predictor.log

# 에러 로그만
grep ERROR logs/ai_predictor.log
```

## 🔧 고급 설정

### GPU 가속
```bash
# CUDA 설치 확인
nvidia-smi

# GPU 모드 실행
export FORCE_GPU=true
../scripts/run_ai_predictor.sh --gpu
```

### 분산 처리
```yaml
performance:
  max_workers: 8
  batch_processing: true
  enable_gpu: true
```

### 모델 자동 재학습
```yaml
models:
  retrain_interval_hours: 24  # 24시간마다 재학습
  model_save_dir: "saved_models"
```

## 🐛 문제 해결

### 일반적인 오류
1. **연결 오류**: xCrack이 실행 중인지 확인
2. **GPU 오류**: CUDA 드라이버 설치 확인
3. **메모리 오류**: 배치 크기 조정

### 디버깅 모드
```bash
# 상세 로그 활성화
export LOG_LEVEL=DEBUG
../scripts/run_ai_predictor.sh --dev --verbose
```

## 📈 성능 최적화

### 권장 시스템 요구사항
- **CPU**: 8코어 이상
- **RAM**: 16GB 이상
- **GPU**: RTX 3070 이상 (옵션)
- **저장공간**: 10GB 이상

### 최적화 팁
1. GPU 사용 시 배치 크기 증가
2. 예측 주기 조정으로 CPU 부하 분산
3. Redis 캐시 활용으로 응답 속도 개선

## 🤝 기여하기

1. Fork 프로젝트
2. Feature 브랜치 생성
3. 변경사항 커밋
4. Pull Request 생성

## 📄 라이선스

xCrack 프로젝트와 동일한 라이선스를 따릅니다.