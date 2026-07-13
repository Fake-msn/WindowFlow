import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useTheme } from '../contexts/ThemeContext'

interface Recommendation {
  workflow_label: string
  score: number
  source: string
}

interface WindowThumbnail {
  hwnd: number
  process_name: string
  thumbnail: string
}

interface MissionControlCardProps {
  recommendation: Recommendation
}

export default function MissionControlCard({ recommendation }: MissionControlCardProps) {
  const [thumbnails, setThumbnails] = useState<WindowThumbnail[]>([])
  const { resolvedTheme } = useTheme()
  const isLight = resolvedTheme === 'light'
  const scorePercentage = Math.round(recommendation.score * 100)
  
  useEffect(() => {
    loadThumbnails()
  }, [recommendation])

  const loadThumbnails = async () => {
    try {
      // 从推荐中提取应用名称（简化示例）
      const apps = recommendation.workflow_label.split(': ').slice(1)
      
      // 获取所有窗口
      const windows = await invoke<any[]>('get_all_windows')
      
      // 为每个推荐的应用加载缩略图
      const thumbs: WindowThumbnail[] = []
      for (const appName of apps) {
        const matchingWindow = windows.find(w => 
          w.process_name.toLowerCase().includes(appName.toLowerCase())
        )
        
        if (matchingWindow) {
          try {
            const thumbnail = await invoke<string>('get_window_thumbnail', {
              hwnd: matchingWindow.hwnd,
              maxWidth: 160,
              maxHeight: 120
            })
            thumbs.push({
              hwnd: matchingWindow.hwnd,
              process_name: matchingWindow.process_name,
              thumbnail
            })
          } catch (error) {
            console.error('Failed to load thumbnail for', appName, error)
          }
        }
      }
      
      setThumbnails(thumbs)
    } catch (error) {
      console.error('Failed to load thumbnails:', error)
    }
  }
  
  return (
    <div className={`border rounded-lg p-3 transition-all cursor-pointer ${
      isLight
        ? 'bg-gradient-to-r from-purple-100/30 to-blue-100/30 border-blue-200 hover:border-blue-300'
        : 'bg-gradient-to-r from-purple-500/10 to-blue-500/10 border-purple-500/30 hover:border-purple-500/50'
    }`}>
      <div className="flex items-start gap-3">
        {/* Mission Control 风格重叠缩略图 */}
        <div className="relative w-24 h-16 flex-shrink-0">
          {thumbnails.length > 0 ? (
            thumbnails.map((thumb, index) => (
              <div
                key={thumb.hwnd}
                className={`absolute rounded shadow-lg overflow-hidden border ${
                  isLight ? 'border-blue-200' : 'border-white/20'
                }`}
                style={{
                  width: '80px',
                  height: '60px',
                  left: `${index * 12}px`,
                  top: `${index * 4}px`,
                  zIndex: thumbnails.length - index,
                  transform: `rotate(${(index - 1) * 3}deg)`
                }}
              >
                <img src={thumb.thumbnail} alt={thumb.process_name} className="w-full h-full object-cover" />
              </div>
            ))
          ) : (
            <div className={`w-full h-full rounded flex items-center justify-center ${
              isLight ? 'bg-blue-100/50' : 'bg-gray-700/50'
            }`}>
              <svg className="w-6 h-6 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16M4 18h16" />
              </svg>
            </div>
          )}
        </div>

        {/* 推荐信息 */}
        <div className="flex-1 min-w-0">
          <div className={`text-sm font-medium mb-1 truncate ${isLight ? 'text-gray-800' : 'text-white'}`}>
            {recommendation.workflow_label}
          </div>
          <div className="flex items-center gap-2 mb-1">
            <div className={`flex-1 rounded-full h-1 ${
              isLight ? 'bg-blue-100/50' : 'bg-gray-700/50'
            }`}>
              <div
                className="bg-gradient-to-r from-purple-500 to-blue-500 h-1 rounded-full transition-all"
                style={{ width: `${scorePercentage}%` }}
              />
            </div>
            <span className={`text-xs ${isLight ? 'text-gray-500' : 'text-gray-400'}`}>{scorePercentage}%</span>
          </div>
          <div className={`text-xs px-1.5 py-0.5 rounded inline-block ${
            recommendation.source === 'local_rule'
              ? isLight
                ? 'bg-green-100 text-green-700'
                : 'bg-green-500/20 text-green-400'
              : isLight
                ? 'bg-blue-100 text-blue-700'
                : 'bg-blue-500/20 text-blue-400'
          }`}>
            {recommendation.source === 'local_rule' ? '本地规则' : '在线模型'}
          </div>
        </div>
      </div>
    </div>
  )
}
