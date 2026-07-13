import { createContext, useContext, useState, useEffect, ReactNode } from 'react'

type Theme = 'dark' | 'light' | 'auto'
type ResolvedTheme = 'dark' | 'light'

interface ThemeContextType {
  theme: Theme
  resolvedTheme: ResolvedTheme
  setTheme: (theme: Theme) => void
}

const ThemeContext = createContext<ThemeContextType | undefined>(undefined)

// 获取系统主题偏好
const getSystemTheme = (): ResolvedTheme => {
  if (typeof window !== 'undefined' && window.matchMedia) {
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
  }
  return 'dark'
}

// 解析实际主题（处理 auto 模式）
const resolveTheme = (theme: Theme): ResolvedTheme => {
  if (theme === 'auto') {
    return getSystemTheme()
  }
  return theme
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<Theme>(() => {
    const saved = localStorage.getItem('windowflow-settings')
    if (saved) {
      try {
        const settings = JSON.parse(saved)
        const t = settings.theme || 'dark'
        console.log('[ThemeContext] init from localStorage:', t)
        return t
      } catch {
        return 'dark'
      }
    }
    return 'dark'
  })

  const setTheme = (t: Theme) => {
    console.log('[ThemeContext] setTheme called:', t)
    setThemeState(t)
  }

  const [resolvedTheme, setResolvedTheme] = useState<ResolvedTheme>(() => resolveTheme(theme))

  // 监听系统主题变化（auto 模式）
  useEffect(() => {
    if (theme !== 'auto') return

    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)')
    const handleChange = () => {
      setResolvedTheme(getSystemTheme())
    }

    mediaQuery.addEventListener('change', handleChange)
    return () => mediaQuery.removeEventListener('change', handleChange)
  }, [theme])

  // 当 theme 改变时更新 resolvedTheme
  useEffect(() => {
    const r = resolveTheme(theme)
    console.log('[ThemeContext] theme changed -> resolvedTheme:', theme, '->', r)
    setResolvedTheme(r)
  }, [theme])

  // 应用主题到 document
  useEffect(() => {
    const root = document.documentElement
    if (resolvedTheme === 'light') {
      root.classList.add('light-theme')
      root.classList.remove('dark-theme')
    } else {
      root.classList.add('dark-theme')
      root.classList.remove('light-theme')
    }
  }, [resolvedTheme])

  return (
    <ThemeContext.Provider value={{ theme, resolvedTheme, setTheme }}>
      {children}
    </ThemeContext.Provider>
  )
}

export function useTheme() {
  const context = useContext(ThemeContext)
  if (!context) {
    throw new Error('useTheme must be used within a ThemeProvider')
  }
  return context
}
