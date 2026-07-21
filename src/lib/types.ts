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

/** SNMP version for Target connections. */
export type SnmpVersion = "v1" | "v2c" | "v3";

/** Authentication protocol for SNMPv3 USM. */
export type V3AuthProtocol = "none" | "md5" | "sha1" | "sha224" | "sha256" | "sha384" | "sha512";

/** Privacy (encryption) protocol for SNMPv3 USM. */
export type V3PrivProtocol = "none" | "des" | "aes128" | "aes192" | "aes256";

/** Security level for SNMPv3. */
export type V3SecurityLevel = "noAuthNoPrivacy" | "authNoPrivacy" | "authPrivacy";

/// Last-used Target connection settings from config.
export interface TargetConfig {
  host: string;
  port: number;
  version: SnmpVersion;
  community: string;
  v3_username: string;
  v3_auth_protocol: V3AuthProtocol;
  v3_auth_passphrase: string;
  v3_priv_protocol: V3PrivProtocol;
  v3_priv_passphrase: string;
  v3_security_level: V3SecurityLevel;
}

/** Connection state for the Target. */
export type ConnectionState = "disconnected" | "connecting" | "connected";

/** Application configuration read from the backend. */
export interface AppConfig {
  mib?: {
    directories?: string[];
  };
  target?: Omit<TargetConfig, "host" | "port"> & Partial<Pick<TargetConfig, "host" | "port">>;
}
