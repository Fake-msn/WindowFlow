import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useTheme } from '../contexts/ThemeContext'

interface WindowInfo {
  hwnd: number
  pid: number
  process_name: string
  window_title_hash: string
  monitor_id: number
  is_visible: boolean
}

interface WindowThumbnailProps {
  window: WindowInfo
  onMigrate: (hwnd: number, targetMonitorId: number) => void
  targetMonitorId: number | null
}

export default function WindowThumbnail({ window, onMigrate, targetMonitorId }: WindowThumbnailProps) {
  const [thumbnail, setThumbnail] = useState<string>('')
  const [isMinimized, setIsMinimized] = useState(false)
  const { resolvedTheme } = useTheme()
  const isLight = resolvedTheme === 'light'
  const processName = window.process_name.replace('.exe', '')
  
  useEffect(() => {
    loadThumbnail()
  }, [window.hwnd])

  const loadThumbnail = async () => {
    try {
      const result = await invoke<string>('get_window_thumbnail', {
        hwnd: window.hwnd,
        maxWidth: 320,
        maxHeight: 180
      })
      setThumbnail(result)
      setIsMinimized(false)
    } catch (error) {
      const msg = String(error)
      if (msg.includes('minimized')) {
        setIsMinimized(true)
      }
      console.error('Failed to load thumbnail:', error)
    }
  }
  
  return (
    <div className={`rounded-lg p-2 border transition-all group ${
      isMinimized
        ? isLight
          ? 'bg-white/30 border-blue-200/30 opacity-60'
          : 'bg-gray-800/30 border-gray-700/50 opacity-60'
        : isLight
          ? 'bg-white/50 border-blue-200 hover:border-blue-300'
          : 'bg-gray-800/50 border-gray-700 hover:border-gray-600'
    }`}>
      <div className="flex gap-2">
        {/* 缩略图 */}
        <div className={`w-20 h-12 rounded flex-shrink-0 overflow-hidden relative ${
          isLight ? 'bg-blue-100/50' : 'bg-gray-900/50'
        }`}>
          {thumbnail ? (
            <img src={thumbnail} alt={processName} className="w-full h-full object-cover" />
          ) : isMinimized ? (
            <div className={`w-full h-full flex flex-col items-center justify-center ${isLight ? 'text-gray-400' : 'text-gray-500'}`}>
              <svg className="w-4 h-4 mb-0.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M20 12H4" />
              </svg>
              <span className="text-[10px]">最小化</span>
            </div>
          ) : (
            <div className={`w-full h-full flex items-center justify-center text-xs font-bold ${isLight ? 'text-gray-400' : 'text-gray-500'}`}>
              {processName.charAt(0).toUpperCase()}
            </div>
          )}
        </div>

        {/* 应用信息 */}
        <div className="flex-1 min-w-0">
          <div className={`text-sm font-medium truncate ${isLight ? 'text-gray-800' : 'text-white'}`}>
            {processName}
          </div>
          <div className={`text-xs ${isLight ? 'text-gray-500' : 'text-gray-400'}`}>
            PID: {window.pid}
          </div>
        </div>

        {/* 迁移按钮 */}
        {targetMonitorId && (
          <button
            onClick={() => onMigrate(window.hwnd, targetMonitorId)}
            className="bg-blue-600 hover:bg-blue-700 text-white text-xs py-1 px-2 rounded transition-colors self-center flex-shrink-0"
          >
            迁移
          </button>
        )}
      </div>
    </div>
  )
}
