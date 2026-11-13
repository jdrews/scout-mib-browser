/** Pluralizes a count label: pluralize(1, "node") -> "1 node", pluralize(3, "node") -> "3 nodes". */
export function pluralize(count: number, singular: string, plural?: string): string {
  return `${count} ${count === 1 ? singular : (plural ?? `${singular}s`)}`;
}

/** Strips leading whitespace from every line of a multi-line MIB string.
 *  DESCRIPTION text starts right after the opening quote, so the first line
 *  is unindented while continuation lines carry the source file's indent;
 *  under whitespace-pre-wrap that renders as centered text. Interior
 *  spacing is preserved and blank lines stay blank. */
export function dedent(text: string): string {
  return text
    .split("\n")
    .map((line) => line.replace(/^\s+/, ""))
    .join("\n");
}
