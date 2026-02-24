/**
 * Mint Semantic Map Types
 *
 * Type definitions matching spec/sourcemap-format.md
 */

export interface SemanticMap {
  version: 1;
  file: string;
  generated_by: string;
  generated_at: string;  // ISO8601
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

export interface MappableNode {
  id: string;           // e.g., "fibonacci", "fibonacci_arm_0"
  range: [number, number];
  nodeType: 'function' | 'type' | 'match_arm';
  ast: any;  // Declaration | MatchArm
  inferredType?: any;  // InferenceType
}
