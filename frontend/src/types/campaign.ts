export interface CampaignMilestone {
  id: string;
  title: string;
  description: string;
  targetAmount: number;
  currentAmount: number;
  isCompleted: boolean;
  dueDate?: string;
}

export interface CampaignWithMilestones {
  id: string;
  title: string;
  goalAmount: number;
  totalRaised: number;
  milestones: CampaignMilestone[];
}