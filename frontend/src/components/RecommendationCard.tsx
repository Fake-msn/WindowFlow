import { useTheme } from '../contexts/ThemeContext'

interface Recommendation {
  workflow_label: string
  score: number
  source: string
}

interface RecommendationCardProps {
  recommendation: Recommendation
}

export default function RecommendationCard({ recommendation }: RecommendationCardProps) {
  const { resolvedTheme } = useTheme()
  const isLight = resolvedTheme === 'light'
  const scorePercentage = Math.round(recommendation.score * 100)
  
  return (
    <div className={`border rounded-lg p-4 transition-all cursor-pointer ${
      isLight
        ? 'bg-gradient-to-r from-purple-100/30 to-blue-100/30 border-blue-200 hover:border-blue-300'
        : 'bg-gradient-to-r from-purple-500/10 to-blue-500/10 border-purple-500/30 hover:border-purple-500/50'
    }`}>
      <div className="flex items-center justify-between">
        <div className="flex-1">
          <div className={`font-medium mb-1 ${isLight ? 'text-gray-800' : 'text-white'}`}>
            {recommendation.workflow_label}
          </div>
          <div className="flex items-center gap-2">
            <div className={`flex-1 rounded-full h-1.5 ${isLight ? 'bg-blue-100/50' : 'bg-gray-700/50'}`}>
              <div
                className="bg-gradient-to-r from-purple-500 to-blue-500 h-1.5 rounded-full transition-all"
                style={{ width: `${scorePercentage}%` }}
              />
            </div>
            <span className={`text-xs ${isLight ? 'text-gray-500' : 'text-gray-400'}`}>{scorePercentage}%</span>
          </div>
        </div>
        
        <div className="ml-4">
          <div className={`text-xs px-2 py-1 rounded ${
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
