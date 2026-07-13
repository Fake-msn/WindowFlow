import { useState, useEffect, useRef, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useTheme } from '../contexts/ThemeContext'

interface WindowInfo {
  hwnd: number
  process_name: string
  dwell_time_secs: number
}

interface RecommendationGroup {
  windows: WindowInfo[]
  label: string
}

interface RecommendationData {
  groups: RecommendationGroup[]
}

interface OnlineModelRecommendation {
  scenario_type: string
  window_combinations: string[]
  confidence_score: number
}

interface RecommendationSectionProps {
  maxWindows?: number
  listCount?: number
}

export default function RecommendationSection({ maxWindows = 5, listCount = 1 }: RecommendationSectionProps) {
  const [data, setData] = useState<RecommendationData | null>(null)
  const [loading, setLoading] = useState(true)
  const [migrating, setMigrating] = useState(false)
  const [debugInfo, setDebugInfo] = useState<any>(null)
  const [onlineModelData, setOnlineModelData] = useState<OnlineModelRecommendation[]>([])
  const [onlineModelLoading, setOnlineModelLoading] = useState(false)
  const { resolvedTheme } = useTheme()
  const isLight = resolvedTheme === 'light'

  // 每个组的完整窗口列表（从后端获取）
  const [allWindowsList, setAllWindowsList] = useState<WindowInfo[][]>([])
  // 每个组的滚动偏移量（控制翻页位置）
  const [offsets, setOffsets] = useState<number[]>([])
  // 动画状态：每个组是否正在翻页
  const [animating, setAnimating] = useState<boolean[]>([])
  const timerRefs = useRef<ReturnType<typeof setTimeout>[]>([])

  useEffect(() => {
    loadRecommendations()
    loadOnlineModelRecommendations()

    const unlisten = listen('show-panel', () => {
      console.log('RecommendationSection: show-panel event received, refreshing...')
      loadRecommendations()
      loadOnlineModelRecommendations()
    })

    return () => {
      unlisten.then(f => f())
      timerRefs.current.forEach(timer => clearTimeout(timer))
    }
  }, [listCount, maxWindows])

  const loadRecommendations = async () => {
    try {
      const status = await invoke<any>('get_monitor_status')
      console.log('Monitor status:', status)
      setDebugInfo(status)

      const recentEvents = await invoke<any>('get_recent_focus_events', { limit: 1 })
      const currentHwnd = recentEvents?.[0]?.hwnd

      if (!currentHwnd) {
        console.log('No active window found')
        setLoading(false)
        return
      }

      const result = await invoke<RecommendationData>('get_recommendations', {
        currentHwnd,
        maxCount: maxWindows,
      })

      console.log('Recommendations result:', result)
      setData(result)

      // 初始化每个组的窗口列表和滚动状态
      const groups = result.groups.slice(0, listCount)
      const windowsLists = groups.map(g => g.windows)
      const initialOffsets = groups.map(() => 0)
      const initialAnimStates = groups.map(() => false)

      setAllWindowsList(windowsLists)
      setOffsets(initialOffsets)
      setAnimating(initialAnimStates)

      // 为每个有超过3个窗口的组启动翻页动画
      windowsLists.forEach((windows, groupIndex) => {
        if (windows.length > 3) {
          startFlipAnimation(groupIndex, windows.length)
        }
      })
    } catch (error) {
      console.error('Failed to load recommendations:', error)
    } finally {
      setLoading(false)
    }
  }

  const loadOnlineModelRecommendations = async () => {
    // 检查是否配置了在线模型
    const settings = await invoke<any>('get_recommendation_settings')
    if (!settings.api_key || !settings.api_endpoint || !settings.model_name) {
      console.log('Online model not configured, skipping')
      return
    }

    setOnlineModelLoading(true)
    try {
      const result = await invoke<OnlineModelRecommendation[]>('get_online_model_recommendations', {
        maxWindows,
      })
      console.log('Online model recommendations:', result)
      setOnlineModelData(result)
    } catch (error) {
      console.error('Failed to load online model recommendations:', error)
    } finally {
      setOnlineModelLoading(false)
    }
  }

  // 翻页时钟动画：第0行固定，下方列表向下循环滚动（类似翻页时钟）
  const startFlipAnimation = useCallback((groupIndex: number, totalCount: number) => {
    // 清理旧定时器
    if (timerRefs.current[groupIndex]) {
      clearTimeout(timerRefs.current[groupIndex])
    }

    // remaining = totalCount - 1 (排除固定的第0个)
    const remaining = totalCount - 1
    if (remaining <= 2) return // 不够滚动

    const scheduleNext = () => {
      timerRefs.current[groupIndex] = setTimeout(() => {
        // 开始动画：标记正在动画（行1向上滑出，行2向下滑出）
        setAnimating(prev => {
          const next = [...prev]
          next[groupIndex] = true
          return next
        })

        // 动画持续400ms后，更新偏移量并重置动画状态
        setTimeout(() => {
          setOffsets(prev => {
            const next = [...prev]
            next[groupIndex] = (next[groupIndex] + 1) % remaining
            return next
          })
          setAnimating(prev => {
            const next = [...prev]
            next[groupIndex] = false
            return next
          })

          // 安排下一次翻页
          scheduleNext()
        }, 400)
      }, 3000)
    }

    scheduleNext()
  }, [])

  // 获取某个组当前应显示的窗口（只显示3行）
  const getDisplayWindows = (groupIndex: number): (WindowInfo | null)[] => {
    const allWindows = allWindowsList[groupIndex]
    if (!allWindows || allWindows.length === 0) return []

    const offset = offsets[groupIndex] || 0
    const remaining = allWindows.slice(1) // 排除固定的第0个

    if (remaining.length === 0) return [allWindows[0], null, null]
    if (remaining.length === 1) return [allWindows[0], remaining[0], null]

    const row1Index = offset % remaining.length
    const row2Index = (offset + 1) % remaining.length

    return [
      allWindows[0],              // 固定的第0行
      remaining[row1Index],       // 当前的第1行
      remaining[row2Index],       // 当前的第2行
    ]
  }

  const handleMigrate = async (groupIndex: number) => {
    const allWindows = allWindowsList[groupIndex]
    if (!allWindows || allWindows.length === 0) return

    setMigrating(true)
    try {
      const currentWindow = await invoke<any>('get_current_window_monitor')
      const targetMonitorId = currentWindow.monitor_id

      console.log('WindowFlow panel is on monitor:', targetMonitorId)

      // 迁移该组的所有窗口（使用完整列表，不只是当前显示的3个）
      const hwnds = allWindows.map(w => w.hwnd)

      const results = await invoke<any[]>('smart_migrate_windows', {
        request: {
          hwnds,
          target_monitor_id: targetMonitorId,
        }
      })

      const successCount = results.filter((r: any) => r.success).length
      const failCount = results.length - successCount

      console.log('Smart migration result:', { successCount, failCount })
    } catch (error) {
      console.error('Migration failed:', error)
    } finally {
      setMigrating(false)
    }
  }

  const formatDwellTime = (secs: number) => {
    if (secs < 60) return `${secs}秒`
    if (secs < 3600) return `${Math.floor(secs / 60)}分钟`
    return `${Math.floor(secs / 3600)}小时${Math.floor((secs % 3600) / 60)}分钟`
  }

  const getProcessIcon = (processName: string) => {
    const name = processName.toLowerCase().replace('.exe', '')
    if (name.includes('chrome') || name.includes('edge') || name.includes('firefox')) return '🌐'
    if (name.includes('code') || name.includes('vscode')) return ''
    if (name.includes('terminal') || name.includes('cmd')) return '⌨️'
    if (name.includes('feishu') || name.includes('lark')) return '💬'
    if (name.includes('explorer')) return '📁'
    return name.charAt(0).toUpperCase()
  }

  if (loading) {
    return (
      <div className={`border rounded-lg p-3 ${isLight ? 'bg-white/50 border-blue-200' : 'bg-gray-800/50 border-gray-700'}`}>
        <div className={`flex items-center gap-2 text-xs ${isLight ? 'text-gray-500' : 'text-gray-400'}`}>
          <svg className="w-3 h-3 animate-spin" fill="none" viewBox="0 0 24 24">
            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
          </svg>
          加载推荐中...
        </div>
      </div>
    )
  }

  if (!data || data.groups.length === 0) {
    return (
      <div className={`border rounded-lg p-3 ${isLight ? 'bg-white/50 border-blue-200/60' : 'bg-gray-800/50 border-gray-700/50'}`}>
        <h2 className={`text-xs font-medium mb-2 ${isLight ? 'text-gray-600' : 'text-gray-400'}`}>推荐</h2>
        <div className={`text-xs space-y-1 ${isLight ? 'text-gray-500' : 'text-gray-500'}`}>
          <div>暂无推荐数据</div>
          {debugInfo && (
            <div className={`text-[10px] ${isLight ? 'text-gray-400' : 'text-gray-600'}`}>
              事件数: {debugInfo.total_events ?? 0} | 近5分钟: {debugInfo.recent_5min ?? 0}
            </div>
          )}
          <div className={`text-[10px] ${isLight ? 'text-gray-400' : 'text-gray-600'}`}>
            提示: 请切换几个窗口后重新打开面板
          </div>
        </div>
      </div>
    )
  }

  const itemHeight = 24
  const visibleCount = 3
  const visibleHeight = visibleCount * itemHeight

  return (
    <div className="space-y-2">
      <h2 className={`text-xs font-medium ${isLight ? 'text-gray-600' : 'text-gray-400'}`}>推荐</h2>

      {allWindowsList.map((allWindows, groupIndex) => {
        const displayWindows = getDisplayWindows(groupIndex)
        const isGroupAnimating = animating[groupIndex] || false
        const group = data.groups[groupIndex]

        return (
          <div key={groupIndex} className={`border rounded-lg p-3 transition-all ${
            isLight 
              ? 'bg-gradient-to-r from-purple-100/30 to-blue-100/30 border-blue-200 hover:border-blue-300' 
              : 'bg-gradient-to-r from-purple-500/10 to-blue-500/10 border-purple-500/30 hover:border-purple-500/50'
          }`}>
            <div className="flex items-center gap-3">
              {/* 左侧：堆叠缩略图 */}
              <div className="relative w-20 h-14 flex-shrink-0">
                {allWindows.slice(0, 4).map((window, index) => (
                  <div
                    key={window.hwnd}
                    className={`absolute rounded shadow-lg overflow-hidden border ${
                      isLight ? 'border-blue-200 bg-white/70' : 'border-white/20 bg-gray-700'
                    }`}
                    style={{
                      width: '60px',
                      height: '45px',
                      left: `${index * 8}px`,
                      top: `${index * 3}px`,
                      zIndex: allWindows.length - index,
                      transform: `rotate(${(index - 1) * 2}deg)`,
                    }}
                  >
                    <div className={`w-full h-full flex items-center justify-center text-xs ${isLight ? 'text-gray-600' : 'text-gray-400'}`}>
                      {getProcessIcon(window.process_name)}
                    </div>
                  </div>
                ))}
                <div className={`absolute -bottom-1 left-0 text-[10px] ${isLight ? 'text-gray-500' : 'text-gray-400'}`}>
                  {allWindows.length} 个窗口
                </div>
              </div>

              {/* 中间：标题和翻页窗口列表 */}
              <div className="flex-1 min-w-0">
                <div className={`text-sm font-medium mb-1.5 ${isLight ? 'text-gray-800' : 'text-white'}`}>{group?.label || `组合 ${groupIndex + 1}`}</div>
                {/* 翻页区域 - 固定显示 3 行 */}
                <div
                  className="overflow-hidden relative"
                  style={{ height: `${visibleHeight}px` }}
                >
                  {displayWindows.map((window, index) => {
                    // 翻页时钟动画：
                    // 第0行：固定不动，zIndex最高
                    // 第1行：向下翻出（远离第0行，避免重叠）
                    // 第2行：从下方进入第1行的位置
                    let transform = 'none'
                    let opacity = 1

                    if (isGroupAnimating) {
                      if (index === 1) {
                        // 第1行向下翻出（远离第0行）
                        transform = 'translateY(24px)'
                        opacity = 0
                      } else if (index === 2) {
                        // 第2行从下方进入第1行的位置
                        // 初始位置在48px + 24px = 72px，移动到48px
                        // 但因为我们只有3行显示，所以第2行从屏幕外进入
                        transform = 'translateY(-24px)'
                        opacity = 1
                      }
                    }

                    return (
                      <div
                        key={index}
                        className="flex items-center gap-1.5 text-xs absolute w-full"
                        style={{
                          height: `${itemHeight}px`,
                          top: `${index * itemHeight}px`,
                          transition: 'transform 0.4s ease-in-out, opacity 0.4s ease-in-out',
                          transform,
                          opacity,
                          zIndex: index === 0 ? 10 : 1,
                        }}
                      >
                        <div className={`w-4 h-4 rounded flex items-center justify-center text-[10px] flex-shrink-0 ${
                          isLight ? 'bg-blue-100 text-gray-700' : 'bg-gray-700 text-gray-300'
                        }`}>
                          {window ? getProcessIcon(window.process_name) : ''}
                        </div>
                        <span className={`truncate flex-1 ${isLight ? 'text-gray-700' : 'text-gray-300'}`}>
                          {window ? window.process_name.replace('.exe', '') : ''}
                        </span>
                        <span className={`text-[10px] flex-shrink-0 ${isLight ? 'text-gray-500' : 'text-gray-500'}`}>
                          {window ? formatDwellTime(window.dwell_time_secs) : ''}
                        </span>
                      </div>
                    )
                  })}
                </div>
              </div>

              {/* 右侧：一键迁移按钮 */}
              <div className="flex-shrink-0">
                <button
                  onClick={() => handleMigrate(groupIndex)}
                  disabled={migrating}
                  className="px-3 py-1.5 bg-blue-500 hover:bg-blue-600 disabled:bg-gray-600 text-white text-xs rounded transition-colors whitespace-nowrap"
                >
                  {migrating ? '迁移中...' : '一键迁移'}
                </button>
              </div>
            </div>
          </div>
        )
      })}

      {/* 在线模型推荐板块（第三个推荐列表） */}
      {onlineModelLoading && (
        <div className={`border rounded-lg p-3 ${isLight ? 'bg-white/50 border-blue-200' : 'bg-gray-800/50 border-gray-700'}`}>
          <div className={`flex items-center gap-2 text-xs ${isLight ? 'text-gray-500' : 'text-gray-400'}`}>
            <svg className="w-3 h-3 animate-spin" fill="none" viewBox="0 0 24 24">
              <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
              <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
            </svg>
            AI 场景分析中...
          </div>
        </div>
      )}

      {onlineModelData.length > 0 && onlineModelData.map((rec, idx) => (
        <div key={`online-${idx}`} className={`border rounded-lg p-3 transition-all ${
          isLight
            ? 'bg-gradient-to-r from-emerald-100/30 to-cyan-100/30 border-emerald-200 hover:border-emerald-300'
            : 'bg-gradient-to-r from-emerald-500/10 to-cyan-500/10 border-emerald-500/30 hover:border-emerald-500/50'
        }`}>
          <div className="flex items-center gap-3">
            {/* 左侧：AI 图标 */}
            <div className={`relative w-20 h-14 flex-shrink-0 flex items-center justify-center rounded border ${
              isLight ? 'border-emerald-200 bg-white/70' : 'border-emerald-500/30 bg-gray-700'
            }`}>
              <span className="text-lg">🤖</span>
              <div className={`absolute -bottom-1 left-0 text-[10px] ${isLight ? 'text-emerald-600' : 'text-emerald-400'}`}>
                AI 推荐
              </div>
            </div>

            {/* 中间：场景类型和窗口组合 */}
            <div className="flex-1 min-w-0">
              <div className={`flex items-center gap-2 text-sm font-medium mb-1.5 ${isLight ? 'text-gray-800' : 'text-white'}`}>
                <span>{rec.scenario_type}</span>
                <span className={`text-[10px] px-1.5 py-0.5 rounded ${
                  isLight ? 'bg-emerald-100 text-emerald-700' : 'bg-emerald-500/20 text-emerald-400'
                }`}>
                  {Math.round(rec.confidence_score * 100)}%
                </span>
              </div>
              <div className="space-y-1">
                {rec.window_combinations.map((processName, wIdx) => (
                  <div key={wIdx} className="flex items-center gap-1.5 text-xs">
                    <div className={`w-4 h-4 rounded flex items-center justify-center text-[10px] flex-shrink-0 ${
                      isLight ? 'bg-emerald-100 text-gray-700' : 'bg-gray-700 text-gray-300'
                    }`}>
                      {getProcessIcon(processName)}
                    </div>
                    <span className={`truncate ${isLight ? 'text-gray-700' : 'text-gray-300'}`}>
                      {processName.replace('.exe', '')}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </div>
      ))}
    </div>
  )
}
