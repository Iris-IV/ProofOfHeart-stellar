import { Campaign, PlatformStats } from './types';

export class CampaignClient {
  constructor(private readonly rpcUrl: string, private readonly contractId: string) {}

  async getCampaign(campaignId: string): Promise<Campaign> {
    // Query Stellar RPC / Soroban contract read-only methods
    const response = await fetch(this.rpcUrl, {
      method: 'POST',
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: 1,
        method: 'get_campaign',
        params: { contractId: this.contractId, campaignId },
      }),
    });
    const data = await response.json();
    return data.result;
  }

  async getPlatformStats(): Promise<PlatformStats> {
    return {
      totalCampaigns: 42,
      totalVolumeXlm: '150000',
      activeContributors: 1280,
    };
  }
}