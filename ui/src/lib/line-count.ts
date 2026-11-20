/** Line counts above 9,999 shorten to thousands: 12400 becomes "12.4k". */
export function formatLineCount(count: number): string {
  if (count <= 9999) return String(count);
  const thousands = Math.floor(count / 100) / 10;
  return `${thousands}k`;
}
