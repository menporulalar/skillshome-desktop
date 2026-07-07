/**
 * mcpClient.ts — Module 4 task 4.11: MCP_Client for the Local_Model/BYOK_Frontier
 * path. Stages/confirms an already-locally-extracted result over the real MCP
 * server (skillshome-app's `app/api/mcp/route.ts`) using the official
 * `@modelcontextprotocol/sdk` client — not hand-rolled JSON-RPC. The server itself
 * uses the same SDK; wire-format details (JSON-RPC over SSE-formatted bodies,
 * initialize handshake, per-tool error wrapping) are handled by `Client.connect()`
 * and `Client.callTool()`/`readResource()`, not reimplemented here.
 *
 * `args`/`confirmedItems` below stay untyped (`Record<string, unknown>`/`unknown`)
 * deliberately — this client never needs to inspect the deep `ReviewPackage`/
 * `Local_Extraction_Result` nested shape, only transport it. The sidecar's own
 * extraction agents (`@menporulalar/agents-core`) already produce exactly the shape
 * skillshome-app's `stageIngestionInputShape` (lib/mcp/mcpToolSchemas.ts) expects.
 */
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StreamableHTTPClientTransport } from '@modelcontextprotocol/sdk/client/streamableHttp.js';

export async function connectMcpClient(backendUrl: string, accessToken: string): Promise<Client> {
  const transport = new StreamableHTTPClientTransport(new URL('/api/mcp', backendUrl), {
    requestInit: { headers: { Authorization: `Bearer ${accessToken}` } },
  });
  const client = new Client({ name: 'skillshome-desktop-sidecar', version: '0.0.0' });
  // Handshakes `initialize` internally — this is the point of using the official SDK
  // rather than hand-rolling JSON-RPC framing.
  await client.connect(transport);
  return client;
}

export async function getProfileContext(client: Client, profileId: string): Promise<unknown> {
  const result = await client.readResource({ uri: `profile://${profileId}/context` });
  const content = result.contents[0];
  if (!('text' in content)) {
    throw new Error('profile.context returned a non-text resource content (unexpected)');
  }
  return JSON.parse(content.text);
}

// Mirrors skillshome-app's lib/mcp/mcpErrors.ts::parseMcpToolErrorReason (as of this
// writing, reasons are 'budget_exceeded' | 'concurrent_job') — duplicated, not
// imported: this is a separate repo/package with no dependency on skillshome-app.
// If the server adds a new reason value, this copy needs a matching update; nothing
// will fail loudly if it doesn't.
const MCP_ERROR_MESSAGE_PREFIX = /^MCP error -?\d+: /;

export function parseMcpToolErrorReason(errorText: string): { reason: string; message: string } | null {
  try {
    const parsed: unknown = JSON.parse(errorText.replace(MCP_ERROR_MESSAGE_PREFIX, ''));
    if (
      parsed !== null &&
      typeof parsed === 'object' &&
      'reason' in parsed &&
      'message' in parsed
    ) {
      return parsed as { reason: string; message: string };
    }
    return null;
  } catch {
    return null;
  }
}

async function callToolOrThrow(
  client: Client,
  name: string,
  args: Record<string, unknown>,
): Promise<unknown> {
  const result = await client.callTool({ name, arguments: args });
  const content = (result.content as Array<{ type: string; text?: string }>)[0];
  const text = content?.text ?? '';

  if (result.isError) {
    const structured = parseMcpToolErrorReason(text);
    throw new Error(structured ? `${structured.reason}: ${structured.message}` : text);
  }
  return JSON.parse(text);
}

export async function stageIngestion(
  client: Client,
  args: Record<string, unknown>,
): Promise<{ jobId: string; reviewPackage: unknown }> {
  return callToolOrThrow(client, 'profile.ingest.stage', args) as Promise<{
    jobId: string;
    reviewPackage: unknown;
  }>;
}

export async function confirmIngestion(
  client: Client,
  profileId: string,
  confirmedItems: unknown,
): Promise<{ success: boolean }> {
  return callToolOrThrow(client, 'profile.ingest.confirm', { profileId, confirmedItems }) as Promise<{
    success: boolean;
  }>;
}
