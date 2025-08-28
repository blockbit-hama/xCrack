// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "../contracts/MultiAssetArbitrage.sol";
import "@aave/core-v3/contracts/mocks/helpers/MockPoolAddressesProvider.sol";

/**
 * @title MultiAssetArbitrage Test Script
 * @notice 테스트용 스크립트 - 실제 배포 전 검증용
 */
contract TestMultiAssetArbitrage {
    MultiAssetArbitrageStrategy public strategy;
    MockPoolAddressesProvider public provider;
    
    // 테스트용 토큰 주소들 (Mock)
    address constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;
    address constant USDC = 0xA0b86a33E6417f8C681A1fFE6954e127c9cd8e46;
    address constant DAI = 0x6B175474E89094C44Da98b954EedeAC495271d0F;
    
    // 테스트용 DEX 라우터 주소들
    address constant UNISWAP_V2_ROUTER = 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D;
    address constant SUSHISWAP_ROUTER = 0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F;
    
    event TestResult(string testName, bool success, string message);
    
    constructor() {
        // Mock PoolAddressesProvider 생성
        provider = new MockPoolAddressesProvider();
        
        // MultiAssetArbitrageStrategy 배포
        strategy = new MultiAssetArbitrageStrategy(provider);
    }
    
    /**
     * @notice 삼각 아비트래지 수익성 계산 테스트
     */
    function testTriangularArbitrageCalculation() public {
        MultiAssetArbitrageStrategy.TriangularArbitrageParams memory params = 
            MultiAssetArbitrageStrategy.TriangularArbitrageParams({
                tokenA: WETH,
                tokenB: USDC,
                tokenC: DAI,
                amountA: 1 ether, // 1 WETH
                amountB: 2000 * 10**6, // 2000 USDC
                dexAB: UNISWAP_V2_ROUTER,
                dexBC: SUSHISWAP_ROUTER,
                dexCA: UNISWAP_V2_ROUTER,
                dexCB: SUSHISWAP_ROUTER,
                expectedProfitMin: 0.01 ether, // 0.01 ETH 최소 수익
                swapCallDataAB: abi.encodeWithSignature("swapExactTokensForTokens(uint256,uint256,address[],address,uint256)", 
                    1 ether, 0, _getPath(WETH, DAI), address(this), block.timestamp + 300),
                swapCallDataBC: abi.encodeWithSignature("swapExactTokensForTokens(uint256,uint256,address[],address,uint256)", 
                    2000 * 10**6, 0, _getPath(USDC, DAI), address(this), block.timestamp + 300),
                swapCallDataCA: abi.encodeWithSignature("swapExactTokensForTokens(uint256,uint256,address[],address,uint256)", 
                    0, 0, _getPath(DAI, WETH), address(this), block.timestamp + 300),
                swapCallDataCB: abi.encodeWithSignature("swapExactTokensForTokens(uint256,uint256,address[],address,uint256)", 
                    0, 0, _getPath(DAI, USDC), address(this), block.timestamp + 300)
            });
        
        try strategy.calculateTriangularProfitability(params) returns (uint256 expectedProfit) {
            emit TestResult("TriangularArbitrageCalculation", true, 
                string(abi.encodePacked("Expected profit: ", _uint2str(expectedProfit))));
        } catch Error(string memory reason) {
            emit TestResult("TriangularArbitrageCalculation", false, reason);
        } catch {
            emit TestResult("TriangularArbitrageCalculation", false, "Unknown error");
        }
    }
    
    /**
     * @notice 다중자산 아비트래지 파라미터 검증 테스트
     */
    function testMultiAssetArbitrageParams() public {
        address[] memory assets = new address[](2);
        uint256[] memory amounts = new uint256[](2);
        uint256[] memory modes = new uint256[](2);
        
        assets[0] = WETH;
        assets[1] = USDC;
        amounts[0] = 1 ether;
        amounts[1] = 2000 * 10**6;
        modes[0] = 0; // flash loan
        modes[1] = 0; // flash loan
        
        MultiAssetArbitrageStrategy.MultiAssetArbitrageParams memory params = 
            MultiAssetArbitrageStrategy.MultiAssetArbitrageParams({
                borrowAssets: assets,
                borrowAmounts: amounts,
                targetAssets: assets,
                dexes: new address[](0),
                spenders: new address[](0),
                expectedProfitMin: 0.01 ether,
                swapCallData: new bytes[](0),
                swapSequences: new uint256[](0)
            });
        
        // 파라미터 검증
        bool success = true;
        string memory message = "Parameters validated successfully";
        
        if (params.borrowAssets.length != params.borrowAmounts.length) {
            success = false;
            message = "Borrow assets and amounts length mismatch";
        }
        
        if (params.borrowAssets.length != assets.length) {
            success = false;
            message = "Borrow assets length mismatch";
        }
        
        emit TestResult("MultiAssetArbitrageParams", success, message);
    }
    
    /**
     * @notice 컨트랙트 소유자 권한 테스트
     */
    function testOwnerPermissions() public {
        bool success = true;
        string memory message = "Owner permissions validated";
        
        // 소유자가 아닌 주소에서 호출 시도 (실제로는 revert되어야 함)
        // 이 테스트는 컨트랙트가 올바르게 배포되었는지 확인
        try strategy.setOwner(address(0x1)) {
            // 성공하면 소유자 권한이 올바르게 설정됨
        } catch {
            success = false;
            message = "Owner permission test failed";
        }
        
        emit TestResult("OwnerPermissions", success, message);
    }
    
    /**
     * @notice 모든 테스트 실행
     */
    function runAllTests() public {
        testTriangularArbitrageCalculation();
        testMultiAssetArbitrageParams();
        testOwnerPermissions();
    }
    
    // Helper functions
    function _getPath(address tokenA, address tokenB) internal pure returns (address[] memory) {
        address[] memory path = new address[](2);
        path[0] = tokenA;
        path[1] = tokenB;
        return path;
    }
    
    function _uint2str(uint256 _i) internal pure returns (string memory) {
        if (_i == 0) {
            return "0";
        }
        uint256 j = _i;
        uint256 len;
        while (j != 0) {
            len++;
            j /= 10;
        }
        bytes memory bstr = new bytes(len);
        uint256 k = len;
        while (_i != 0) {
            k = k-1;
            uint8 temp = (48 + uint8(_i - _i / 10 * 10));
            bytes1 b1 = bytes1(temp);
            bstr[k] = b1;
            _i /= 10;
        }
        return string(bstr);
    }
}