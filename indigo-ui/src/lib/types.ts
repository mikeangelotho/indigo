export interface NodeInfo {
  id: string;
  address: string;
  port: number;
  model_name: string;
  is_local: boolean;
  status: string;
}

export interface ModelInfo {
  name: string;
  file: string;
}

// Tool Management Types
export interface ToolDefinition {
  id: string;
  name: string;
  description: string;
  parameters: any;
  type: ToolType;
  config: ToolConfig;
  permissions: string[];
  nodeCompatible: boolean;
  createdAt: string;
  updatedAt: string;
}

export enum ToolType {
  Cli = 'Cli',
  Http = 'Http',
  Mcp = 'Mcp',
  Native = 'Native',
  Wasm = 'Wasm'
}

export interface ToolConfig {
  type: 'Cli' | 'Http' | 'Mcp' | 'Wasm' | 'Native';
  // CLI Config
  command?: string;
  args?: string[];
  workingDir?: string;
  environment?: Record<string, string>;
  timeout?: number;
  // HTTP Config
  endpoint?: string;
  method?: string;
  headers?: Record<string, string>;
  authType?: string;
  authToken?: string;
  verifySsl?: boolean;
  // MCP Config
  serverName?: string;
  serverType?: string;
  // WASM Config
  modulePath?: string;
  functionName?: string;
  memoryLimit?: number;
  // Native Config
  nativeName?: string;
}

// Store state
export interface ToolRegistryState {
  tools: ToolDefinition[];
  loading: boolean;
  error: string | null;
  selectedTool: ToolDefinition | null;
  isCreating: boolean;
  isEditing: boolean;
  formData: {
    name: string;
    description: string;
    type: ToolType;
    command?: string;
    endpoint?: string;
    method?: string;
    serverName?: string;
    serverType?: string;
    permissions: string[];
    nodeCompatible: boolean;
    parameters: string;
  };
}