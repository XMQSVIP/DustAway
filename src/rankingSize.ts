export function relativeSize(bytes: string, maximum: bigint) {
  try {
    return maximum > 0n ? Number((BigInt(bytes) * 10000n) / maximum) / 100 : 0;
  } catch {
    return 0;
  }
}
