import { useState, useRef, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useTheme } from '../contexts/ThemeContext'

interface SettingsWindowProps {
  onClose: () => void
  recommendListCount: number
  recommendMaxWindows: number
  onRecommendListCountChange: (count: number) => void
  onRecommendMaxWindowsChange: (max: number) => void
}

interface Settings {
  autoStart: boolean
  showOnLaunch: boolean
  hotkey: string
  theme: string
  thumbnailQuality: number
  cacheDuration: number
  recommendListCount: number
  recommendMaxWindows: number
  commonComboMinDwellSecs: number
  commonComboMouseIdleThresholdSecs: number
  recentMaxDwellSecs: number
  recentMinSwitchCount: number
  ignoreList: string[]
  mouseSideButtonEnabled: boolean
  mouseSideButtonXButton1: boolean
  mouseSideButtonXButton2: boolean
  apiKey: string
  apiEndpoint: string
  modelName: string
}

const DEFAULT_SETTINGS: Settings = {
  autoStart: true,
  showOnLaunch: false,
  hotkey: 'Ctrl+Shift+Space',
  theme: 'dark',
  thumbnailQuality: 80,
  cacheDuration: 30,
  recommendListCount: 1,
  recommendMaxWindows: 5,
  commonComboMinDwellSecs: 600,
  commonComboMouseIdleThresholdSecs: 180,
  recentMaxDwellSecs: 300,
  recentMinSwitchCount: 3,
  ignoreList: [],
  mouseSideButtonEnabled: true,
  mouseSideButtonXButton1: true,
  mouseSideButtonXButton2: true,
  apiKey: '',
  apiEndpoint: '',
  modelName: '',
}

const getInitialSettings = (recommendListCount: number, recommendMaxWindows: number): Settings => {
  const savedSettings = localStorage.getItem('windowflow-settings')
  if (savedSettings) {
    try {
      const parsed = JSON.parse(savedSettings)
      return { ...DEFAULT_SETTINGS, ...parsed }
    } catch (e) {
      console.error('Failed to load settings:', e)
    }
  }
  return {
    ...DEFAULT_SETTINGS,
    recommendListCount,
    recommendMaxWindows,
  }
}

const KEY_CODE_MAP: Record<string, string> = {
  'Space': 'Space',
  'KeyA': 'A', 'KeyB': 'B', 'KeyC': 'C', 'KeyD': 'D', 'KeyE': 'E',
  'KeyF': 'F', 'KeyG': 'G', 'KeyH': 'H', 'KeyI': 'I', 'KeyJ': 'J',
  'KeyK': 'K', 'KeyL': 'L', 'KeyM': 'M', 'KeyN': 'N', 'KeyO': 'O',
  'KeyP': 'P', 'KeyQ': 'Q', 'KeyR': 'R', 'KeyS': 'S', 'KeyT': 'T',
  'KeyU': 'U', 'KeyV': 'V', 'KeyW': 'W', 'KeyX': 'X', 'KeyY': 'Y', 'KeyZ': 'Z',
  'Digit0': '0', 'Digit1': '1', 'Digit2': '2', 'Digit3': '3', 'Digit4': '4',
  'Digit5': '5', 'Digit6': '6', 'Digit7': '7', 'Digit8': '8', 'Digit9': '9',
  'F1': 'F1', 'F2': 'F2', 'F3': 'F3', 'F4': 'F4', 'F5': 'F5', 'F6': 'F6',
  'F7': 'F7', 'F8': 'F8', 'F9': 'F9', 'F10': 'F10', 'F11': 'F11', 'F12': 'F12',
}

// 设置项注解说明
const SETTING_TOOLTIPS: Record<string, { title: string; description: string }> = {
  recommendListCount: {
    title: '推荐列表数量',
    description: '控制面板显示几组推荐窗口组合。例如设置为2时，面板会显示两个独立的推荐区域，每个区域推荐不同的窗口组合。',
  },
  recommendMaxWindows: {
    title: '每个列表最大窗口数',
    description: '控制每组推荐中最多显示几个窗口。数值越大，推荐的窗口组合越丰富，但也会占用更多面板空间。',
  },
  commonComboMinDwellSecs: {
    title: '常用组合最小停留时间',
    description: '只有当您在某个窗口上停留时间超过此阈值时，该窗口才会被认定为"常用窗口"。阈值越高，推荐越倾向于您长时间使用的应用。',
  },
  commonComboMouseIdleThresholdSecs: {
    title: '鼠标静止超时阈值',
    description: '如果鼠标静止超过此时间，系统认为您可能暂时离开，该段时间的窗口停留将不计入有效使用时间。这可以避免将挂机时间误判为活跃使用。',
  },
  recentMaxDwellSecs: {
    title: '频繁切换最大停留时间',
    description: '只有单次停留时间低于此值的窗口才算"频繁切换"。此设置用于识别您正在快速切换使用的应用，阈值越低，推荐越倾向于您高频切换的应用。',
  },
  recentMinSwitchCount: {
    title: '频繁切换最小次数',
    description: '需要在规定时间内切换窗口达到此次数以上，才会被识别为"频繁切换"模式。次数越高，推荐条件越严格。',
  },
}

