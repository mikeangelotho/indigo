import { createSignal, createEffect, For, onMount, Show } from 'solid-js';
import { useNavigate } from '@solidjs/router';
import { A, createRouteAction } from '@solidjs/router';
import { createStore, unwrap } from 'solid-js/store';
import { ToolDefinition, ToolType, ToolConfig } from '../../lib/types';

// Types for the UI
interface ToolRegistryState {
  tools: ToolDefinition[];
  loading: boolean;
  error: string | null;
  selectedTool: ToolDefinition | null;
  isCreating: boolean;
  isEditing: boolean;
}

interface ToolFormData {
  name: string;
  description: string;
  type: ToolType;
  command?: string;
  endpoint?: string;
  method?: string;
  serverName?: string;
  serverType?: string;
  nativeName?: string;
  permissions: string[];
  nodeCompatible: boolean;
  parameters: string;
}

const [tools, setTools] = createSignal<ToolDefinition[]>([]);
const [loading, setLoading] = createSignal(false);
const [error, setError] = createSignal<string | null>(null);
const [selectedTool, setSelectedTool] = createSignal<ToolDefinition | null>(null);
const [isCreating, setIsCreating] = createSignal(false);
const [isEditing, setIsEditing] = createSignal(false);

// Form data for creating/editing tools
const [formData, setFormData] = createSignal<ToolFormData>({
  name: '',
  description: '',
  type: ToolType.Cli,
  permissions: ['file:read'],
  nodeCompatible: true,
  parameters: JSON.stringify({
    type: 'object',
    properties: {},
    required: []
  }, null, 2)
});

// API functions
const fetchTools = async () => {
  setLoading(true);
  setError(null);
  
  try {
    const response = await fetch('/v1/tools');
    if (!response.ok) {
      throw new Error(`Failed to fetch tools: ${response.statusText}`);
    }
    const toolsData = await response.json();
    setTools(toolsData);
  } catch (err) {
    setError(err instanceof Error ? err.message : 'Failed to fetch tools');
  } finally {
    setLoading(false);
  }
};

const saveTool = async (toolData: ToolFormData) => {
  setLoading(true);
  setError(null);
  
  try {
    const method = isEditing() ? 'PUT' : 'POST';
    const url = isEditing() ? `/v1/tools/${selectedTool()?.id}` : '/v1/tools';
    
    // Convert form data to ToolDefinition
    let config: ToolConfig;
    let parameters;
    
    try {
      parameters = JSON.parse(toolData.parameters);
    } catch {
      setError('Invalid JSON in parameters');
      setLoading(false);
      return;
    }
    
    switch (toolData.type) {
      case ToolType.Cli:
        config = {
          type: 'Cli',
          command: toolData.command || '',
          args: [],
          workingDir: '.',
          environment: {},
          timeout: 30
        };
        break;
      case ToolType.Http:
        config = {
          type: 'Http',
          endpoint: toolData.endpoint || '',
          method: toolData.method || 'GET',
          headers: {},
          authType: '',
          authToken: '',
          verifySsl: true
        };
        break;
      case ToolType.Mcp:
        config = {
          type: 'Mcp',
          serverName: toolData.serverName || '',
          serverType: toolData.serverType || 'stdio',
          args: [],
          environment: {},
          endpoint: ''
        };
        break;
      case ToolType.Native:
        config = {
          type: 'Native',
          nativeName: toolData.nativeName || toolData.name,
        };
        break;
      default:
        config = {
          type: 'Cli',
          command: '',
          args: [],
          workingDir: '.',
          environment: {},
          timeout: 30
        };
    }
    
    const toolDefinition: ToolDefinition = {
      id: isEditing() ? selectedTool()?.id || '' : `tool_${Date.now()}`,
      name: toolData.name,
      description: toolData.description,
      parameters,
      type: toolData.type,
      config,
      permissions: toolData.permissions,
      nodeCompatible: toolData.nodeCompatible,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString()
    };
    
    const response = await fetch(url, {
      method,
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(toolDefinition)
    });
    
    if (!response.ok) {
      throw new Error(`Failed to save tool: ${response.statusText}`);
    }
    
    // Reset form and refresh tools
    setFormData({
      name: '',
      description: '',
      type: ToolType.Cli,
      permissions: ['file:read'],
      nodeCompatible: true,
      parameters: JSON.stringify({
        type: 'object',
        properties: {},
        required: []
      }, null, 2)
    });
    setIsCreating(false);
    setIsEditing(false);
    setSelectedTool(null);
    
    // Refresh tools list
    await fetchTools();
  } catch (err) {
    setError(err instanceof Error ? err.message : 'Failed to save tool');
  } finally {
    setLoading(false);
  }
};

const deleteTool = async (toolId: string) => {
  if (!confirm('Are you sure you want to delete this tool?')) {
    return;
  }
  
  setLoading(true);
  setError(null);
  
  try {
    const response = await fetch(`/v1/tools/${toolId}`, {
      method: 'DELETE'
    });
    
    if (!response.ok) {
      throw new Error(`Failed to delete tool: ${response.statusText}`);
    }
    
    // Refresh tools list
    await fetchTools();
  } catch (err) {
    setError(err instanceof Error ? err.message : 'Failed to delete tool');
  } finally {
    setLoading(false);
  }
};

