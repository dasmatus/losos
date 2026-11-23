/* Native LosOS Office: capabilities and open/import routing. */

export type OfficeKind = "native" | "import" | "view";

export interface OfficeType {
  extension: string;
  edit: boolean;
  mode?: "native" | "import" | "view";
  importTarget?: string | null;
}

export interface OpenNative {
  kind: "native";
  fileId: string;
  name: string;
  writable: boolean;
  extension: string;
}

export interface OpenImport {
  kind: "import";
  fileId: string;
  name: string;
  extension: string;
  importTarget?: string | null;
}

export interface OpenView {
  kind: "view";
  fileId: string;
  name: string;
  extension: string;
}

export type OpenResponse = OpenNative | OpenImport | OpenView;

export type OfficeError = "notFound" | "unsupported" | "starting" | "signedOut" | "other";

export class OfficeFailure extends Error {
  constructor(
    readonly reason: OfficeError,
    readonly extension = "",
  ) {
    super(reason);
  }
}

export const CLOUD_OFFICE_API = "/cloud/api/office";

async function failure(res: Response): Promise<OfficeFailure> {
  let body: { error?: string; extension?: string } = {};
  try {
    body = (await res.json()) as typeof body;
  } catch {
    /* not JSON */
  }
  if (res.status === 401) return new OfficeFailure("signedOut");
  if (res.status === 404) return new OfficeFailure("notFound");
  if (res.status === 422) return new OfficeFailure("unsupported", body.extension ?? "");
  if (res.status === 502 || res.status === 503) return new OfficeFailure("starting");
  return new OfficeFailure("other");
}

export async function openOffice(fileId: string, base = CLOUD_OFFICE_API): Promise<OpenResponse> {
  const res = await fetch(`${base}/open`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify({ fileId }),
  });
  if (!res.ok) throw await failure(res);
  return (await res.json()) as OpenResponse;
}

export async function officeTypes(base = CLOUD_OFFICE_API): Promise<OfficeType[]> {
  const res = await fetch(`${base}/extensions`, { credentials: "same-origin" });
  if (!res.ok) throw await failure(res);
  return ((await res.json()) as { extensions?: OfficeType[] }).extensions ?? [];
}

export function opensInOffice(name: string, types: readonly OfficeType[]): boolean {
  const dot = name.lastIndexOf(".");
  if (dot <= 0) return false;
  const ext = name.slice(dot + 1).toLowerCase();
  return types.some((t) => t.extension === ext);
}

export function officeTypeFor(name: string, types: readonly OfficeType[]): OfficeType | null {
  const dot = name.lastIndexOf(".");
  if (dot <= 0) return null;
  const ext = name.slice(dot + 1).toLowerCase();
  return types.find((t) => t.extension === ext) ?? null;
}
