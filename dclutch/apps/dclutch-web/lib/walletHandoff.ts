/**
 * Browser-internal compatibility path.
 *
 * The SDK's conformance module owns these primitives, while its public
 * `walletHandoff` facade deliberately remains inspection-only. Product routes
 * must still wrap signing and submission in their operation-specific durable
 * journals.
 */
export * from '../../../packages/dclutch-sdk/lib/walletHandoff';
