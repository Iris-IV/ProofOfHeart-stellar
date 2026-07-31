export interface Campaign {
  id: string;
  creator: string;
  title: string;
  description: string;
  goalAmount: string;
  totalRaised: string;
  deadline: number;
  category: string;
  tags: string[];
  isCompleted: boolean;
}

export interface PlatformStats {
  totalCampaigns: number;
  totalVolumeXlm: string;
  activeContributors: number;
}

export type Category = 'DeFi' | 'Social' | 'Infrastructure' | 'NFT' | 'Education';