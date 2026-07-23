import { useTheme } from '../contexts/ThemeContext'

interface AboutDialogProps {
  onClose: () => void
}

export default function AboutDialog({ onClose }: AboutDialogProps) {
  const { resolvedTheme } = useTheme()
  const isLight = resolvedTheme === 'light'

  return (
    <div className="fixed inset-0 flex items-center justify-center z-50" style={{ backgroundColor: isLight ? 'rgba(0,0,0,0.2)' : 'rgba(0,0,0,0.5)' }}>
      <div className={`rounded-xl shadow-2xl border w-[400px] overflow-hidden ${
        isLight ? 'bg-[#E1F5FE] border-blue-200' : 'bg-gray-900 border-gray-700'
      }`} style={isLight ? { backgroundColor: '#E1F5FE' } : { backgroundColor: '#1a1d23' }}>
        {/* 头部 - 可拖拽区域 */}
        <div className={`flex items-center justify-between px-6 py-4 border-b ${isLight ? 'border-blue-200' : 'border-gray-700'}`} data-tauri-drag-region>
          <h2 className={`text-lg font-semibold ${isLight ? 'text-gray-800' : 'text-white'}`}>关于</h2>
          <button
            onClick={onClose}
            className={`transition-colors p-1 ${isLight ? 'text-gray-500 hover:text-gray-800' : 'text-gray-400 hover:text-white'}`}
          >
            <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        {/* 内容 */}
        <div className="px-6 py-6 text-center">
          {/* 图标 */}
          <div className="w-16 h-16 mx-auto mb-4 bg-gradient-to-br from-blue-500 to-purple-600 rounded-xl flex items-center justify-center">
            <svg className="w-10 h-10 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16M4 18h16" />
            </svg>
          </div>

          {/* 应用信息 */}
          <h3 className={`text-xl font-bold mb-2 ${isLight ? 'text-gray-800' : 'text-white'}`}>WindowFlow</h3>
          <p className={`text-sm mb-4 ${isLight ? 'text-gray-500' : 'text-gray-400'}`}>版本 1.2.0</p>

          {/* 描述 */}
          <p className={`text-sm mb-6 leading-relaxed ${isLight ? 'text-gray-700' : 'text-gray-300'}`}>
            新式桌面窗口管理软件<br />
            一键迁移多屏幕窗口，智能推荐工作流
          </p>

          {/* 技术栈 */}
          <div className={`rounded-lg p-3 mb-6 ${isLight ? 'bg-white/50' : 'bg-gray-800/50'}`}>
            <p className={`text-xs mb-1 ${isLight ? 'text-gray-500' : 'text-gray-400'}`}>技术栈</p>
            <p className={`text-sm ${isLight ? 'text-gray-700' : 'text-gray-300'}`}>Rust + Tauri + React</p>
          </div>

          {/* 版权信息 */}
          <p className={`text-xs ${isLight ? 'text-gray-400' : 'text-gray-500'}`}>
            © 2026 WindowFlow. All rights reserved.
          </p>
        </div>

        {/* 底部按钮 */}
        <div className={`flex items-center justify-center px-6 py-4 border-t ${isLight ? 'border-blue-200' : 'border-gray-700'}`}>
          <button
            onClick={onClose}
            className="px-6 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded transition-colors"
          >
            关闭
          </button>
        </div>
      </div>
    </div>
  )
}