export default function SettingsWindow({ onClose, recommendListCount, recommendMaxWindows, onRecommendListCountChange, onRecommendMaxWindowsChange }: SettingsWindowProps) {
  const [settings, setSettings] = useState<Settings>(() => getInitialSettings(recommendListCount, recommendMaxWindows))
  const [saved, setSaved] = useState(false)
  const [hotkeyError, setHotkeyError] = useState('')
  const hotkeyInputRef = useRef<HTMLInputElement>(null)
  const [isCapturing, setIsCapturing] = useState(false)
  const [activeTooltip, setActiveTooltip] = useState<string | null>(null)
  const { resolvedTheme, setTheme } = useTheme()
  const isLight = resolvedTheme === 'light'

  const toggleTooltip = (key: string) => {
    setActiveTooltip(prev => prev === key ? null : key)
  }

  // 问号图标组件
  const HelpIcon = ({ settingKey }: { settingKey: string }) => (
    <div className="relative inline-block ml-2">
      <button
        type="button"
        onClick={() => toggleTooltip(settingKey)}
        className={`w-4 h-4 rounded-full flex items-center justify-center text-xs transition-colors ${
          activeTooltip === settingKey
            ? 'bg-blue-600 text-white'
            : isLight
              ? 'bg-blue-100 text-blue-600 hover:bg-blue-200 hover:text-blue-700'
              : 'bg-gray-700 text-gray-400 hover:bg-gray-600 hover:text-gray-200'
        }`}
        title="点击查看详情"
      >
        ?
      </button>
      {activeTooltip === settingKey && SETTING_TOOLTIPS[settingKey] && (
        <div className={`absolute left-6 top-1/2 -translate-y-1/2 z-50 w-72 p-3 border rounded-lg shadow-xl ${
          isLight ? 'bg-white border-blue-200' : 'bg-gray-800 border-gray-600'
        }`}>
          <div className={`text-sm font-medium mb-1 ${isLight ? 'text-blue-600' : 'text-blue-400'}`}>{SETTING_TOOLTIPS[settingKey].title}</div>
          <div className={`text-xs leading-relaxed ${isLight ? 'text-gray-700' : 'text-gray-300'}`}>{SETTING_TOOLTIPS[settingKey].description}</div>
          {/* 小箭头 */}
          <div className={`absolute left-0 top-1/2 -translate-x-1 -translate-y-1/2 w-2 h-2 border-l border-b rotate-45 ${
            isLight ? 'bg-white border-blue-200' : 'bg-gray-800 border-gray-600'
          }`}></div>
        </div>
      )}
    </div>
  )

  const updateSetting = <K extends keyof Settings>(key: K, value: Settings[K]) => {
    setSettings(prev => ({ ...prev, [key]: value }))
    setSaved(false)

    if (key === 'recommendListCount') {
      onRecommendListCountChange(value as number)
    } else if (key === 'recommendMaxWindows') {
      onRecommendMaxWindowsChange(value as number)
    } else if (key === 'theme') {
      console.log('[SettingsWindow] updateSetting theme:', value)
      setTheme(value as 'dark' | 'light' | 'auto')
    }
  }

  const handleHotkeyKeyDown = useCallback((e: React.KeyboardEvent) => {
    e.preventDefault()
    e.stopPropagation()

    const modifiers: string[] = []
    if (e.ctrlKey) modifiers.push('Ctrl')
    if (e.shiftKey) modifiers.push('Shift')
    if (e.altKey) modifiers.push('Alt')

    const code = e.code
    let keyName = KEY_CODE_MAP[code]

    if (!keyName) {
      const upperKey = e.key.toUpperCase()
      if (upperKey.length === 1 && /[A-Z0-9]/.test(upperKey)) {
        keyName = upperKey
      } else if (e.key === ' ') {
        keyName = 'Space'
      } else if (e.key.startsWith('F') && /^F\d{1,2}$/.test(e.key)) {
        keyName = e.key
      }
    }

    if (!keyName) return

    if (modifiers.length === 0) {
      setHotkeyError('请至少包含一个修饰键 (Ctrl/Shift/Alt)')
      return
    }

    const hotkeyStr = [...modifiers, keyName].join('+')
    updateSetting('hotkey', hotkeyStr)
    setHotkeyError('')
  }, [])

  const handleSave = async () => {
    console.log('[SettingsWindow] handleSave, theme:', settings.theme)
    localStorage.setItem('windowflow-settings', JSON.stringify(settings))

    try {
      await invoke('update_hotkey', { newHotkey: settings.hotkey })
      console.log('Hotkey updated:', settings.hotkey)
    } catch (error) {
      console.error('Failed to update hotkey:', error)
      setHotkeyError(String(error))
      return
    }

    try {
      await invoke('update_mouse_side_button', {
        enabled: settings.mouseSideButtonEnabled,
        xbutton1: settings.mouseSideButtonXButton1,
        xbutton2: settings.mouseSideButtonXButton2,
      })
      console.log('Mouse side button config updated')
    } catch (error) {
      console.error('Failed to update mouse side button:', error)
    }

    try {
      const backendSettings = {
        list_count: settings.recommendListCount,
        max_windows_per_list: settings.recommendMaxWindows,
        common_combo_min_dwell_secs: settings.commonComboMinDwellSecs,
        common_combo_mouse_idle_threshold_secs: settings.commonComboMouseIdleThresholdSecs,
        recent_max_dwell_secs: settings.recentMaxDwellSecs,
        recent_min_switch_count: settings.recentMinSwitchCount,
        ignore_list: settings.ignoreList,
        api_key: settings.apiKey || null,
        api_endpoint: settings.apiEndpoint || null,
        model_name: settings.modelName || null,
      }
      await invoke('update_recommendation_settings', { settings: backendSettings })
    } catch (error) {
      console.error('Failed to sync settings to backend:', error)
    }

    onRecommendListCountChange(settings.recommendListCount)
    onRecommendMaxWindowsChange(settings.recommendMaxWindows)

    setSaved(true)
    setTimeout(() => {
      setSaved(false)
      onClose()
    }, 800)
  }

  // 主题感知的样式辅助
  const s = {
    heading: isLight ? 'text-gray-700' : 'text-gray-300',
    label: isLight ? 'text-gray-700' : 'text-gray-300',
    sublabel: isLight ? 'text-gray-500' : 'text-gray-400',
    hint: isLight ? 'text-gray-400' : 'text-gray-500',
    inputBg: isLight ? 'bg-white border-blue-200 text-gray-800' : 'bg-gray-800 border-gray-700 text-white',
    cardBg: isLight ? 'bg-white/60' : 'bg-gray-800',
    cardBgAlt: isLight ? 'bg-blue-50' : 'bg-gray-700',
    border: isLight ? 'border-blue-200' : 'border-gray-700',
    borderAlt: isLight ? 'border-blue-300' : 'border-gray-600',
    btnSecondary: isLight ? 'bg-blue-50 text-blue-700 hover:bg-blue-100' : 'bg-gray-800 text-gray-300 hover:bg-gray-700',
    btnActive: 'bg-blue-600 text-white',
  }

  return (
    <div className="fixed inset-0 flex items-center justify-center z-50" style={{ backgroundColor: isLight ? 'rgba(0,0,0,0.2)' : 'rgba(0,0,0,0.5)' }}>
      <div className={`rounded-xl shadow-2xl border w-[600px] max-h-[80vh] overflow-hidden flex flex-col ${
        isLight ? 'bg-[#E1F5FE] border-blue-200' : 'bg-gray-900 border-gray-700'
      }`} style={isLight ? { backgroundColor: '#E1F5FE' } : { backgroundColor: '#1a1d23' }}>
        <div className={`flex items-center justify-between px-6 py-4 border-b ${s.border}`} data-tauri-drag-region>
          <h2 className={`${isLight ? 'text-gray-800' : 'text-white'} text-lg font-semibold`}>设置</h2>
          <button onClick={onClose} className={`transition-colors p-1 ${isLight ? 'text-gray-500 hover:text-gray-800' : 'text-gray-400 hover:text-white'}`}>
            <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        <div className="flex-1 overflow-y-auto px-6 py-4 space-y-6">
          <section>
            <h3 className={`${s.heading} text-sm font-medium mb-3`}>通用</h3>
            <div className="space-y-3">
              <label className="flex items-center justify-between cursor-pointer">
                <span className={`${s.label} text-sm`}>开机自动启动</span>
                <input type="checkbox" checked={settings.autoStart} onChange={(e) => updateSetting('autoStart', e.target.checked)} className="w-4 h-4 rounded border-gray-600 text-blue-600 focus:ring-blue-500" />
              </label>
              <label className="flex items-center justify-between cursor-pointer">
                <span className={`${s.label} text-sm`}>启动时显示面板</span>
                <input type="checkbox" checked={settings.showOnLaunch} onChange={(e) => updateSetting('showOnLaunch', e.target.checked)} className="w-4 h-4 rounded border-gray-600 text-blue-600 focus:ring-blue-500" />
              </label>
            </div>
          </section>

          <section>
            <h3 className={`${s.heading} text-sm font-medium mb-3`}>快捷键</h3>
            <div className="space-y-3">
              <div>
                <label className={`${s.label} text-sm block mb-1`}>显示/隐藏面板</label>
                <input
                  ref={hotkeyInputRef}
                  type="text"
                  value={settings.hotkey}
                  readOnly
                  onClick={() => {
                    setIsCapturing(true)
                    hotkeyInputRef.current?.focus()
                  }}
                  onKeyDown={isCapturing ? handleHotkeyKeyDown : undefined}
                  placeholder="点击后按下快捷键..."
                  className={`w-full border rounded px-3 py-2 text-sm focus:outline-none cursor-pointer ${s.inputBg} ${
                    isCapturing ? 'border-blue-500 ring-1 ring-blue-500' : ''
                  }`}
                />
                {hotkeyError && <p className="text-red-400 text-xs mt-1">{hotkeyError}</p>}
                <p className={`${s.hint} text-xs mt-1`}>点击输入框后按下快捷键组合（需包含 Ctrl 或 Shift）</p>
              </div>
            </div>
          </section>

          <section>
            <h3 className={`${s.heading} text-sm font-medium mb-3`}>鼠标侧键</h3>
            <div className="space-y-3">
              <label className="flex items-center justify-between cursor-pointer">
                <span className={`${s.label} text-sm`}>启用鼠标侧键切换面板</span>
                <input
                  type="checkbox"
                  checked={settings.mouseSideButtonEnabled}
                  onChange={(e) => updateSetting('mouseSideButtonEnabled', e.target.checked)}
                  className="w-4 h-4 rounded border-gray-600 text-blue-600 focus:ring-blue-500"
                />
              </label>
              {settings.mouseSideButtonEnabled && (
                <div className={`ml-4 space-y-2 border-l-2 pl-4 ${isLight ? 'border-blue-200' : 'border-gray-700'}`}>
                  <label className="flex items-center justify-between cursor-pointer">
                    <span className={`${s.sublabel} text-sm`}>侧键1（后退键 / XButton1）</span>
                    <input
                      type="checkbox"
                      checked={settings.mouseSideButtonXButton1}
                      onChange={(e) => updateSetting('mouseSideButtonXButton1', e.target.checked)}
                      className="w-4 h-4 rounded border-gray-600 text-blue-600 focus:ring-blue-500"
                    />
                  </label>
                  <label className="flex items-center justify-between cursor-pointer">
                    <span className={`${s.sublabel} text-sm`}>侧键2（前进键 / XButton2）</span>
                    <input
                      type="checkbox"
                      checked={settings.mouseSideButtonXButton2}
                      onChange={(e) => updateSetting('mouseSideButtonXButton2', e.target.checked)}
                      className="w-4 h-4 rounded border-gray-600 text-blue-600 focus:ring-blue-500"
                    />
                  </label>
                </div>
              )}
              <p className={`${s.hint} text-xs`}>鼠标侧键可快速切换面板显示/隐藏，支持独立配置每个侧键</p>
            </div>
          </section>

          <section>
            <h3 className={`${s.heading} text-sm font-medium mb-3`}>外观</h3>
            <div className="space-y-3">
              <div>
                <label className={`${s.label} text-sm block mb-1`}>主题</label>
                <select
                  value={settings.theme}
                  onChange={(e) => updateSetting('theme', e.target.value)}
                  className={`w-full border rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-500 ${s.inputBg}`}
                >
                  <option value="dark">深色</option>
                  <option value="light">浅色</option>
                  <option value="auto">跟随系统</option>
                </select>
              </div>
            </div>
          </section>

          <section>
            <h3 className={`${s.heading} text-sm font-medium mb-3`}>性能</h3>
            <div className="space-y-3">
              <div>
                <label className={`${s.label} text-sm block mb-1`}>缩略图质量: {settings.thumbnailQuality}%</label>
                <input type="range" min="50" max="100" value={settings.thumbnailQuality} onChange={(e) => updateSetting('thumbnailQuality', Number(e.target.value))} className="w-full" />
              </div>
              <div>
                <label className={`${s.label} text-sm block mb-1`}>缓存时长: {settings.cacheDuration} 秒</label>
                <input type="range" min="10" max="300" step="10" value={settings.cacheDuration} onChange={(e) => updateSetting('cacheDuration', Number(e.target.value))} className="w-full" />
              </div>
            </div>
          </section>

          <section>
            <h3 className={`${s.heading} text-sm font-medium mb-3`}>推荐</h3>
            <div className="space-y-3">
              <div>
                <div className="flex items-center mb-1">
                  <label className={`${s.label} text-sm`}>推荐列表数量: {settings.recommendListCount} 个</label>
                  <HelpIcon settingKey="recommendListCount" />
                </div>
                <div className="flex gap-2">
                  {[1, 2, 3].map(count => (
                    <button key={count} onClick={() => updateSetting('recommendListCount', count)} className={`flex-1 px-3 py-1.5 rounded text-sm transition-colors ${settings.recommendListCount === count ? s.btnActive : s.btnSecondary}`}>
                      {count}
                    </button>
                  ))}
                </div>
              </div>
              <div>
                <div className="flex items-center mb-1">
                  <label className={`${s.label} text-sm`}>每个列表最大窗口数: {settings.recommendMaxWindows} 个</label>
                  <HelpIcon settingKey="recommendMaxWindows" />
                </div>
                <input type="range" min="2" max="10" value={settings.recommendMaxWindows} onChange={(e) => updateSetting('recommendMaxWindows', Number(e.target.value))} className="w-full" />
                <div className={`flex justify-between text-[10px] mt-1 ${s.hint}`}><span>2</span><span>10</span></div>
              </div>
            </div>
          </section>

          <section>
            <h3 className={`${s.heading} text-sm font-medium mb-3`}>推荐准入条件</h3>
            <div className="space-y-3">
              <div>
                <div className="flex items-center mb-1">
                  <label className={`${s.label} text-sm`}>常用组合最小停留时间: {Math.round(settings.commonComboMinDwellSecs / 60)} 分钟</label>
                  <HelpIcon settingKey="commonComboMinDwellSecs" />
                </div>
                <input type="range" min="5" max="30" step="1" value={Math.round(settings.commonComboMinDwellSecs / 60)} onChange={(e) => updateSetting('commonComboMinDwellSecs', Number(e.target.value) * 60)} className="w-full" />
                <div className={`flex justify-between text-[10px] mt-1 ${s.hint}`}><span>5分钟</span><span>30分钟</span></div>
              </div>
              <div>
                <div className="flex items-center mb-1">
                  <label className={`${s.label} text-sm`}>鼠标静止超时阈值: {Math.round(settings.commonComboMouseIdleThresholdSecs / 60)} 分钟</label>
                  <HelpIcon settingKey="commonComboMouseIdleThresholdSecs" />
                </div>
                <input type="range" min="1" max="10" step="1" value={Math.round(settings.commonComboMouseIdleThresholdSecs / 60)} onChange={(e) => updateSetting('commonComboMouseIdleThresholdSecs', Number(e.target.value) * 60)} className="w-full" />
                <div className={`flex justify-between text-[10px] mt-1 ${s.hint}`}><span>1分钟</span><span>10分钟</span></div>
              </div>
              <div>
                <div className="flex items-center mb-1">
                  <label className={`${s.label} text-sm`}>频繁切换最大停留时间: {Math.round(settings.recentMaxDwellSecs / 60)} 分钟</label>
                  <HelpIcon settingKey="recentMaxDwellSecs" />
                </div>
                <input type="range" min="1" max="10" step="1" value={Math.round(settings.recentMaxDwellSecs / 60)} onChange={(e) => updateSetting('recentMaxDwellSecs', Number(e.target.value) * 60)} className="w-full" />
                <div className={`flex justify-between text-[10px] mt-1 ${s.hint}`}><span>1分钟</span><span>10分钟</span></div>
              </div>
              <div>
                <div className="flex items-center mb-1">
                  <label className={`${s.label} text-sm`}>频繁切换最小次数: {settings.recentMinSwitchCount} 次</label>
                  <HelpIcon settingKey="recentMinSwitchCount" />
                </div>
                <input type="range" min="2" max="10" step="1" value={settings.recentMinSwitchCount} onChange={(e) => updateSetting('recentMinSwitchCount', Number(e.target.value))} className="w-full" />
                <div className={`flex justify-between text-[10px] mt-1 ${s.hint}`}><span>2次</span><span>10次</span></div>
              </div>
            </div>
          </section>

          <section>
            <h3 className={`${s.heading} text-sm font-medium mb-3`}>在线模型推荐</h3>
            <div className="space-y-3">
              <p className={`${s.sublabel} text-xs`}>配置在线模型 API，启用第三个推荐列表：场景化感知推荐</p>
              <div>
                <label className={`${s.label} text-sm block mb-1`}>API Key</label>
                <input
                  type="password"
                  value={settings.apiKey}
                  onChange={(e) => updateSetting('apiKey', e.target.value)}
                  placeholder="sk-..."
                  className={`w-full border rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-500 ${s.inputBg}`}
                />
              </div>
              <div>
                <label className={`${s.label} text-sm block mb-1`}>API 端点</label>
                <input
                  type="text"
                  value={settings.apiEndpoint}
                  onChange={(e) => updateSetting('apiEndpoint', e.target.value)}
                  placeholder="https://api.openai.com/v1/chat/completions"
                  className={`w-full border rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-500 ${s.inputBg}`}
                />
              </div>
              <div>
                <label className={`${s.label} text-sm block mb-1`}>模型名称</label>
                <input
                  type="text"
                  value={settings.modelName}
                  onChange={(e) => updateSetting('modelName', e.target.value)}
                  placeholder="gpt-4o-mini"
                  className={`w-full border rounded px-3 py-2 text-sm focus:outline-none focus:border-blue-500 ${s.inputBg}`}
                />
              </div>
              <p className={`${s.hint} text-xs`}>留空则不启用在线模型推荐。支持 OpenAI 兼容 API 格式。</p>
            </div>
          </section>

          <section>
            <h3 className={`${s.heading} text-sm font-medium mb-3`}>窗口忽略清单</h3>
            <div className="space-y-3">
              <p className={`${s.sublabel} text-xs`}>添加到忽略清单的窗口将不会出现在推荐列表中</p>
              <div className={`${s.cardBg} rounded p-3 max-h-40 overflow-y-auto`}>
                {settings.ignoreList.length === 0 ? (
                  <p className={`${s.hint} text-xs text-center py-2`}>暂无忽略的窗口</p>
                ) : (
                  <div className="space-y-2">
                    {settings.ignoreList.map((processName, index) => (
                      <div key={index} className={`flex items-center justify-between rounded px-3 py-2 ${s.cardBgAlt}`}>
                        <span className={`${s.label} text-xs`}>{processName}</span>
                        <button onClick={() => { const newIgnoreList = settings.ignoreList.filter((_, i) => i !== index); updateSetting('ignoreList', newIgnoreList) }} className="text-red-400 hover:text-red-300 text-xs">移除</button>
                      </div>
                    ))}
                  </div>
                )}
              </div>
              <button onClick={async () => { try { const stats = await invoke('get_window_stats'); const availableWindows = (stats as any[]).filter((s: any) => !settings.ignoreList.includes(s.process_name)); if (availableWindows.length > 0) { const selected = availableWindows[0].process_name; updateSetting('ignoreList', [...settings.ignoreList, selected]) } } catch (error) { console.error('Failed to get window stats:', error) } }} className={`w-full ${s.btnSecondary} text-xs py-2 rounded transition-colors`}>
                + 从统计列表添加
              </button>
            </div>
          </section>
        </div>

        <div className={`flex items-center justify-end gap-3 px-6 py-4 border-t ${s.border}`}>
          <button onClick={onClose} className={`px-4 py-2 transition-colors ${isLight ? 'text-gray-600 hover:text-gray-800' : 'text-gray-300 hover:text-white'}`}>取消</button>
          <button onClick={handleSave} className={`px-4 py-2 text-white rounded transition-colors ${saved ? 'bg-green-600' : 'bg-blue-600 hover:bg-blue-700'}`}>
            {saved ? '已保存' : '保存'}
          </button>
        </div>
      </div>
    </div>
  )
}
