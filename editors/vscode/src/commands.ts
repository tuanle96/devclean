export interface Filters {
  olderThan: string;
  minSize: string;
}

export function scanArguments(root: string, filters: Filters): string[] {
  return [
    "scan",
    "--format",
    "json",
    "--older-than",
    filters.olderThan,
    "--min-size",
    filters.minSize,
    root,
  ];
}

export function reportArguments(root: string, output: string, filters: Filters): string[] {
  return [
    "scan",
    "--format",
    "html",
    "--output",
    output,
    "--older-than",
    filters.olderThan,
    "--min-size",
    filters.minSize,
    root,
  ];
}

export function cleanArguments(root: string, filters: Filters, confirmed: boolean): string[] {
  return [
    "clean",
    confirmed ? "--yes" : "--dry-run",
    "--older-than",
    filters.olderThan,
    "--min-size",
    filters.minSize,
    root,
  ];
}
