/** Pluralizes a count label: pluralize(1, "node") -> "1 node", pluralize(3, "node") -> "3 nodes". */
export function pluralize(count: number, singular: string, plural?: string): string {
  return `${count} ${count === 1 ? singular : (plural ?? `${singular}s`)}`;
}
