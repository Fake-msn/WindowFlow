import { useState, useEffect, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import WindowThumbnail from './WindowThumbnail'
import RecommendationSection from './RecommendationSection'
import { useTheme } from '../contexts/ThemeContext'

interface MonitorInfo {
  id: number
  name: string
  is_primary: boolean
  dpi: number
}

// 将 Windows 设备路径（如 \\.\DISPLAY1）转换为友好名称（如 "显示器 1"）
function formatMonitorName(name: string): string {
  const match = name.match(/DISPLAY(\d+)/i)
  if (match) {
    return `显示器 ${match[1]}`
  }
  return name
}

interface WindowInfo {
  hwnd: number
  pid: number
  process_name: string
  window_title_hash: string
  monitor_id: number
  is_visible: boolean
}

interface FloatingPanelProps {
  monitors: MonitorInfo[]
  onClose: () => void
  onOpenSettings: () => void
  recommendListCount: number
  recommendMaxWindows: number
}

export default function FloatingPanel({ monitors, onClose, onOpenSettings, recommendListCount, recommendMaxWindows }: FloatingPanelProps) {
  const [windows, setWindows] = useState<WindowInfo[]>([])
  const [selectedMonitor, setSelectedMonitor] = useState<number | null>(null)
  const [loading, setLoading] = useState(false)
  const { resolvedTheme, setTheme } = useTheme()
  const isLight = resolvedTheme === 'light'

  const toggleTheme = () => {
    setTheme(isLight ? 'dark' : 'light')
  }

  useEffect(() => {
    loadWindows()

    const unlisten = listen('show-panel', () => {
      console.log('FloatingPanel: show-panel event received, refreshing windows...')
      loadWindows()
    })

    return () => {
      unlisten.then(f => f())
    }
  }, [])

  const loadWindows = async () => {
    try {
      const result = await invoke<WindowInfo[]>('get_all_windows')
      setWindows(result)
    } catch (error) {
      console.error('Failed to load windows:', error)
    }
  }

  const handleMigrateWindow = async (hwnd: number, targetMonitorId: number) => {
    setLoading(true)
    try {
      await invoke('migrate_window', {
        request: { hwnd, target_monitor_id: targetMonitorId }
      })
      await loadWindows()
    } catch (error) {
      console.error('Failed to migrate window:', error)
    } finally {
      setLoading(false)
    }
  }

  const handleMigrateAll = async (targetMonitorId: number) => {
    setLoading(true)
    try {
      const hwnds = windows.map(w => w.hwnd)
      await invoke('migrate_windows', {
        request: { hwnds, target_monitor_id: targetMonitorId }
      })
      await loadWindows()
    } catch (error) {
      console.error('Failed to migrate windows:', error)
    } finally {
      setLoading(false)
    }
  }

  const headerRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const el = headerRef.current
    if (!el) return

    const handleMouseDown = (e: MouseEvent) => {
      const target = e.target as HTMLElement
      if (target.tagName !== 'BUTTON' && !target.closest('button')) {
        getCurrentWindow().startDragging()
      }
    }

    el.addEventListener('mousedown', handleMouseDown)
    return () => el.removeEventListener('mousedown', handleMouseDown)
  }, [])

  const handleMonitorClick = async (monitorId: number) => {
    setSelectedMonitor(monitorId)
    // 在对应显示器上显示标识，帮助用户识别
    try {
      await invoke('flash_monitor', { monitorId })
    } catch (error) {
      console.error('Failed to flash monitor:', error)
    }
  }

  return (
    <div className={`w-full h-full flex flex-col overflow-hidden ${isLight ? 'bg-[#E1F5FE]' : 'bg-gray-900'}`}>
      {/* 头部 - 可拖拽区域 */}
      <div
        ref={headerRef}
        className={`flex items-center justify-between px-4 py-3 border-b cursor-move select-none ${
          isLight ? 'border-blue-200/60' : 'border-gray-700/50'
        }`}
      >
        <h1 className={`text-base font-semibold ${isLight ? 'text-gray-800' : 'text-white'}`}>WindowFlow</h1>
        <div className="flex items-center gap-1">
          {/* 关闭按钮 */}
          <button
            onClick={onClose}
            className={`transition-colors p-1 ${isLight ? 'text-gray-500 hover:text-gray-800' : 'text-gray-400 hover:text-white'}`}
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
          {/* 主题切换按钮：太阳/月亮线型图标 */}
          <button
            onClick={toggleTheme}
            onMouseDown={(e) => e.stopPropagation()}
            className={`transition-colors p-1.5 rounded ${isLight ? 'text-blue-600 hover:bg-blue-100' : 'text-gray-400 hover:text-white hover:bg-gray-800'}`}
            title={isLight ? '切换到深色模式' : '切换到浅色模式'}
          >
            {isLight ? (
              /* 太阳线型图标（浅色模式时显示，表示当前为浅色） */
              <svg className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="1.5" viewBox="0 0 24 24">
                <circle cx="12" cy="12" r="4" />
                <path strokeLinecap="round" d="M12 2v3M12 19v3M4.22 4.22l2.12 2.12M17.66 17.66l2.12 2.12M2 12h3M19 12h3M4.22 19.78l2.12-2.12M17.66 6.34l2.12-2.12" />
              </svg>
            ) : (
              /* 月亮线型图标（深色模式时显示，表示当前为深色） */
              <svg className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="1.5" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" />
              </svg>
            )}
          </button>
          {/* 设置入口：竖向三点 */}
          <button
            onClick={(e) => {
              e.stopPropagation();
              onOpenSettings();
            }}
            onMouseDown={(e) => e.stopPropagation()}
            className={`transition-colors p-1.5 rounded ${isLight ? 'text-gray-500 hover:text-gray-800 hover:bg-blue-100' : 'text-gray-400 hover:text-white hover:bg-gray-800'}`}
            title="设置"
          >
            <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 24 24">
              <circle cx="12" cy="5" r="1.5" />
              <circle cx="12" cy="12" r="1.5" />
              <circle cx="12" cy="19" r="1.5" />
            </svg>
          </button>
        </div>
      </div>

      {/* 内容区域 */}
      <div className="flex-1 overflow-y-auto px-4 py-3 space-y-4">
        {/* 推荐板块 */}
        <RecommendationSection 
          listCount={recommendListCount}
          maxWindows={recommendMaxWindows}
        />

        {/* 显示器选择 */}
        <div>
          <h2 className={`text-xs font-medium mb-2 ${isLight ? 'text-gray-600' : 'text-gray-400'}`}>目标显示器</h2>
          <div className="grid grid-cols-2 gap-2">
            {monitors.map((monitor) => (
              <button
                key={monitor.id}
                onClick={() => handleMonitorClick(monitor.id)}
                className={`p-2.5 rounded-lg border transition-all text-left ${
                  selectedMonitor === monitor.id
                    ? 'border-blue-500 bg-blue-500/10'
                    : isLight
                      ? 'border-blue-200 bg-white/50 hover:border-blue-300'
                      : 'border-gray-700 bg-gray-800/50 hover:border-gray-600'
                }`}
              >
                <div className={`text-sm font-medium truncate ${isLight ? 'text-gray-800' : 'text-white'}`}>{formatMonitorName(monitor.name)}</div>
                <div className={`text-xs ${isLight ? 'text-gray-500' : 'text-gray-400'}`}>
                  {monitor.is_primary ? '主' : '副'} • {monitor.dpi}DPI
                </div>
              </button>
            ))}
          </div>

          {selectedMonitor && (
            <button
              onClick={() => handleMigrateAll(selectedMonitor)}
              disabled={loading}
              className="mt-2 w-full bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium py-2 px-3 rounded-lg transition-colors disabled:opacity-50"
            >
              {loading ? '迁移中...' : '一键迁移所有窗口'}
            </button>
          )}
        </div>

        {/* 窗口列表 */}
        <div className="flex flex-col min-h-0">
          <h2 className={`text-xs font-medium mb-2 ${isLight ? 'text-gray-600' : 'text-gray-400'}`}>当前窗口</h2>
          <div className="space-y-2 overflow-y-auto flex-1 min-h-0">
            {windows.map((window) => (
              <WindowThumbnail
                key={window.hwnd}
                window={window}
                onMigrate={handleMigrateWindow}
                targetMonitorId={selectedMonitor}
              />
            ))}
          </div>
        </div>
      </div>
    </div>
  )
}
