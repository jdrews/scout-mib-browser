/** Single node in the hierarchical MIB tree for UI rendering. */
export interface TreeNode {
  oid: string;
  name: string;
  syntax_type?: string;
  mib_name: string;
  children?: TreeNode[];
}

/** Result of a MIB search query (autocomplete). */
export interface MibSearchResult {
  oid: string;
  name: string;
  syntax_type: string;
  mib_name: string;
}

/** Metadata about a loaded MIB file for the Manage MIBs dialog. */
export interface LoadedMib {
  mibName: string;
  filePath: string;
  nodeCount: number;
  isFallback: boolean;
}

/** Status response from MIB loading operations. */
export interface LoadDirectoriesStatus {
  nodeCount: number;
  fallbackMibs: string[];
}

/** Application configuration read from the backend. */
export interface AppConfig {
  mib?: {
    directories?: string[];
  };
}
