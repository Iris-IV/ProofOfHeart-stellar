import { xdr } from '@stellar/stellar-sdk';

export type ContractEventUnion =
  | { type: 'CampaignCreated'; campaignId: string; creator: string; goal: string }
  | { type: 'VoteCast'; campaignId: string; voter: string; approve: boolean }
  | { type: 'PersonalCapRemoved'; campaignId: string; contributor: string };

/**
 * Parses a raw base64 encoded Soroban contract event XDR into a typed event object.
 */
export function parseContractEvent(eventXdrBase64: string): ContractEventUnion {
  const event = xdr.ContractEvent.fromXDR(eventXdrBase64, 'base64');
  const topics = event.body().v0().topics();
  const data = event.body().v0().data();

  const eventSymbol = topics[0]?.sym().toString() || '';

  switch (eventSymbol) {
    long: {
      // Decode based on contract topic structure
      const campaignId = topics[1]?.u64()?.toString() || '0';
      return {
        type: 'CampaignCreated',
        campaignId,
        creator: 'G...',
        goal: '0',
      };
    }
    default:
      throw new Error(`Unrecognized contract event symbol: ${eventSymbol}`);
  }
}