"use client"

import { useEffect, useRef, useState } from 'react'

interface UseWebSocketOptions {
  url: string
  reconnectInterval?: number
  maxReconnectAttempts?: number
}

export function useWebSocket<T = any>({
  url,
  reconnectInterval = 3000,
  maxReconnectAttempts = 5,
}: UseWebSocketOptions) {
  const [data, setData] = useState<T | null>(null)
  const [isConnected, setIsConnected] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const wsRef = useRef<WebSocket | null>(null)
  const reconnectAttemptsRef = useRef(0)
  const reconnectTimeoutRef = useRef<NodeJS.Timeout | null>(null)

  const connect = () => {
    try {
      wsRef.current = new WebSocket(url)

      wsRef.current.onopen = () => {
        setIsConnected(true)
        setError(null)
        reconnectAttemptsRef.current = 0
      }

      wsRef.current.onmessage = (event) => {
        try {
          const parsedData = JSON.parse(event.data)
          setData(parsedData)
        } catch (err) {
          console.error('Failed to parse WebSocket message:', err)
        }
      }

      wsRef.current.onclose = () => {
        setIsConnected(false)
        
        if (reconnectAttemptsRef.current < maxReconnectAttempts) {
          reconnectAttemptsRef.current++
          reconnectTimeoutRef.current = setTimeout(() => {
            connect()
          }, reconnectInterval)
        } else {
          setError('최대 재연결 시도 횟수에 도달했습니다.')
        }
      }

      wsRef.current.onerror = (error) => {
        setError('WebSocket 연결 오류가 발생했습니다.')
        console.error('WebSocket error:', error)
      }
    } catch (err) {
      setError('WebSocket 연결을 생성할 수 없습니다.')
      console.error('WebSocket connection error:', err)
    }
  }

  const disconnect = () => {
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current)
    }
    if (wsRef.current) {
      wsRef.current.close()
    }
  }

  const sendMessage = (message: any) => {
    if (wsRef.current && wsRef.current.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(message))
    }
  }

  useEffect(() => {
    connect()

    return () => {
      disconnect()
    }
  }, [url])

  return {
    data,
    isConnected,
    error,
    sendMessage,
    reconnect: connect,
  }
}
