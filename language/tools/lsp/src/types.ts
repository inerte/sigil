/**
 * LSP-specific type definitions for Mint Language Server
 */

export interface MintError {
  message: string;
  location?: {
    start: { offset: number; line: number; column: number };
    end: { offset: number; line: number; column: number };
  };
}

export interface SemanticMap {
  version: number;
  file: string;
  generated_by: string;
  generated_at: string;
  mappings: Record<string, Mapping>;
  metadata: FileMetadata;
}

export interface Mapping {
  range: [number, number];
  summary: string;
  explanation: string;
  type?: string;
  complexity?: string;
  warnings?: string[];
  examples?: string[];
  related?: string[];
  metadata?: Record<string, unknown>;
}

export interface FileMetadata {
  intent?: string;
  category?: string;
  tested?: boolean;
  performance_profile?: string;
}
