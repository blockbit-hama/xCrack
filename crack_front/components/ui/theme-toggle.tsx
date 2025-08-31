"use client"

import React from 'react'
import { Moon, Sun, Monitor } from 'lucide-react'
import { useTheme } from '../theme-provider'
import { Button } from './button'

export function ThemeToggle() {
  const { theme, setTheme } = useTheme()

  const cycleTheme = () => {
    if (theme === 'light') {
      setTheme('dark')
    } else if (theme === 'dark') {
      setTheme('system')
    } else {
      setTheme('light')
    }
  }

  const getIcon = () => {
    switch (theme) {
      case 'light':
        return <Sun className="h-4 w-4" />
      case 'dark':
        return <Moon className="h-4 w-4" />
      case 'system':
        return <Monitor className="h-4 w-4" />
      default:
        return <Sun className="h-4 w-4" />
    }
  }

  return (
    <Button
      variant="outline"
      size="sm"
      onClick={cycleTheme}
      className="w-9 h-9 p-0"
    >
      {getIcon()}
      <span className="sr-only">테마 변경</span>
    </Button>
  )
}
