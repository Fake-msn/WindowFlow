import { useEffect, useState, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import FloatingPanel from './components/FloatingPanel'
import SettingsWindow from './components/SettingsWindow'
import AboutDialog from './components/AboutDialog'
import { ThemeProvider } from './contexts/ThemeContext'
import './App.css'

interface MonitorInfo {
  id: number
  name: string
  is_primary: boolean
  dpi: number
}

type ViewMode = 'panel' | 'settings' | 'about'

// 从 localStorage 加载设置（移到组件外部，避免每次渲染都执行）
const loadSettings = () => {
  const saved = localStorage.getItem('windowflow-settings')
  if (saved) {
    try {
      const settings = JSON.parse(saved)
      return {
        listCount: settings.recommendListCount || 1,
        maxWindows: settings.recommendMaxWindows || 5,
        mouseSideButtonEnabled: settings.mouseSideButtonEnabled ?? true,
        mouseSideButtonXButton1: settings.mouseSideButtonXButton1 ?? true,
        mouseSideButtonXButton2: settings.mouseSideButtonXButton2 ?? true,
      }
    } catch (e) {
      console.error('Failed to load settings:', e)
    }
  }
  return { listCount: 1, maxWindows: 5, mouseSideButtonEnabled: true, mouseSideButtonXButton1: true, mouseSideButtonXButton2: true }
}

function App() {
  const [viewMode, setViewMode] = useState<ViewMode>('panel')
  const [monitors, setMonitors] = useState<MonitorInfo[]>([])
  
  // 只在初始化时加载一次设置
  const [recommendListCount, setRecommendListCount] = useState(() => loadSettings().listCount)
  const [recommendMaxWindows, setRecommendMaxWindows] = useState(() => loadSettings().maxWindows)

  const loadMonitors = useCallback(async () => {
    try {
      const result = await invoke<MonitorInfo[]>('get_all_monitors')
      setMonitors(result)
    } catch (error) {
      console.error('Failed to load monitors:', error)
    }
  }, [])

  const hideWindow = useCallback(async () => {
    const win = getCurrentWindow()
    await win.hide()
  }, [])

  const showWindow = useCallback(async () => {
    const win = getCurrentWindow()
    await win.show()
    await win.setFocus()
  }, [])

  useEffect(() => {
    console.log('Setting up event listeners...')

    // 启动时同步鼠标侧键配置到后端
    const settings = loadSettings()
    invoke('update_mouse_side_button', {
      enabled: settings.mouseSideButtonEnabled,
      xbutton1: settings.mouseSideButtonXButton1,
      xbutton2: settings.mouseSideButtonXButton2,
    }).then(() => {
      console.log('Mouse side button config synced to backend')
    }).catch((e) => {
      console.error('Failed to sync mouse side button config:', e)
    })

    const unlistenPanel = listen('show-panel', async () => {
      console.log('show-panel event received')
      setViewMode('panel')
      await showWindow()
      loadMonitors()
    })

    const unlistenSettings = listen('show-settings', async () => {
      console.log('show-settings event received')
      setViewMode('settings')
      await showWindow()
    })

    const unlistenAbout = listen('show-about', async () => {
      console.log('show-about event received')
      setViewMode('about')
      await showWindow()
    })

    loadMonitors()

    // 每 500ms 调用一次后端 poll_focus_changes，记录窗口焦点变化
    let pollCount = 0
    const pollInterval = setInterval(() => {
      pollCount++
      invoke('poll_focus_changes')
        .then((count) => {
          const eventCount = count as number
          if (pollCount <= 5 || eventCount > 0) {
            console.log(`Poll #${pollCount}: events = ${eventCount}`)
          }
        })
        .catch((e) => {
          console.error(`Poll #${pollCount} failed:`, e)
        })
    }, 500)

    return () => {
      unlistenPanel.then(f => f())
      unlistenSettings.then(f => f())
      unlistenAbout.then(f => f())
      clearInterval(pollInterval)
    }
  }, [loadMonitors, showWindow])

  const handleClose = async () => {
    setViewMode('panel')
    await hideWindow()
  }

  // 关闭设置窗口：返回面板视图，但不隐藏窗口
  const handleCloseSettings = () => {
    setViewMode('panel')
  }

  return (
    <ThemeProvider>
      <div className="w-full h-full flex items-center justify-center">
        {viewMode === 'panel' && (
          <FloatingPanel
            monitors={monitors}
            onClose={handleClose}
            onOpenSettings={() => setViewMode('settings')}
            recommendListCount={recommendListCount}
            recommendMaxWindows={recommendMaxWindows}
          />
        )}
        {viewMode === 'settings' && (
          <SettingsWindow
            onClose={handleCloseSettings}
            recommendListCount={recommendListCount}
            recommendMaxWindows={recommendMaxWindows}
            onRecommendListCountChange={setRecommendListCount}
            onRecommendMaxWindowsChange={setRecommendMaxWindows}
          />
        )}
        {viewMode === 'about' && (
          <AboutDialog onClose={handleClose} />
        )}
      </div>
    </ThemeProvider>
  )
}

export default App
