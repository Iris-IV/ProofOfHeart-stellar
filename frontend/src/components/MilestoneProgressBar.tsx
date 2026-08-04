import React from 'react';
import { CampaignMilestone } from '@/types/campaign';

interface MilestoneProgressBarProps {
  milestones: CampaignMilestone[];
}

export const MilestoneProgressBar: React.FC<MilestoneProgressBarProps> = ({ milestones }) => {
  if (!milestones || milestones.length === 0) {
    return null;
  }

  return (
    <div className="space-y-6 my-6 p-6 bg-white rounded-xl shadow-sm border border-gray-100">
      <h3 className="text-lg font-semibold text-gray-900">Campaign Milestones</h3>
      <div className="space-y-4">
        {milestones.map((milestone, index) => {
          const progressPercentage = Math.min(
            Math.round((milestone.currentAmount / milestone.targetAmount) * 100),
            100
          );

          return (
            <div key={milestone.id || index} className="space-y-2">
              <div className="flex justify-between items-center text-sm">
                <span className="font-medium text-gray-800">
                  {index + 1}. {milestone.title}
                </span>
                <span className="text-gray-600">
                  {milestone.currentAmount} / {milestone.targetAmount} XLM ({progressPercentage}%)
                </span>
              </div>
              <div className="w-full bg-gray-200 rounded-full h-2.5 overflow-hidden">
                <div
                  className={`h-2.5 rounded-full transition-all duration-500 ${
                    milestone.isCompleted ? 'bg-green-600' : 'bg-indigo-600'
                  }`}
                  style={{ width: `${progressPercentage}%` }}
                />
              </div>
              <p className="text-xs text-gray-500">{milestone.description}</p>
            </div>
          );
        })}
      </div>
    </div>
  );
};