const editTool = (tool: ToolDefinition) => {
  setSelectedTool(tool);
  setIsEditing(true);
  setIsCreating(false);
  
  // Convert tool to form data
  let command = '';
  let endpoint = '';
  let method = 'GET';
  let serverName = '';
  let serverType = 'stdio';
  let nativeName = '';
  
  if (tool.config.type === 'Cli') {
    command = tool.config.command || '';
  } else if (tool.config.type === 'Http') {
    endpoint = tool.config.endpoint || '';
    method = tool.config.method || 'GET';
  } else if (tool.config.type === 'Mcp') {
    serverName = tool.config.serverName || '';
    serverType = tool.config.serverType || 'stdio';
  } else if (tool.config.type === 'Native') {
    nativeName = tool.config.nativeName || '';
  }
  
  setFormData({
    name: tool.name,
    description: tool.description,
    type: tool.type,
    command,
    endpoint,
    method,
    serverName,
    serverType,
    nativeName,
    permissions: tool.permissions,
    nodeCompatible: tool.nodeCompatible,
    parameters: JSON.stringify(tool.parameters, null, 2)
  });
};

// Load tools on component mount
createEffect(() => {
  fetchTools();
});

export default function ToolsPage() {
  const navigate = useNavigate();
  
  return (
    <div class="container mx-auto p-6">
      <div class="mb-6">
        <h1 class="text-3xl font-bold text-gray-900 mb-2">Tools Management</h1>
        <p class="text-gray-600 mb-4">Create and manage custom tools for your AI agents</p>
        
        <div class="flex justify-between items-center mb-4">
          <button
            class="bg-blue-600 hover:bg-blue-700 text-white px-4 py-2 rounded-md transition-colors"
            onClick={() => {
              setSelectedTool(null);
              setIsCreating(true);
              setIsEditing(false);
              setFormData({
                name: '',
                description: '',
                type: ToolType.Cli,
                permissions: ['file:read'],
                nodeCompatible: true,
                parameters: JSON.stringify({
                  type: 'object',
                  properties: {},
                  required: []
                }, null, 2)
              });
            }}
          >
            Create New Tool
          </button>
        </div>
      </div>
      
      {/* Error Display */}
      <Show when={error()}>
        {(errorMsg) => (
          <div class="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded mb-4">
            <strong>Error:</strong> {errorMsg}
          </div>
        )}
      </Show>
      
      {/* Loading State */}
      <Show when={loading()}>
        <div class="flex justify-center py-8">
          <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600"></div>
        </div>
      </Show>
      
      {/* Tool Creation/Edit Form */}
      <Show when={isCreating() || isEditing()}>
        <div class="bg-white shadow-md rounded-lg p-6 mb-6">
          <h2 class="text-xl font-semibold mb-4">
            {isCreating() ? 'Create New Tool' : 'Edit Tool'}
          </h2>
          
          <form onSubmit={(e) => { e.preventDefault(); saveTool(formData()); }}>
            <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-2">Tool Name</label>
                <input
                  type="text"
                  value={formData().name}
                  onInput={(e) => setFormData({ ...formData(), name: e.target.value })}
                  class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                  required
                />
              </div>
              
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-2">Tool Type</label>
                <select
                  value={formData().type}
                  onChange={(e) => setFormData({ ...formData(), type: e.target.value as ToolType })}
                  class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                >
                  <option value={ToolType.Native}>Native Opencode Tool</option>
                  <option value={ToolType.Cli}>CLI Command</option>
                  <option value={ToolType.Http}>HTTP API</option>
                  <option value={ToolType.Mcp}>MCP Server</option>
                  <option value={ToolType.Wasm}>WASM Module</option>
                </select>
              </div>
            </div>
            
            <div class="mb-4">
              <label class="block text-sm font-medium text-gray-700 mb-2">Description</label>
              <textarea
                value={formData().description}
                onInput={(e) => setFormData({ ...formData(), description: e.target.value })}
                class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                rows={3}
                required
              />
            </div>
            
            {/* Type-specific configuration */}
            <div class="mb-4">
              <h3 class="text-lg font-medium mb-2">Configuration</h3>
              
              <Show when={formData().type === ToolType.Native}>
                <div>
                  <label class="block text-sm font-medium text-gray-700 mb-2">Opencode Tool Type</label>
                  <select
                    value={formData().nativeName}
                    onChange={(e) => setFormData({ ...formData(), nativeName: e.target.value })}
                    class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                  >
                    <option value="">Select a tool...</option>
                    <option value="bash">bash (Shell Command)</option>
                    <option value="read">read (Read File)</option>
                    <option value="glob">glob (List Files)</option>
                    <option value="grep">grep (Search Content)</option>
                    <option value="write_file">write_file (Write File)</option>
                  </select>
                </div>
              </Show>

              <Show when={formData().type === ToolType.Cli}>
                <div>
                  <label class="block text-sm font-medium text-gray-700 mb-2">Command</label>
                  <input
                    type="text"
                    value={formData().command}
                    onInput={(e) => setFormData({ ...formData(), command: e.target.value })}
                    class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                    placeholder="e.g., ls, grep, curl"
                  />
                </div>
              </Show>
              
              <Show when={formData().type === ToolType.Http}>
                <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <label class="block text-sm font-medium text-gray-700 mb-2">API Endpoint</label>
                    <input
                      type="url"
                      value={formData().endpoint}
                      onInput={(e) => setFormData({ ...formData(), endpoint: e.target.value })}
                      class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                      placeholder="https://api.example.com/endpoint"
                    />
                  </div>
                  <div>
                    <label class="block text-sm font-medium text-gray-700 mb-2">HTTP Method</label>
                    <select
                      value={formData().method}
                      onChange={(e) => setFormData({ ...formData(), method: e.target.value })}
                      class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                    >
                      <option value="GET">GET</option>
                      <option value="POST">POST</option>
                      <option value="PUT">PUT</option>
                      <option value="DELETE">DELETE</option>
                      <option value="PATCH">PATCH</option>
                    </select>
                  </div>
                </div>
              </Show>
              
              <Show when={formData().type === ToolType.Mcp}>
                <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <label class="block text-sm font-medium text-gray-700 mb-2">Server Name</label>
                    <input
                      type="text"
                      value={formData().serverName}
                      onInput={(e) => setFormData({ ...formData(), serverName: e.target.value })}
                      class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                      placeholder="e.g., filesystem, github"
                    />
                  </div>
                  <div>
                    <label class="block text-sm font-medium text-gray-700 mb-2">Server Type</label>
                    <select
                      value={formData().serverType}
                      onChange={(e) => setFormData({ ...formData(), serverType: e.target.value })}
                      class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                    >
                      <option value="stdio">STDIO</option>
                      <option value="http">HTTP</option>
                    </select>
                  </div>
                </div>
              </Show>
            </div>
            
            <div class="mb-4">
              <label class="block text-sm font-medium text-gray-700 mb-2">Parameters (JSON Schema)</label>
              <textarea
                value={formData().parameters}
                onInput={(e) => setFormData({ ...formData(), parameters: e.target.value })}
                class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 font-mono text-sm"
                rows={6}
                placeholder='{"type": "object", "properties": {}, "required": []}'
              />
            </div>
            
            <div class="flex justify-between">
              <button
                type="button"
                onClick={() => {
                  setIsCreating(false);
                  setIsEditing(false);
                  setSelectedTool(null);
                }}
                class="bg-gray-300 hover:bg-gray-400 text-gray-800 px-4 py-2 rounded-md transition-colors"
              >
                Cancel
              </button>
              <button
                type="submit"
                disabled={loading()}
                class="bg-blue-600 hover:bg-blue-700 disabled:bg-blue-400 text-white px-4 py-2 rounded-md transition-colors"
              >
                {loading() ? 'Saving...' : (isEditing() ? 'Update Tool' : 'Create Tool')}
              </button>
            </div>
          </form>
        </div>
      </Show>
      
      {/* Tools List */}
      <Show when={!isCreating() && !isEditing()}>
        <div class="bg-white shadow-md rounded-lg overflow-hidden">
          <div class="px-6 py-4 border-b border-gray-200">
            <h2 class="text-xl font-semibold">Available Tools</h2>
          </div>
          
          <Show when={tools().length === 0 && !loading()}>
            <div class="text-center py-8 text-gray-500">
              No tools found. Create your first tool to get started.
            </div>
          </Show>
          
          <div class="divide-y divide-gray-200">
            <For each={tools()}>
              {(tool) => (
                <div class="p-6 hover:bg-gray-50 transition-colors">
                  <div class="flex justify-between items-start">
                    <div class="flex-1">
                      <h3 class="text-lg font-medium text-gray-900">{tool.name}</h3>
                      <p class="text-gray-600 mt-1">{tool.description}</p>
                      
                      <div class="mt-2">
                        <span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-blue-100 text-blue-800">
                          {tool.type}
                        </span>
                        <Show when={tool.nodeCompatible}>
                          <span class="ml-2 inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-800">
                            Node Compatible
                          </span>
                        </Show>
                      </div>
                    </div>
                    
                    <div class="flex space-x-2 ml-4">
                      <button
                        onClick={() => editTool(tool)}
                        class="bg-white hover:bg-gray-50 text-gray-700 px-3 py-2 rounded-md border border-gray-300 transition-colors"
                      >
                        Edit
                      </button>
                      <button
                        onClick={() => deleteTool(tool.id)}
                        class="bg-red-600 hover:bg-red-700 text-white px-3 py-2 rounded-md transition-colors"
                      >
                        Delete
                      </button>
                    </div>
                  </div>
                </div>
              )}
            </For>
          </div>
        </div>
      </Show>
    </div>
  );
}