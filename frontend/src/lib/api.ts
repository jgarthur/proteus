import { API_BASE_URL } from '../constants';
import type { CellResponse, MetricsMessage, SimConfig, SimStatusResponse } from '../types';

interface ErrorEnvelope {
  message?: string;
  error?:
    | string
    | {
        code?: string;
        message?: string;
        status?: number;
      };
  code?: string;
}

function normalizeFetchError(error: unknown): Error {
  if (error instanceof Error && (error.name === 'TypeError' || error.message === 'Failed to fetch')) {
    return new Error(`Backend unreachable at ${API_BASE_URL}. Start the Proteus server and reload.`);
  }

  if (error instanceof Error) {
    return error;
  }

  return new Error(`Backend request failed at ${API_BASE_URL}.`);
}

async function parseJson<T>(response: Response): Promise<T> {
  if (!response.ok) {
    let message = `${response.status} ${response.statusText}`;
    try {
      const data = (await response.json()) as ErrorEnvelope;
      if (typeof data.error === 'string') {
        message = data.error;
      } else if (data.error?.message) {
        message = data.error.message;
      } else {
        message = data.message ?? data.code ?? message;
      }
    } catch {
      // Ignore JSON parse failure and surface the status line instead.
    }

    throw new Error(message);
  }

  if (response.status === 204) {
    return undefined as T;
  }

  return (await response.json()) as T;
}

export async function getSimStatus(): Promise<SimStatusResponse | null> {
  try {
    const response = await fetch(`${API_BASE_URL}/v1/sim`);
    if (response.status === 404) {
      return null;
    }

    return parseJson<SimStatusResponse>(response);
  } catch (error) {
    throw normalizeFetchError(error);
  }
}

export async function createSimulation(config: SimConfig): Promise<SimStatusResponse> {
  try {
    const response = await fetch(`${API_BASE_URL}/v1/sim`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(config),
    });

    return parseJson<SimStatusResponse>(response);
  } catch (error) {
    throw normalizeFetchError(error);
  }
}

export async function postSimulationAction(
  action: 'start' | 'pause' | 'resume' | 'reset',
): Promise<SimStatusResponse> {
  try {
    const response = await fetch(`${API_BASE_URL}/v1/sim/${action}`, {
      method: 'POST',
    });

    return parseJson<SimStatusResponse>(response);
  } catch (error) {
    throw normalizeFetchError(error);
  }
}

export async function stepSimulation(count: number): Promise<SimStatusResponse> {
  try {
    const response = await fetch(`${API_BASE_URL}/v1/sim/step?count=${count}`, {
      method: 'POST',
    });

    return parseJson<SimStatusResponse>(response);
  } catch (error) {
    throw normalizeFetchError(error);
  }
}

export async function destroySimulation(): Promise<void> {
  try {
    const response = await fetch(`${API_BASE_URL}/v1/sim`, {
      method: 'DELETE',
    });

    if (response.status === 404) {
      return;
    }

    await parseJson<void>(response);
  } catch (error) {
    throw normalizeFetchError(error);
  }
}

export async function fetchCell(x: number, y: number): Promise<CellResponse> {
  try {
    const response = await fetch(`${API_BASE_URL}/v1/sim/cell?x=${x}&y=${y}`);
    return parseJson<CellResponse>(response);
  } catch (error) {
    throw normalizeFetchError(error);
  }
}

export async function fetchMetrics(): Promise<MetricsMessage> {
  try {
    const response = await fetch(`${API_BASE_URL}/v1/sim/metrics`);
    return parseJson<MetricsMessage>(response);
  } catch (error) {
    throw normalizeFetchError(error);
  }
}
